use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tokio::sync::mpsc as tokio_mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing_subscriber::EnvFilter;

mod assistant;
mod config;
mod dict;
mod events;
mod instruments;
mod ir;
mod osc;
mod parser;
mod pipe;
mod reconciler;
mod scd;
mod stage;
mod state;
mod timeline;
mod transport;
mod watch;
mod watcher;

use dict::DictStore;
use events::DaemonEvent;
use instruments::InstrumentStore;
use ir::{BufferManager, NoteSequencer, SequencerEvent};
use osc::ScsynthClient;
use parser::ThingType;
use pipe::executor::expand_pipe;
use pipe::parser::parse_pipe_block;
use reconciler::ReconcileOp;
use stage::StageStore;
use state::StateStore;
use transport::{TransportCmd, TransportReply};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // CLI client mode: if args provided, dispatch command to daemon and exit
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        run_cli(&args[1..]).await?;
        return Ok(());
    }

    // Daemon mode
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cfg = config::Config::load()?;
    tracing::info!("scsynth host: {}", cfg.scsynth_host);
    println!("hum-rt: scsynth host = {}", cfg.scsynth_host);

    // Connect to scsynth
    let mut client = ScsynthClient::connect(&cfg.scsynth_host).await?;

    // Health check -- fail fast with a clear error if scsynth is unreachable
    if let Err(e) = client.check_alive().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }

    // Ensure default group exists (headless scsynth doesn't create it)
    if let Err(e) = client.ensure_default_group().await {
        tracing::warn!("could not create default group: {e}");
    }

    // Load all .scd files from out/sc/ and send to scsynth
    let scd_dir_path = PathBuf::from("out/sc");
    let scd_store = match scd::ScdStore::load_dir(&scd_dir_path) {
        Ok(store) => {
            println!("hum-rt: found {} SynthDef(s) in out/sc/", store.len());
            store
        }
        Err(e) => {
            tracing::warn!("could not read out/sc/: {e}");
            scd::ScdStore::empty()
        }
    };

    // Load each SynthDef into scsynth
    for (thing_name, bytes) in scd_store.iter() {
        match client.load_synthdef(bytes.to_vec()).await {
            Ok(()) => {
                tracing::info!("loaded SynthDef for '{}'", thing_name);
                println!("hum-rt: loaded SynthDef '{}'", thing_name);
            }
            Err(e) => {
                tracing::warn!("failed to load SynthDef for '{}': {e}", thing_name);
            }
        }
    }

    // Load instrument definitions from instruments/ directory
    let instruments_path = PathBuf::from("instruments");
    let instrument_store = InstrumentStore::load_dir(&instruments_path)
        .unwrap_or_else(|e| {
            tracing::warn!("instruments/ load failed: {e}");
            InstrumentStore::default()
        });
    if let Some(count) = {
        // Quick count for logging
        let mut c = 0u32;
        let dir = Path::new("instruments");
        if dir.exists() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if entry.path().extension().and_then(|e| e.to_str()) == Some("hum") {
                        c += 1;
                    }
                }
            }
        }
        if c > 0 { Some(c) } else { None }
    } {
        println!("hum-rt: loaded {} instrument(s) from instruments/", count);
    }

    // Load dictionary: global (~/.config/hum/global.dict) + project (hum.dict)
    let global_dict_path = std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".config/hum/global.dict"))
        .unwrap_or_else(|| PathBuf::from("/tmp/no-global.dict"));
    let global_dict = DictStore::load(&global_dict_path).unwrap_or_default();
    let dict_path = PathBuf::from("hum.dict");
    let project_dict = DictStore::load(&dict_path).unwrap_or_default();
    let mut dict_store = DictStore::merge_with_global(global_dict, project_dict);
    println!("hum-rt: dict loaded ({} terms)", dict_store.all_terms().len());

    // Buffer manager for sample playback
    let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut buffer_mgr = BufferManager::new(project_root);

    // Parse piece.hum into desired state
    let mut state = StateStore::new();
    let mut stage_store = StageStore::new();
    let piece_hum_path = PathBuf::from("piece.hum");
    if let Ok(content) = std::fs::read_to_string(&piece_hum_path) {
        match parser::parse_hum(&content) {
            Ok(mut piece) => {
                tracing::info!("parsed {} things from piece.hum", piece.len());
                println!("hum-rt: parsed {} things from piece.hum", piece.len());

                // Resolve ref: and ref(thing).field references before compilation
                if let Err(e) = ir::ref_resolver::resolve_refs(&mut piece) {
                    tracing::error!("ref resolution failed: {e}");
                }

                // Pipe expansion pass: expand pipe: things into multiple synthetic things
                // Must happen after ref resolution so source things have resolved synths
                let pipe_expanded = expand_pipe_things(&piece);
                for (node_name, synth_block) in &pipe_expanded {
                    match ir::compile_synth_block(node_name, synth_block) {
                        Ok(bytes) => {
                            match client.load_synthdef(bytes).await {
                                Ok(()) => tracing::info!("'{}': pipe-expanded IR compiled and loaded", node_name),
                                Err(e) => tracing::error!("'{}': failed to load pipe-expanded IR: {}", node_name, e),
                            }
                        }
                        Err(e) => tracing::error!("'{}': pipe-expanded IR compilation failed: {}", node_name, e),
                    }
                }

                // IR compilation pass: compile synth: blocks for things without .scd override
                // If thing has instrument: field, merge instrument base with thing's synth
                // Skip stage things -- they are structural, not playable synths
                // Skip pipe things -- they are expanded above, not compiled directly
                for (name, thing) in piece.iter() {
                    // Skip stage things (handled below)
                    if thing.thing_type == Some(ThingType::Stage) {
                        continue;
                    }

                    // Skip pipe things -- expanded above
                    if thing.pipe.is_some() {
                        continue;
                    }

                    if scd_store.get(name).is_some() {
                        // Escape hatch: .scd exists, already loaded above, skip IR
                        tracing::info!("'{}': using .scd escape hatch", name);
                        continue;
                    }

                    // Resolve synth block: merge with instrument/dict base if specified
                    let resolved_synth = resolve_synth_block(thing, &instrument_store, &dict_store);

                    if let Some(synth_block) = &resolved_synth {
                        match ir::compile_synth_block(name, synth_block) {
                            Ok(bytes) => {
                                match client.load_synthdef(bytes).await {
                                    Ok(()) => tracing::info!("'{}': compiled IR and loaded SynthDef", name),
                                    Err(e) => tracing::error!("'{}': failed to load compiled IR: {}", name, e),
                                }
                            }
                            Err(e) => tracing::error!("'{}': IR compilation failed: {}", name, e),
                        }
                        // Load sample buffer if synth has sample: field
                        if let Some(sample_path) = &synth_block.sample {
                            let buf_id = buffer_mgr.alloc(sample_path);
                            let abs_path = buffer_mgr.resolve_path(sample_path);
                            match client.load_buffer(buf_id, &abs_path.to_string_lossy()).await {
                                Ok(()) => {
                                    tracing::info!("'{}': loaded sample buffer {} <- {}", name, buf_id, sample_path);
                                    println!("hum-rt: loaded sample '{}' -> buffer {}", sample_path, buf_id);
                                }
                                Err(e) => tracing::error!("'{}': failed to load sample '{}': {}", name, sample_path, e),
                            }
                        }
                    } else if scd_store.get(name).is_none() {
                        tracing::warn!("'{}': no synth: block and no .scd file — will not play", name);
                    }
                }

                // Stage setup: detect type: stage things, create scsynth groups,
                // compile + load effect SynthDefs, build StageStore
                for (name, thing) in piece.iter() {
                    if thing.thing_type != Some(ThingType::Stage) {
                        continue;
                    }
                    let applies_to = thing.applies_to.clone().unwrap_or_default();
                    if applies_to.is_empty() {
                        tracing::warn!("stage '{}': no applies-to list, skipping", name);
                        continue;
                    }

                    // Create scsynth Group node
                    match client.create_group().await {
                        Ok(group_id) => {
                            let mut effect_node_id = None;

                            // Compile and load effect SynthDef if stage has fx
                            if let Some(fx) = &thing.fx {
                                match stage::compile_stage_effect(name, fx) {
                                    Ok(bytes) => {
                                        let synthdef_name = format!("stage-{}", name);
                                        match client.load_synthdef(bytes).await {
                                            Ok(()) => {
                                                // Spawn effect node at tail of group
                                                match client.start_effect_at_tail(
                                                    &synthdef_name,
                                                    &synthdef_name,
                                                    group_id,
                                                ).await {
                                                    Ok(eid) => {
                                                        effect_node_id = Some(eid);
                                                        tracing::info!(
                                                            "stage '{}': group {} + effect node {} loaded",
                                                            name, group_id, eid
                                                        );
                                                    }
                                                    Err(e) => tracing::error!(
                                                        "stage '{}': failed to spawn effect node: {}",
                                                        name, e
                                                    ),
                                                }
                                            }
                                            Err(e) => tracing::error!(
                                                "stage '{}': failed to load effect SynthDef: {}",
                                                name, e
                                            ),
                                        }
                                    }
                                    Err(e) => tracing::error!(
                                        "stage '{}': effect compilation failed: {}",
                                        name, e
                                    ),
                                }
                            }

                            stage_store.insert(
                                name.clone(),
                                stage::StageConfig {
                                    applies_to,
                                    fx: thing.fx.clone(),
                                    group_id,
                                    effect_node_id,
                                },
                            );
                            println!(
                                "hum-rt: stage '{}' -> group {} (applies-to: {:?})",
                                name, group_id,
                                thing.applies_to.as_deref().unwrap_or(&[])
                            );
                        }
                        Err(e) => tracing::error!("stage '{}': failed to create group: {}", name, e),
                    }
                }

                // Inject pipe-expanded nodes into piece so reconciler sees them
                inject_pipe_nodes_into_piece(&mut piece, &pipe_expanded);

                state.desired = Some(piece);
            }
            Err(e) => {
                eprintln!("error parsing piece.hum: {e}");
            }
        }
    }

    // Divergence detection: track synth hashes across reloads
    let mut synth_hashes: HashMap<String, String> = HashMap::new();
    // Initialize hashes from initial parse
    if let Some(piece) = &state.desired {
        for (name, thing) in piece.iter() {
            if thing.pipe.is_some() {
                if let Some(synth) = &thing.synth {
                    synth_hashes.insert(name.clone(), format!("{:?}", synth));
                }
            }
        }
    }

    // Like: change detection: track like: values across reloads (SYNC-01)
    let mut like_hashes: HashMap<String, String> = HashMap::new();
    if let Some(piece) = &state.desired {
        update_like_hashes(&mut like_hashes, piece);
    }

    // Create event channel
    let (tx, mut rx) = tokio::sync::mpsc::channel::<DaemonEvent>(64);

    // Start file watcher (includes samples/ directory for buffer hot-reload)
    let samples_dir_path = PathBuf::from("samples");
    let mut watch_paths = vec![piece_hum_path.clone(), scd_dir_path.clone(), dict_path.clone()];
    if samples_dir_path.exists() {
        watch_paths.push(samples_dir_path);
    }
    watcher::start_watcher(&watch_paths, tx.clone())?;
    tracing::info!("watching piece.hum, out/sc/, hum.dict, and samples/ for changes");

    // Spawn timeline ticker (always runs; handle_tick gates on state.playing)
    let mut ticker_handle: Option<JoinHandle<()>> = Some(
        tokio::spawn(timeline::run_ticker(tx.clone(), 0.0)),
    );
    tracing::info!("timeline ticker started at pos 0.0");

    // Start transport socket server
    tokio::spawn(transport::start_socket_server(tx.clone()));
    tracing::info!("transport socket server started");

    // Sequencer infrastructure: channel + active handles map
    let (seq_tx, mut seq_rx) = tokio_mpsc::channel::<SequencerEvent>(256);
    let mut sequencer_handles: HashMap<String, JoinHandle<()>> = HashMap::new();

    // Do initial reconciliation -- start any things active at t=0
    reconcile_now(&mut state, &mut client, &seq_tx, &mut sequencer_handles, &stage_store, &buffer_mgr).await;

    println!("hum-rt: event loop running (Ctrl-C to stop)");

    // Event loop
    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                match event {
                    DaemonEvent::FileChanged(path) => {
                        handle_file_change(
                            &mut state, &mut client, &path,
                            &seq_tx, &mut sequencer_handles,
                            &instrument_store, &stage_store,
                            &mut dict_store,
                            &mut synth_hashes,
                            &mut like_hashes,
                            &mut buffer_mgr,
                        ).await;
                    }
                    DaemonEvent::Tick(pos) => {
                        handle_tick(
                            &mut state, &mut client, pos,
                            &mut ticker_handle, &tx,
                            &seq_tx, &mut sequencer_handles,
                            &stage_store,
                            &buffer_mgr,
                        ).await;
                    }
                    DaemonEvent::Transport(cmd, reply_tx) => {
                        handle_transport(
                            cmd, reply_tx,
                            &mut state, &mut client,
                            &mut ticker_handle, &tx,
                            &seq_tx, &mut sequencer_handles,
                            &stage_store,
                            &buffer_mgr,
                        ).await;
                    }
                }
            }
            Some(seq_event) = seq_rx.recv() => {
                match seq_event {
                    SequencerEvent::SetFreq { thing_name, freq } => {
                        if let Err(e) = client.set_param(&thing_name, "freq", freq).await {
                            tracing::debug!("sequencer set_param failed for '{}': {}", thing_name, e);
                        }
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("shutting down -- freeing all nodes");
                println!("shutting down -- freeing all nodes");
                // Abort all sequencers
                for (_, handle) in sequencer_handles.drain() {
                    handle.abort();
                }
                let _ = client.free_all_nodes().await;
                // Clean up socket file
                let _ = std::fs::remove_file(transport::SOCKET_PATH);
                break;
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// CLI client mode
// ---------------------------------------------------------------------------

async fn run_cli(args: &[String]) -> anyhow::Result<()> {
    let cmd = match args.first().map(|s| s.as_str()) {
        Some("play") => {
            // "play from <t>" => atomic PlayFrom (seek + play in one command)
            if args.len() >= 3 && args[1] == "from" {
                let pos = parse_time_arg(&args[2])?;
                TransportCmd::PlayFrom { pos }
            } else {
                TransportCmd::Play
            }
        }
        Some("stop") => TransportCmd::Stop,
        Some("status") => TransportCmd::Status,
        Some("loop") => {
            if args.len() < 3 {
                eprintln!("usage: hum-rt loop <start> <end>");
                std::process::exit(1);
            }
            let start = parse_time_arg(&args[1])?;
            let end = parse_time_arg(&args[2])?;
            TransportCmd::Loop { start, end }
        }
        Some("solo") => {
            if args.len() < 2 {
                eprintln!("usage: hum-rt solo <thing>");
                std::process::exit(1);
            }
            TransportCmd::Solo {
                thing: args[1].clone(),
            }
        }
        Some("mute") => {
            if args.len() < 2 {
                eprintln!("usage: hum-rt mute <thing>");
                std::process::exit(1);
            }
            TransportCmd::Mute {
                thing: args[1].clone(),
            }
        }
        Some("dict") => {
            // Dict commands are client-side only (no daemon needed, just file read)
            handle_dict_cli(&args[1..]);
            return Ok(());
        }
        Some("suggest") => {
            handle_suggest_cli();
            return Ok(());
        }
        Some("analyze") => {
            handle_analyze_cli();
            return Ok(());
        }
        Some("watch") => {
            watch::run_watch();
            return Ok(());
        }
        _ => {
            eprintln!(
                "usage: hum-rt [play|stop|status|watch|suggest|analyze|dict list|dict show <term>|dict add <thing> <term>|dict suggest|play from <t>|loop <s> <e>|solo <thing>|mute <thing>]"
            );
            std::process::exit(1);
        }
    };

    let reply = transport::send_cmd(cmd).await?;
    print_reply(&reply);
    Ok(())
}

/// Handle dict subcommands client-side (no daemon socket needed).
fn handle_dict_cli(args: &[String]) {
    // Load dict from project root (same logic as daemon startup)
    let global_dict_path = std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".config/hum/global.dict"))
        .unwrap_or_else(|| PathBuf::from("/tmp/no-global.dict"));
    let global_dict = DictStore::load(&global_dict_path).unwrap_or_default();
    let dict_path = PathBuf::from("hum.dict");
    let project_dict = DictStore::load(&dict_path).unwrap_or_default();
    let dict_store = DictStore::merge_with_global(global_dict, project_dict);

    match args.first().map(|s| s.as_str()) {
        Some("list") => {
            let terms = dict_store.all_terms();
            if terms.is_empty() {
                println!("(dictionary is empty)");
            } else {
                for term in &terms {
                    println!("  {}", term);
                }
                println!("{} term(s)", terms.len());
            }
        }
        Some("show") => {
            let term = match args.get(1) {
                Some(t) => t,
                None => {
                    eprintln!("usage: hum dict show <term>");
                    std::process::exit(1);
                }
            };
            match dict_store.get(term) {
                Some(entry) => {
                    println!("{}:", term);
                    println!("  synth: {:?}", entry.synth);
                    if let Some(ctx) = &entry.context {
                        println!("  context: {}", ctx);
                    }
                    if let Some(lf) = &entry.learned_from {
                        println!("  learned-from: {}", lf);
                    }
                }
                None => {
                    eprintln!("error: term '{}' not found in dictionary", term);
                    std::process::exit(1);
                }
            }
        }
        Some("add") => {
            let thing_name = match args.get(1) {
                Some(t) => t,
                None => {
                    eprintln!("usage: hum dict add <thing> <term>");
                    std::process::exit(1);
                }
            };
            let term = match args.get(2) {
                Some(t) => t,
                None => {
                    eprintln!("usage: hum dict add <thing> <term>");
                    std::process::exit(1);
                }
            };

            // Parse piece.hum to find the thing
            let piece_path = PathBuf::from("piece.hum");
            let content = match std::fs::read_to_string(&piece_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error reading piece.hum: {}", e);
                    std::process::exit(1);
                }
            };
            let piece = match parser::parse_hum(&content) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error parsing piece.hum: {}", e);
                    std::process::exit(1);
                }
            };

            let thing_def = match piece.get(thing_name.as_str()) {
                Some(t) => t,
                None => {
                    eprintln!("error: thing '{}' not found in piece.hum", thing_name);
                    std::process::exit(1);
                }
            };

            let synth = match &thing_def.synth {
                Some(s) => s.clone(),
                None => {
                    eprintln!("error: thing '{}' has no synth: block", thing_name);
                    std::process::exit(1);
                }
            };

            let entry = dict::DictEntry {
                synth,
                context: None,
                learned_from: Some(thing_name.to_string()),
            };

            let dict_path = PathBuf::from("hum.dict");
            match DictStore::add_entry(&dict_path, term, entry) {
                Ok(()) => {
                    println!("dict: added '{}' (learned from '{}')", term, thing_name);
                }
                Err(e) => {
                    eprintln!("error writing dict entry: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some("suggest") => {
            let piece_path = PathBuf::from("piece.hum");
            let content = match std::fs::read_to_string(&piece_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error reading piece.hum: {}", e);
                    std::process::exit(1);
                }
            };
            let piece: parser::Piece = match parser::parse_hum(&content) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error parsing piece.hum: {}", e);
                    std::process::exit(1);
                }
            };
            suggest_dict_entries(&piece);
        }
        _ => {
            eprintln!("usage: hum dict [list|show <term>|add <thing> <term>|suggest]");
            std::process::exit(1);
        }
    }
}

/// Handle `hum suggest` -- client-side structural analysis.
fn handle_suggest_cli() {
    let piece_path = PathBuf::from("piece.hum");
    let content = match std::fs::read_to_string(&piece_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error reading piece.hum: {}", e);
            std::process::exit(1);
        }
    };
    let piece = match parser::parse_hum(&content) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error parsing piece.hum: {}", e);
            std::process::exit(1);
        }
    };

    // Load dict
    let global_dict_path = std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".config/hum/global.dict"))
        .unwrap_or_else(|| PathBuf::from("/tmp/no-global.dict"));
    let global_dict = DictStore::load(&global_dict_path).unwrap_or_default();
    let dict_path = PathBuf::from("hum.dict");
    let project_dict = DictStore::load(&dict_path).unwrap_or_default();
    let dict_store = DictStore::merge_with_global(global_dict, project_dict);

    let hints = assistant::suggest(&piece, &dict_store);
    println!("Suggestions for piece.hum:");
    for hint in &hints {
        println!("  - {}", hint);
    }
}

/// Handle `hum analyze` -- client-side frequency balance assessment.
fn handle_analyze_cli() {
    let piece_path = PathBuf::from("piece.hum");
    let content = match std::fs::read_to_string(&piece_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error reading piece.hum: {}", e);
            std::process::exit(1);
        }
    };
    let piece = match parser::parse_hum(&content) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error parsing piece.hum: {}", e);
            std::process::exit(1);
        }
    };

    let lines = assistant::analyze(&piece);
    println!("Frequency balance analysis:");
    for line in &lines {
        println!("  {}", line);
    }
}

/// Parse a time argument supporting multiple formats:
/// - "10s" -> 10.0, "10" -> 10.0, "1.5s" -> 1.5
/// - "1m30s" -> 90.0, "1m" -> 60.0, "2m15s" -> 135.0
fn parse_time_arg(s: &str) -> anyhow::Result<f64> {
    if let Some(m_pos) = s.find('m') {
        // Has minutes component: split on 'm'
        let minutes_str = &s[..m_pos];
        let minutes: f64 = minutes_str
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid time (minutes): {}", s))?;
        let rest = &s[m_pos + 1..];
        let seconds: f64 = if rest.is_empty() {
            0.0
        } else {
            let stripped = rest.strip_suffix('s').unwrap_or(rest);
            stripped
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid time (seconds): {}", s))?
        };
        Ok(minutes * 60.0 + seconds)
    } else {
        let stripped = s.strip_suffix('s').unwrap_or(s);
        stripped
            .parse::<f64>()
            .map_err(|_| anyhow::anyhow!("invalid time: {}", s))
    }
}

fn print_reply(reply: &TransportReply) {
    match reply {
        TransportReply::Ack => println!("ok"),
        TransportReply::Status {
            playing,
            pos,
            active,
            solo,
            mute,
            amplitudes: _,
        } => {
            let status = if *playing { "playing" } else { "stopped" };
            println!("status: {}", status);
            println!("pos: {:.2}s", pos);
            if !active.is_empty() {
                println!("active: {}", active.join(", "));
            }
            if !solo.is_empty() {
                println!("solo: {}", solo.join(", "));
            }
            if !mute.is_empty() {
                println!("mute: {}", mute.join(", "));
            }
        }
        TransportReply::Error { message } => {
            eprintln!("error: {}", message);
        }
        TransportReply::DictVocab { terms } => {
            if terms.is_empty() {
                println!("(dictionary is empty)");
            } else {
                for term in terms {
                    println!("  {}", term);
                }
                println!("{} term(s)", terms.len());
            }
        }
        TransportReply::DictEntry { term, synth, context } => {
            println!("{}:", term);
            println!("  synth: {}", synth);
            if let Some(ctx) = context {
                println!("  context: {}", ctx);
            }
        }
        TransportReply::DictAdded { term } => {
            println!("added '{}' to dictionary", term);
        }
    }
}

// ---------------------------------------------------------------------------
// Transport command handler (daemon side)
// ---------------------------------------------------------------------------

async fn handle_transport(
    cmd: TransportCmd,
    reply_tx: oneshot::Sender<TransportReply>,
    state: &mut StateStore,
    client: &mut ScsynthClient,
    ticker_handle: &mut Option<JoinHandle<()>>,
    tx: &tokio::sync::mpsc::Sender<DaemonEvent>,
    seq_tx: &tokio_mpsc::Sender<SequencerEvent>,
    sequencer_handles: &mut HashMap<String, JoinHandle<()>>,
    stage_store: &StageStore,
    buffer_mgr: &BufferManager,
) {
    let reply = match cmd {
        TransportCmd::Play => {
            state.playing = true;
            restart_ticker(ticker_handle, tx, state.playback_pos);
            reconcile_now(state, client, seq_tx, sequencer_handles, stage_store, buffer_mgr).await;
            tracing::info!("transport: play from {:.2}s", state.playback_pos);
            TransportReply::Ack
        }
        TransportCmd::Stop => {
            state.playing = false;
            if let Some(handle) = ticker_handle.take() {
                handle.abort();
            }
            // Abort all sequencers
            for (_, handle) in sequencer_handles.drain() {
                handle.abort();
            }
            let _ = client.free_all_nodes().await;
            state.actual.nodes.clear();
            tracing::info!("transport: stop");
            TransportReply::Ack
        }
        TransportCmd::PlayFrom { pos } => {
            state.playback_pos = pos;
            state.playing = true;
            restart_ticker(ticker_handle, tx, pos);
            reconcile_now(state, client, seq_tx, sequencer_handles, stage_store, buffer_mgr).await;
            tracing::info!("transport: play from {:.2}s", pos);
            TransportReply::Ack
        }
        TransportCmd::Status => {
            let active: Vec<String> = state
                .active_things_filtered(state.playback_pos)
                .keys()
                .cloned()
                .collect();
            let solo: Vec<String> = state.solo_set.iter().cloned().collect();
            let mute: Vec<String> = state.mute_set.iter().cloned().collect();
            // Collect amplitudes for all active nodes (placeholder: 0.0)
            let mut amplitudes = std::collections::HashMap::new();
            for (thing_name, &node_id) in state.actual.nodes.iter() {
                if let Some(amp) = client.get_node_amplitude(node_id) {
                    amplitudes.insert(thing_name.clone(), amp);
                }
            }
            TransportReply::Status {
                playing: state.playing,
                pos: state.playback_pos,
                active,
                solo,
                mute,
                amplitudes,
            }
        }
        TransportCmd::Seek { pos } => {
            state.playback_pos = pos;
            restart_ticker(ticker_handle, tx, pos);
            if state.playing {
                reconcile_now(state, client, seq_tx, sequencer_handles, stage_store, buffer_mgr).await;
            }
            tracing::info!("transport: seek to {:.2}s", pos);
            TransportReply::Ack
        }
        TransportCmd::Loop { start, end } => {
            state.loop_range = Some((start, end));
            tracing::info!("transport: loop {:.2}s - {:.2}s", start, end);
            TransportReply::Ack
        }
        TransportCmd::Solo { thing } => {
            if state.solo_set.contains(&thing) {
                state.solo_set.remove(&thing);
                tracing::info!("transport: unsolo '{}'", thing);
            } else {
                state.solo_set.insert(thing.clone());
                tracing::info!("transport: solo '{}'", thing);
            }
            if state.playing {
                reconcile_now(state, client, seq_tx, sequencer_handles, stage_store, buffer_mgr).await;
            }
            TransportReply::Ack
        }
        TransportCmd::Mute { thing } => {
            if state.mute_set.contains(&thing) {
                state.mute_set.remove(&thing);
                tracing::info!("transport: unmute '{}'", thing);
            } else {
                state.mute_set.insert(thing.clone());
                tracing::info!("transport: mute '{}'", thing);
            }
            if state.playing {
                reconcile_now(state, client, seq_tx, sequencer_handles, stage_store, buffer_mgr).await;
            }
            TransportReply::Ack
        }
        TransportCmd::DictList | TransportCmd::DictShow { .. } | TransportCmd::DictAdd { .. } => {
            // Dict commands are handled in run_cli directly (no daemon needed)
            // If they arrive over the socket, return an error
            TransportReply::Error {
                message: "dict commands are handled client-side, not via daemon".to_string(),
            }
        }
    };

    let _ = reply_tx.send(reply);
}

/// Abort existing ticker (if any) and spawn a new one from the given position.
fn restart_ticker(
    ticker_handle: &mut Option<JoinHandle<()>>,
    tx: &tokio::sync::mpsc::Sender<DaemonEvent>,
    start_pos: f64,
) {
    if let Some(handle) = ticker_handle.take() {
        handle.abort();
    }
    *ticker_handle = Some(tokio::spawn(timeline::run_ticker(tx.clone(), start_pos)));
}

// ---------------------------------------------------------------------------
// File change handlers
// ---------------------------------------------------------------------------

/// Handle a file change event.
/// .hum files: reparse and reconcile.
/// .scd files: reload SynthDef and hot-swap if active.
async fn handle_file_change(
    state: &mut StateStore,
    client: &mut ScsynthClient,
    path: &Path,
    seq_tx: &tokio_mpsc::Sender<SequencerEvent>,
    sequencer_handles: &mut HashMap<String, JoinHandle<()>>,
    instrument_store: &InstrumentStore,
    stage_store: &StageStore,
    dict_store: &mut DictStore,
    synth_hashes: &mut HashMap<String, String>,
    like_hashes: &mut HashMap<String, String>,
    buffer_mgr: &mut BufferManager,
) {
    // Dict hot-reload: check before the extension match
    if path.file_name().and_then(|f| f.to_str()) == Some("hum.dict")
        || path.to_str().map(|s| s.ends_with("global.dict")).unwrap_or(false)
    {
        let global_dict_path = std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".config/hum/global.dict"))
            .unwrap_or_else(|| PathBuf::from("/tmp/no-global.dict"));
        let global_dict = DictStore::load(&global_dict_path).unwrap_or_default();
        let project_dict = DictStore::load(Path::new("hum.dict")).unwrap_or_default();
        *dict_store = DictStore::merge_with_global(global_dict, project_dict);
        tracing::info!("dict: hot-reloaded ({} terms)", dict_store.all_terms().len());
        println!("hum-rt: dict reloaded ({} terms)", dict_store.all_terms().len());
        return;
    }

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "hum" => {
            tracing::info!("file changed: {:?} (reparse)", path);
            match std::fs::read_to_string("piece.hum") {
                Ok(content) => match parser::parse_hum(&content) {
                    Ok(mut piece) => {
                        tracing::info!("reparsed {} things from piece.hum", piece.len());

                        // Divergence detection: check if synth: was manually edited
                        let updated_content = detect_divergences(&content, &piece, synth_hashes);
                        if updated_content != content {
                            // Write updated file with divergence comments
                            if let Err(e) = std::fs::write("piece.hum", &updated_content) {
                                tracing::error!("failed to write divergence comment: {e}");
                            } else {
                                tracing::info!("divergence: wrote comment(s) to piece.hum");
                            }
                        }
                        // Update synth hashes for next reload
                        for (name, thing) in piece.iter() {
                            if thing.pipe.is_some() {
                                if let Some(synth) = &thing.synth {
                                    synth_hashes.insert(name.clone(), format!("{:?}", synth));
                                }
                            }
                        }

                        // Like: change detection (SYNC-01)
                        let like_changed = detect_like_changes(like_hashes, &piece);
                        for thing_name in &like_changed {
                            tracing::info!(
                                "sync: like: changed for '{}' — LLM should regenerate pipe: and synth:",
                                thing_name
                            );
                            println!(
                                "hum: like: changed for '{}' (LLM action needed)",
                                thing_name
                            );
                        }
                        update_like_hashes(like_hashes, &piece);

                        // Resolve ref: and ref(thing).field references before recompilation
                        if let Err(e) = ir::ref_resolver::resolve_refs(&mut piece) {
                            tracing::error!("ref resolution failed on reload: {e}");
                        }

                        // Pipe expansion pass on reload: expand pipe things into synthetic nodes
                        let pipe_expanded = expand_pipe_things(&piece);
                        for (node_name, synth_block) in &pipe_expanded {
                            match ir::compile_synth_block(node_name, synth_block) {
                                Ok(bytes) => {
                                    match client.load_synthdef(bytes).await {
                                        Ok(()) => {
                                            tracing::info!("'{}': pipe-expanded IR recompiled on reload", node_name);
                                            // Hot-swap if node is running
                                            if state.actual.nodes.contains_key(node_name.as_str()) {
                                                match client.new_synth(node_name, node_name).await {
                                                    Ok(node_id) => {
                                                        state.actual.nodes.insert(node_name.clone(), node_id);
                                                        tracing::info!("'{}': hot-swapped pipe node", node_name);
                                                    }
                                                    Err(e) => tracing::error!("'{}': hot-swap pipe node failed: {}", node_name, e),
                                                }
                                            }
                                        }
                                        Err(e) => tracing::error!("'{}': pipe-expanded IR load failed: {}", node_name, e),
                                    }
                                }
                                Err(e) => tracing::error!("'{}': pipe-expanded IR recompilation failed: {}", node_name, e),
                            }
                        }

                        // Recompile IR for things with synth: blocks (no .scd override)
                        // Merge with instrument base if thing has instrument: field
                        // Skip stage things -- they are structural (restart hum-rt to reconfigure)
                        // Skip pipe things -- expanded above
                        let scd_dir = std::path::Path::new("out/sc");
                        for (name, thing) in piece.iter() {
                            // Skip stage things -- hot-swap not supported for stages
                            if thing.thing_type == Some(ThingType::Stage) {
                                tracing::warn!(
                                    "stage '{}': stage hot-swap not supported -- restart hum-rt to reconfigure stages",
                                    name
                                );
                                continue;
                            }
                            // Skip pipe things -- expanded above
                            if thing.pipe.is_some() {
                                continue;
                            }
                            // Escape hatch: if .scd file exists on disk, skip IR
                            let scd_path = scd_dir.join(format!("{}.scd", name));
                            if scd_path.exists() {
                                continue;
                            }
                            let resolved_synth = resolve_synth_block(thing, instrument_store, dict_store);
                            if let Some(synth_block) = &resolved_synth {
                                match ir::compile_synth_block(name, synth_block) {
                                    Ok(bytes) => {
                                        match client.load_synthdef(bytes).await {
                                            Ok(()) => {
                                                tracing::info!("'{}': hot-swap IR recompiled", name);
                                                // Load sample buffer if synth has sample: field
                                                if let Some(sample_path) = &synth_block.sample {
                                                    let buf_id = buffer_mgr.alloc(sample_path);
                                                    let abs_path = buffer_mgr.resolve_path(sample_path);
                                                    let _ = client.free_buffer(buf_id).await;
                                                    match client.load_buffer(buf_id, &abs_path.to_string_lossy()).await {
                                                        Ok(()) => tracing::info!("'{}': reloaded sample buffer {} <- {}", name, buf_id, sample_path),
                                                        Err(e) => tracing::error!("'{}': sample reload failed: {}", name, e),
                                                    }
                                                }
                                                // If thing is running, trigger a node swap
                                                if state.actual.nodes.contains_key(name.as_str()) {
                                                    let swap_result = if let Some(sample_path) = &synth_block.sample {
                                                        if let Some(buf_id) = buffer_mgr.get(sample_path) {
                                                            client.new_synth_with_args(name, name, &[("bufnum", buf_id as f32)]).await
                                                        } else {
                                                            client.new_synth(name, name).await
                                                        }
                                                    } else {
                                                        client.new_synth(name, name).await
                                                    };
                                                    match swap_result {
                                                        Ok(node_id) => {
                                                            state.actual.nodes.insert(name.clone(), node_id);
                                                            tracing::info!("'{}': hot-swapped node after IR change", name);
                                                        }
                                                        Err(e) => tracing::error!("'{}': hot-swap new_synth failed: {}", name, e),
                                                    }
                                                }
                                            }
                                            Err(e) => tracing::error!("'{}': IR load failed on hot-swap: {}", name, e),
                                        }
                                    }
                                    Err(e) => tracing::error!("'{}': IR recompilation failed: {}", name, e),
                                }
                            }
                        }

                        // Hot-swap sequencers: abort old, spawn new for changed things
                        for (name, thing) in piece.iter() {
                            if let Some(synth_block) = &thing.synth {
                                if let (Some(notes), Some(tempo_str)) = (&synth_block.notes, &synth_block.tempo) {
                                    if let Some(tempo) = NoteSequencer::parse_tempo(tempo_str) {
                                        // Abort old sequencer if present
                                        if let Some(old_handle) = sequencer_handles.remove(name) {
                                            old_handle.abort();
                                        }
                                        // Spawn new sequencer if thing is running
                                        if state.actual.nodes.contains_key(name.as_str()) {
                                            let seq = NoteSequencer {
                                                thing_name: name.clone(),
                                                notes: notes.clone(),
                                                tempo,
                                            };
                                            let handle = seq.spawn(seq_tx.clone());
                                            sequencer_handles.insert(name.clone(), handle);
                                            tracing::info!("'{}': hot-swapped sequencer", name);
                                        }
                                    }
                                }
                            }
                        }

                        // Inject pipe-expanded nodes into piece for reconciler
                        inject_pipe_nodes_into_piece(&mut piece, &pipe_expanded);

                        state.desired = Some(piece);
                        reconcile_now(state, client, seq_tx, sequencer_handles, stage_store, buffer_mgr).await;
                    }
                    Err(e) => {
                        tracing::error!("parse error on piece.hum reload: {e}");
                    }
                },
                Err(e) => {
                    tracing::error!("could not read piece.hum: {e}");
                }
            }
        }
        "scd" => {
            handle_scd_change(state, client, path).await;
        }
        "wav" | "aif" | "aiff" => {
            // Sample file changed: reload buffer if it's a tracked sample
            if let Some(sample_key) = buffer_mgr.sample_for_path(path) {
                if let Some(buf_id) = buffer_mgr.get(&sample_key) {
                    let abs_path = buffer_mgr.resolve_path(&sample_key);
                    // Free old buffer, then re-load from disk
                    let _ = client.free_buffer(buf_id).await;
                    match client.load_buffer(buf_id, &abs_path.to_string_lossy()).await {
                        Ok(()) => {
                            tracing::info!("sample hot-reload: '{}' -> buffer {}", sample_key, buf_id);
                            println!("hum-rt: sample reloaded '{}'", sample_key);
                        }
                        Err(e) => tracing::error!("sample hot-reload failed for '{}': {}", sample_key, e),
                    }
                }
            } else {
                tracing::debug!("ignoring untracked sample file change: {:?}", path);
            }
        }
        _ => {
            tracing::debug!("ignoring file change: {:?}", path);
        }
    }
}

/// Handle .scd file change: read from disk, load SynthDef, hot-swap if running.
async fn handle_scd_change(
    state: &mut StateStore,
    client: &mut ScsynthClient,
    changed_path: &Path,
) {
    let Some(stem) = changed_path.file_stem().and_then(|s| s.to_str()) else {
        return;
    };
    tracing::info!("scd changed: {:?} (stem: {})", changed_path, stem);

    let Ok(bytes) = std::fs::read(changed_path) else {
        tracing::error!("could not read scd file: {:?}", changed_path);
        return;
    };

    if let Err(e) = client.load_synthdef(bytes).await {
        tracing::error!("failed to load synthdef {}: {}", stem, e);
        return;
    }
    tracing::info!("reloaded SynthDef for '{}'", stem);

    if state.actual.nodes.contains_key(stem) {
        match client.new_synth(stem, stem).await {
            Ok(node_id) => {
                state.actual.nodes.insert(stem.to_string(), node_id);
                tracing::info!("hot-swapped synthdef for '{}'", stem);
            }
            Err(e) => tracing::error!("hot-swap new_synth failed for '{}': {}", stem, e),
        }
    }
}

// ---------------------------------------------------------------------------
// Tick handler with playing guard and loop wrapping
// ---------------------------------------------------------------------------

/// Handle a timeline tick: update playback position and reconcile if active set changed.
/// Returns early if not playing. Handles loop wrapping.
async fn handle_tick(
    state: &mut StateStore,
    client: &mut ScsynthClient,
    pos: f64,
    ticker_handle: &mut Option<JoinHandle<()>>,
    tx: &tokio::sync::mpsc::Sender<DaemonEvent>,
    seq_tx: &tokio_mpsc::Sender<SequencerEvent>,
    sequencer_handles: &mut HashMap<String, JoinHandle<()>>,
    stage_store: &StageStore,
    buffer_mgr: &BufferManager,
) {
    // Gate: only process ticks when playing
    if !state.playing {
        return;
    }

    // Loop wrapping: if loop_range is set and pos >= end, restart ticker from start
    if let Some((loop_start, loop_end)) = state.loop_range {
        if pos >= loop_end {
            state.playback_pos = loop_start;
            restart_ticker(ticker_handle, tx, loop_start);
            reconcile_now(state, client, seq_tx, sequencer_handles, stage_store, buffer_mgr).await;
            tracing::info!("loop wrap: {:.2}s -> {:.2}s", loop_end, loop_start);
            return;
        }
    }

    // Compute old active key set before updating position
    let old_active_keys: Vec<String> = state
        .active_things_filtered(state.playback_pos)
        .into_keys()
        .collect();

    // Update playback position
    state.playback_pos = pos;

    // Compute new active key set
    let new_active = state.active_things_filtered(pos);
    let new_active_keys: Vec<String> = new_active.keys().cloned().collect();

    // Only reconcile if the active set actually changed (prevents 20 OSC calls/sec)
    if old_active_keys != new_active_keys {
        tracing::info!(
            "active set changed at pos {:.2}s: {:?} -> {:?}",
            pos,
            old_active_keys,
            new_active_keys
        );
        let ops = reconciler::diff(&new_active, &state.actual);
        apply_ops(state, client, ops, seq_tx, sequencer_handles, stage_store, buffer_mgr).await;
    }
}

/// Compute active things from current state and reconcile against actual.
async fn reconcile_now(
    state: &mut StateStore,
    client: &mut ScsynthClient,
    seq_tx: &tokio_mpsc::Sender<SequencerEvent>,
    sequencer_handles: &mut HashMap<String, JoinHandle<()>>,
    stage_store: &StageStore,
    buffer_mgr: &BufferManager,
) {
    let active = state.active_things_filtered(state.playback_pos);
    let ops = reconciler::diff(&active, &state.actual);
    if !ops.is_empty() {
        tracing::info!("reconciling {} ops", ops.len());
    }
    apply_ops(state, client, ops, seq_tx, sequencer_handles, stage_store, buffer_mgr).await;
}

/// Apply reconciliation operations to scsynth and update actual state.
/// Also manages note sequencer lifecycle: spawn on Add, abort on Remove.
/// Stage-aware: if a thing is in a stage's applies-to list, spawn it in the
/// stage's group instead of the default group.
/// Buffer-aware: if a thing has sample:, passes bufnum arg at /s_new time.
async fn apply_ops(
    state: &mut StateStore,
    client: &mut ScsynthClient,
    ops: Vec<ReconcileOp>,
    seq_tx: &tokio_mpsc::Sender<SequencerEvent>,
    sequencer_handles: &mut HashMap<String, JoinHandle<()>>,
    stage_store: &StageStore,
    buffer_mgr: &BufferManager,
) {
    for op in ops {
        match op {
            ReconcileOp::Add {
                thing_name,
                synthdef_name,
            } => {
                // Determine if this thing needs a bufnum argument
                let bufnum_arg = state.desired.as_ref().and_then(|piece| {
                    piece.get(&thing_name).and_then(|td| {
                        td.synth.as_ref().and_then(|sb| {
                            sb.sample.as_ref().and_then(|sp| {
                                buffer_mgr.get(sp).map(|id| id as f32)
                            })
                        })
                    })
                });

                // Check if this thing belongs to a stage group
                let result = if let Some(group_id) = stage_store.group_for_thing(&thing_name) {
                    tracing::info!(
                        "reconcile: Add '{}' into stage group {}",
                        thing_name, group_id
                    );
                    client
                        .start_synth_in_group(&thing_name, &synthdef_name, group_id)
                        .await
                } else if let Some(buf_id) = bufnum_arg {
                    // Sample-based synth: pass bufnum at /s_new time
                    client.new_synth_with_args(&thing_name, &synthdef_name, &[("bufnum", buf_id)]).await
                } else {
                    client.new_synth(&thing_name, &synthdef_name).await
                };
                match result {
                Ok(node_id) => {
                    tracing::info!("reconcile: Add '{}' -> node {}", thing_name, node_id);
                    state.actual.nodes.insert(thing_name.clone(), node_id);

                    // Spawn note sequencer if thing has notes + tempo
                    if let Some(piece) = &state.desired {
                        if let Some(thing_def) = piece.get(&thing_name) {
                            if let Some(synth_block) = &thing_def.synth {
                                if let (Some(notes), Some(tempo_str)) = (&synth_block.notes, &synth_block.tempo) {
                                    if let Some(tempo) = NoteSequencer::parse_tempo(tempo_str) {
                                        let seq = NoteSequencer {
                                            thing_name: thing_name.clone(),
                                            notes: notes.clone(),
                                            tempo,
                                        };
                                        let handle = seq.spawn(seq_tx.clone());
                                        sequencer_handles.insert(thing_name.clone(), handle);
                                        tracing::info!("'{}': spawned note sequencer ({} notes, {:.2}s/note)", thing_name, notes.len(), tempo);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => tracing::error!("synth spawn failed for '{}': {}", thing_name, e),
                }
            }
            ReconcileOp::Remove { thing_name } => {
                // Abort sequencer if running
                if let Some(handle) = sequencer_handles.remove(&thing_name) {
                    handle.abort();
                    tracing::info!("'{}': aborted note sequencer", thing_name);
                }
                if let Err(e) = client.free_node(&thing_name).await {
                    tracing::error!("free_node failed for '{}': {}", thing_name, e);
                }
                state.actual.nodes.shift_remove(&thing_name);
                tracing::info!("reconcile: Remove '{}'", thing_name);
            }
            ReconcileOp::Swap {
                thing_name,
                new_synthdef_name,
            } => match client.new_synth(&thing_name, &new_synthdef_name).await {
                Ok(node_id) => {
                    tracing::info!("reconcile: Swap '{}' -> node {}", thing_name, node_id);
                    state.actual.nodes.insert(thing_name, node_id);
                }
                Err(e) => tracing::error!("swap new_synth failed for '{}': {}", thing_name, e),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Instrument merge helper
// ---------------------------------------------------------------------------

/// Resolve a thing's synth block by merging with its instrument or dict base.
/// Priority (highest to lowest):
///   .scd escape hatch (checked before this fn is called)
///   instrument: merge (thing.synth overrides instrument base)
///   style: dict lookup (thing.synth overrides dict entry base)
///   bare synth: block
fn resolve_synth_block(
    thing: &parser::ThingDef,
    store: &InstrumentStore,
    dict_store: &DictStore,
) -> Option<ir::types::SynthBlock> {
    match &thing.instrument {
        Some(inst_name) => {
            match store.get(inst_name) {
                Some(base) => {
                    // Merge: thing's synth fields override instrument base
                    let over = thing.synth.as_ref();
                    match over {
                        Some(over_block) => Some(InstrumentStore::merge(base, over_block)),
                        None => {
                            // No synth: on thing, use instrument base as-is
                            Some(base.clone())
                        }
                    }
                }
                None => {
                    tracing::warn!(
                        "instrument '{}' not found in store, using thing's synth as-is",
                        inst_name
                    );
                    thing.synth.clone()
                }
            }
        }
        None => {
            // No instrument: — check style: for dict lookup
            match &thing.style {
                Some(term) => {
                    match dict_store.get(term) {
                        Some(entry) => {
                            // Dict entry is base, thing's synth: overrides
                            match &thing.synth {
                                Some(over) => Some(InstrumentStore::merge(&entry.synth, over)),
                                None => Some(entry.synth.clone()),
                            }
                        }
                        None => {
                            tracing::warn!("dict term '{}' not found, using thing's synth as-is", term);
                            thing.synth.clone()
                        }
                    }
                }
                None => thing.synth.clone(),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Like: change detection helper (SYNC-01)
// ---------------------------------------------------------------------------

/// Detect like: field changes between old and new piece state.
/// Returns a list of thing names whose like: value changed.
/// Only fires when the old value was non-empty (first parse is not a "change").
fn detect_like_changes(
    like_hashes: &HashMap<String, String>,
    piece: &parser::Piece,
) -> Vec<String> {
    let mut changed = Vec::new();
    for (thing_name, thing) in piece.iter() {
        let new_like = thing.like.as_deref().unwrap_or("").to_string();
        let old_like = like_hashes.get(thing_name).cloned().unwrap_or_default();
        if !old_like.is_empty() && new_like != old_like {
            changed.push(thing_name.clone());
        }
    }
    changed
}

/// Update like_hashes with current piece state.
fn update_like_hashes(like_hashes: &mut HashMap<String, String>, piece: &parser::Piece) {
    for (thing_name, thing) in piece.iter() {
        let new_like = thing.like.as_deref().unwrap_or("").to_string();
        like_hashes.insert(thing_name.clone(), new_like);
    }
}

// ---------------------------------------------------------------------------
// Dict suggest helper (SYNC-05)
// ---------------------------------------------------------------------------

/// A suggestion for a potential dict entry based on recurring synth patterns.
struct DictSuggestion {
    description: String,
    count: usize,
    thing_names: Vec<String>,
}

/// Scan all ThingDef.synth blocks in a piece, group by shape key (osc|filter|fx),
/// and return suggestions for groups with 2+ things sharing the same shape.
fn compute_dict_suggestions(piece: &parser::Piece) -> Vec<DictSuggestion> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for (name, thing) in piece.iter() {
        if let Some(synth) = &thing.synth {
            let key = format!("{:?}|{:?}|{:?}", synth.osc, synth.filter, synth.fx);
            groups.entry(key).or_default().push(name.clone());
        }
    }

    let mut suggestions = Vec::new();
    let mut sorted_groups: Vec<_> = groups.into_iter().collect();
    sorted_groups.sort_by_key(|(_, names)| std::cmp::Reverse(names.len()));

    for (key, mut names) in sorted_groups {
        if names.len() >= 2 {
            // Build readable description from the key parts
            let parts: Vec<&str> = key.split('|').collect();
            let readable: Vec<String> = parts
                .iter()
                .filter(|p| **p != "None")
                .map(|p| {
                    p.replace("Some(", "")
                        .replace(')', "")
                        .to_lowercase()
                })
                .collect();
            let desc = if readable.is_empty() {
                "same shape".to_string()
            } else {
                readable.join(" + ")
            };
            names.sort();
            suggestions.push(DictSuggestion {
                description: desc,
                count: names.len(),
                thing_names: names,
            });
        }
    }
    suggestions
}

/// Print dict suggestions to stdout.
fn suggest_dict_entries(piece: &parser::Piece) {
    let suggestions = compute_dict_suggestions(piece);
    if suggestions.is_empty() {
        println!("(no recurring patterns found — all things have distinct synth shapes)");
        return;
    }
    for s in &suggestions {
        println!(
            "suggest: {} things share {} — consider adding a term to hum.dict",
            s.count, s.description
        );
        println!("  things: {}", s.thing_names.join(", "));
    }
}

// ---------------------------------------------------------------------------
// Pipe expansion helper
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Divergence detection
// ---------------------------------------------------------------------------

/// Detect synth: blocks that have been manually edited to diverge from pipe: output.
/// Returns the file content with divergence comments inserted where needed.
///
/// Logic: for each thing with both pipe: and synth:, compare the current synth repr
/// against the old hash. If the hash changed (not first load), it's a manual edit
/// — insert a comment above the synth: line in that thing's block.
fn detect_divergences(
    file_content: &str,
    piece: &parser::Piece,
    old_synth_hashes: &HashMap<String, String>,
) -> String {
    let mut diverged_things: Vec<String> = Vec::new();

    for (name, thing) in piece.iter() {
        // Only care about things with BOTH pipe: and synth:
        if thing.pipe.is_none() || thing.synth.is_none() {
            continue;
        }

        let new_repr = format!("{:?}", thing.synth.as_ref().unwrap());
        let old_repr = old_synth_hashes.get(name).cloned().unwrap_or_default();

        // Skip first load (old_repr is empty)
        if old_repr.is_empty() {
            continue;
        }

        // If synth changed since last load, it's a manual edit
        if new_repr != old_repr {
            diverged_things.push(name.clone());
        }
    }

    if diverged_things.is_empty() {
        return file_content.to_string();
    }

    // Insert comments into file content using line-scan approach
    let mut result = String::new();
    let mut current_thing: Option<String> = None;
    let comment_line = "  # synth: manually tuned, pipe: may be stale";

    for line in file_content.lines() {
        // Detect top-level thing key: line that doesn't start with space and ends with ':'
        if !line.starts_with(' ') && !line.starts_with('#') && line.trim_end().ends_with(':') {
            let key = line.trim_end().trim_end_matches(':');
            current_thing = Some(key.to_string());
        }

        // Check if this is the synth: line in a diverged thing's block
        if let Some(ref thing_name) = current_thing {
            if diverged_things.contains(thing_name) {
                let trimmed = line.trim();
                if trimmed == "synth:" || trimmed.starts_with("synth:") {
                    // Check if previous line is already the comment
                    let already_present = result.lines().last()
                        .map(|l| l.trim() == "# synth: manually tuned, pipe: may be stale")
                        .unwrap_or(false);

                    if !already_present {
                        result.push_str(comment_line);
                        result.push('\n');
                    }
                }
            }
        }

        result.push_str(line);
        result.push('\n');
    }

    result
}

/// Expand all pipe: things in a piece into synthetic (name, SynthBlock) pairs.
/// Also injects the expanded nodes as ThingDefs into the piece so the reconciler
/// sees them as normal active things.
///
/// Returns the expanded pairs for IR compilation.
/// Mutates `piece` by removing the pipe thing and adding synthetic things.
fn expand_pipe_things(piece: &parser::Piece) -> Vec<(String, ir::types::SynthBlock)> {
    let mut expanded = Vec::new();

    // Collect pipe things first to avoid borrow issues
    let pipe_things: Vec<(String, String)> = piece
        .iter()
        .filter_map(|(name, thing)| {
            thing.pipe.as_ref().map(|p| (name.clone(), p.clone()))
        })
        .collect();

    for (thing_name, pipe_str) in pipe_things {
        // Check if source references another pipe thing (not supported)
        match parse_pipe_block(&pipe_str) {
            Ok(expr) => {
                let source_name = match &expr.source {
                    pipe::types::PipeSource::Thing(n) => n.as_str(),
                    pipe::types::PipeSource::Field(n, _) => n.as_str(),
                };
                if let Some(source_thing) = piece.get(source_name) {
                    if source_thing.pipe.is_some() {
                        tracing::warn!(
                            "nested pipe source '{}' not supported, skipping '{}'",
                            source_name, thing_name
                        );
                        continue;
                    }
                }

                match expand_pipe(&thing_name, &expr, piece) {
                    Ok(nodes) => {
                        tracing::info!(
                            "'{}': pipe expanded into {} nodes",
                            thing_name, nodes.len()
                        );
                        expanded.extend(nodes);
                    }
                    Err(e) => {
                        tracing::error!("pipe expansion failed for '{}': {}", thing_name, e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("pipe parse failed for '{}': {}", thing_name, e);
            }
        }
    }

    expanded
}

/// Inject pipe-expanded nodes into the piece as synthetic ThingDefs.
/// This allows the reconciler to see them as active things.
/// Also removes the original pipe thing from the active set (it's replaced by its children).
fn inject_pipe_nodes_into_piece(
    piece: &mut parser::Piece,
    expanded: &[(String, ir::types::SynthBlock)],
) {
    // Remove original pipe things from piece (they are replaced by expanded nodes)
    let pipe_thing_names: Vec<String> = piece
        .iter()
        .filter_map(|(name, thing)| {
            if thing.pipe.is_some() { Some(name.clone()) } else { None }
        })
        .collect();
    for name in &pipe_thing_names {
        piece.shift_remove(name);
    }

    // Insert synthetic ThingDefs for each expanded node
    for (node_name, synth_block) in expanded {
        let synthetic_thing = parser::ThingDef {
            at: None,     // Inherit timing from piece context
            until: None,
            does: None,
            location: None,
            has: None,
            within: None,
            every: None,
            like: None,
            reference: None,
            mood: None,
            synth: Some(synth_block.clone()),
            thing_type: None,
            instrument: None,
            style: None,
            applies_to: None,
            fx: None,
            pipe: None,
        };
        piece.insert(node_name.clone(), synthetic_thing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::types::{OscPrimitive, OscLayer, SynthBlock, Value};

    fn empty_synth() -> SynthBlock {
        SynthBlock {
            notes: None, osc: None, filter: None, env: None,
            distort: None, fx: None, pan: None, amp: None, tempo: None,
            sample: None, loop_mode: None,
        }
    }

    fn make_thing() -> parser::ThingDef {
        parser::ThingDef {
            at: None, until: None, does: None, location: None,
            has: None, within: None, every: None, like: None,
            reference: None, mood: None, synth: None, thing_type: None,
            instrument: None, style: None, applies_to: None, fx: None,
            pipe: None,
        }
    }

    fn dict_with_laser() -> DictStore {
        let dir = tempfile::tempdir().unwrap();
        let dict_path = dir.path().join("hum.dict");
        std::fs::write(&dict_path, "laser:\n  synth:\n    osc: sine\n    amp: 0.8\n").unwrap();
        let store = DictStore::load(&dict_path).unwrap();
        // Keep tempdir alive by leaking (test-only)
        std::mem::forget(dir);
        store
    }

    // --- Divergence detection ---

    #[test]
    fn divergence_comment_inserted_when_synth_differs_from_pipe() {
        // A thing with both pipe: and synth:, where synth has been manually edited
        // (differs from what pipe would produce)
        let file_content = r#"glass:
  synth:
    osc: sine
    amp: 0.3
    notes: [D4, Eb4]
  pipe: "glass |> replicate(3)"
"#;
        let old_hashes: HashMap<String, String> = {
            let mut m = HashMap::new();
            // Simulate a previous synth hash that differs from current
            m.insert("glass".to_string(), "OLD_DIFFERENT_HASH".to_string());
            m
        };

        // Build a piece for pipe expansion comparison
        let piece = parser::parse_hum(file_content).unwrap();
        let result = detect_divergences(file_content, &piece, &old_hashes);
        // The comment should be inserted above the synth: line in glass's block
        assert!(
            result.contains("# synth: manually tuned, pipe: may be stale"),
            "Expected divergence comment in output, got:\n{}",
            result
        );
    }

    #[test]
    fn divergence_comment_not_duplicated_if_already_present() {
        let file_content = r#"glass:
  # synth: manually tuned, pipe: may be stale
  synth:
    osc: sine
    amp: 0.3
    notes: [D4, Eb4]
  pipe: "glass |> replicate(3)"
"#;
        let old_hashes: HashMap<String, String> = {
            let mut m = HashMap::new();
            m.insert("glass".to_string(), "OLD_DIFFERENT_HASH".to_string());
            m
        };

        let piece = parser::parse_hum(file_content).unwrap();
        let result = detect_divergences(file_content, &piece, &old_hashes);
        // Should only have ONE instance of the comment
        let count = result.matches("# synth: manually tuned, pipe: may be stale").count();
        assert_eq!(count, 1, "Expected exactly 1 divergence comment, got {}", count);
    }

    #[test]
    fn no_divergence_comment_when_synth_matches_pipe_output() {
        // When synth: matches what pipe expansion would produce, no comment needed.
        // Since we can't easily run expand_pipe in a unit test context without
        // the pipe source existing, we test via hash: if old_hash is empty (first load),
        // no divergence is detected.
        let file_content = r#"glass:
  synth:
    osc: sine
  pipe: "glass |> replicate(3)"
"#;
        let old_hashes: HashMap<String, String> = HashMap::new(); // empty = first load

        let piece = parser::parse_hum(file_content).unwrap();
        let result = detect_divergences(file_content, &piece, &old_hashes);
        assert!(
            !result.contains("# synth: manually tuned, pipe: may be stale"),
            "Should not insert comment on first load, got:\n{}",
            result
        );
    }

    // --- Like: change detection (SYNC-01) ---

    #[test]
    fn like_change_detected_when_value_differs() {
        let mut like_hashes: HashMap<String, String> = HashMap::new();
        // Simulate a previous parse where glass had like: "bright laser"
        like_hashes.insert("glass".to_string(), "bright laser".to_string());

        // New piece has glass with different like:
        let mut piece = indexmap::IndexMap::new();
        let mut thing = make_thing();
        thing.like = Some("warm pad".to_string());
        piece.insert("glass".to_string(), thing);

        let changed = detect_like_changes(&like_hashes, &piece);
        assert_eq!(changed, vec!["glass".to_string()]);
    }

    #[test]
    fn like_change_not_fired_on_first_load() {
        let like_hashes: HashMap<String, String> = HashMap::new(); // empty = first load

        let mut piece = indexmap::IndexMap::new();
        let mut thing = make_thing();
        thing.like = Some("bright laser".to_string());
        piece.insert("glass".to_string(), thing);

        let changed = detect_like_changes(&like_hashes, &piece);
        assert!(changed.is_empty(), "should not fire on first load");
    }

    #[test]
    fn like_change_not_fired_when_same() {
        let mut like_hashes: HashMap<String, String> = HashMap::new();
        like_hashes.insert("glass".to_string(), "bright laser".to_string());

        let mut piece = indexmap::IndexMap::new();
        let mut thing = make_thing();
        thing.like = Some("bright laser".to_string());
        piece.insert("glass".to_string(), thing);

        let changed = detect_like_changes(&like_hashes, &piece);
        assert!(changed.is_empty(), "should not fire when like: unchanged");
    }

    // --- Dict suggest (SYNC-05) ---

    #[test]
    fn suggest_detects_recurring_synth_pattern() {
        let mut piece = indexmap::IndexMap::new();
        // 3 things with identical osc: sine
        for name in &["a", "b", "c"] {
            let mut thing = make_thing();
            let mut synth = empty_synth();
            synth.osc = Some(OscLayer(vec![OscPrimitive::Sine { freq: None }]));
            thing.synth = Some(synth);
            piece.insert(name.to_string(), thing);
        }
        let suggestions = compute_dict_suggestions(&piece);
        assert_eq!(suggestions.len(), 1, "should find 1 recurring pattern");
        assert_eq!(suggestions[0].count, 3);
        assert!(suggestions[0].thing_names.contains(&"a".to_string()));
        assert!(suggestions[0].thing_names.contains(&"b".to_string()));
        assert!(suggestions[0].thing_names.contains(&"c".to_string()));
    }

    #[test]
    fn suggest_no_pattern_when_all_distinct() {
        let mut piece = indexmap::IndexMap::new();
        let mut t1 = make_thing();
        let mut s1 = empty_synth();
        s1.osc = Some(OscLayer(vec![OscPrimitive::Sine { freq: None }]));
        t1.synth = Some(s1);
        piece.insert("a".to_string(), t1);

        let mut t2 = make_thing();
        let mut s2 = empty_synth();
        s2.osc = Some(OscLayer(vec![OscPrimitive::Saw { detune: None }]));
        t2.synth = Some(s2);
        piece.insert("b".to_string(), t2);

        let suggestions = compute_dict_suggestions(&piece);
        assert!(suggestions.is_empty(), "should find no patterns when all distinct");
    }

    #[test]
    fn resolve_style_no_synth_uses_dict_entry() {
        let inst_store = InstrumentStore::default();
        let dict_store = dict_with_laser();

        let mut thing = make_thing();
        thing.style = Some("laser".to_string());
        // No synth: block on the thing

        let resolved = resolve_synth_block(&thing, &inst_store, &dict_store);
        assert!(resolved.is_some());
        let sb = resolved.unwrap();
        assert_eq!(sb.osc, Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])));
        assert_eq!(sb.amp, Some(Value::Fixed(0.8)));
    }

    #[test]
    fn resolve_style_with_synth_override() {
        let inst_store = InstrumentStore::default();
        let dict_store = dict_with_laser();

        let mut thing = make_thing();
        thing.style = Some("laser".to_string());
        let mut over = empty_synth();
        over.amp = Some(Value::Fixed(0.5));
        thing.synth = Some(over);

        let resolved = resolve_synth_block(&thing, &inst_store, &dict_store);
        assert!(resolved.is_some());
        let sb = resolved.unwrap();
        // Dict's osc preserved, thing's amp overrides
        assert_eq!(sb.osc, Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])));
        assert_eq!(sb.amp, Some(Value::Fixed(0.5)));
    }

    #[test]
    fn resolve_instrument_wins_over_style() {
        // Create an instrument store with "foo"
        let dir = tempfile::tempdir().unwrap();
        let inst_path = dir.path().join("foo.hum");
        std::fs::write(&inst_path, "type: instrument\nsynth:\n  osc: saw\n  amp: 0.9\n").unwrap();
        let inst_store = InstrumentStore::load_dir(dir.path()).unwrap();
        let dict_store = dict_with_laser();

        let mut thing = make_thing();
        thing.instrument = Some("foo".to_string());
        thing.style = Some("laser".to_string()); // should be ignored

        let resolved = resolve_synth_block(&thing, &inst_store, &dict_store);
        assert!(resolved.is_some());
        let sb = resolved.unwrap();
        // Instrument "foo" (saw) wins, not dict "laser" (sine)
        assert_eq!(sb.osc, Some(OscLayer(vec![OscPrimitive::Saw { detune: None }])));
        assert_eq!(sb.amp, Some(Value::Fixed(0.9)));
    }
}

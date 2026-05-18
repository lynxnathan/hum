---
phase: 03-state-reconciler-watcher
plan: 3
type: execute
wave: 3
depends_on:
  - 03-PLAN-1
  - 03-PLAN-2
files_modified:
  - src/main.rs
autonomous: false
requirements:
  - WATCH-01
  - WATCH-02
  - WATCH-03
  - WATCH-04
  - TIME-01
  - TIME-02
  - TIME-03
must_haves:
  truths:
    - "Saving piece.hum while hum-rt runs triggers a diff and sends only changed OSC messages"
    - "Things inactive at current pos do not get new_synth called; active things do"
    - "Saving a .scd file for an active thing causes load_synthdef + new_synth (hot-swap)"
    - "Things with no until: remain playing until piece.hum removes them"
    - "hum-rt starts, connects to scsynth, loads SynthDefs, enters event loop — no crash"
  artifacts:
    - path: "src/main.rs"
      provides: "Tokio event loop: mpsc channel, watcher spawn, ticker spawn, reconciler calls"
      contains: "DaemonEvent::FileChanged, DaemonEvent::Tick, tokio::select!, StateStore"
  key_links:
    - from: "src/main.rs"
      to: "src/reconciler.rs"
      via: "diff(&active, &state.actual) called on FileChanged and Tick events"
      pattern: "reconciler::diff"
    - from: "src/main.rs"
      to: "src/osc/bridge.rs"
      via: "apply_ops calls client.new_synth / client.free_node"
      pattern: "client\\.new_synth|client\\.free_node"
    - from: "src/main.rs"
      to: "src/watcher.rs"
      via: "start_watcher called with [piece_hum_path, scd_dir_path] and tx clone"
      pattern: "watcher::start_watcher"
---

<objective>
Wire all Phase 3 modules into main.rs: replace the smoke test loop with a real tokio event loop that watches files, ticks the timeline, and reconciles OSC state on every change.

Purpose: This is the completion of the core feedback loop — editing piece.hum changes what scsynth plays within ~1 second.
Output: A working hum-rt daemon that reacts to file saves in real time.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/phases/03-state-reconciler-watcher/03-CONTEXT.md
@.planning/phases/03-state-reconciler-watcher/research/RESEARCH.md
@.planning/phases/03-state-reconciler-watcher/03-1-SUMMARY.md
@.planning/phases/03-state-reconciler-watcher/03-2-SUMMARY.md

<interfaces>
<!-- All modules from Plans 1 and 2 — use these exactly. -->

From src/events.rs:
```rust
pub enum DaemonEvent {
    FileChanged(std::path::PathBuf),
    Tick(f64),
}
```

From src/state.rs:
```rust
pub struct StateStore { pub desired: Option<Piece>, pub actual: ActualState, pub playback_pos: f64 }
pub struct ActualState { pub nodes: IndexMap<String, i32> }
impl StateStore {
    pub fn new() -> Self;
    pub fn active_things(&self, pos: f64) -> IndexMap<String, &ThingDef>;
}
pub fn parse_seconds(s: &str) -> Option<f64>;
```

From src/reconciler.rs:
```rust
pub enum ReconcileOp { Add { thing_name, synthdef_name }, Remove { thing_name }, Swap { thing_name, new_synthdef_name } }
pub fn diff(active: &IndexMap<String, &ThingDef>, actual: &ActualState) -> Vec<ReconcileOp>;
```

From src/watcher.rs:
```rust
pub fn start_watcher(paths: &[PathBuf], tx: tokio::sync::mpsc::Sender<DaemonEvent>) -> anyhow::Result<()>;
```

From src/timeline.rs:
```rust
pub async fn run_ticker(tx: tokio::sync::mpsc::Sender<DaemonEvent>, start_pos: f64);
```

From src/osc/bridge.rs (already in project):
```rust
pub async fn load_synthdef(&mut self, synthdef_bytes: Vec<u8>) -> Result<()>
pub async fn new_synth(&mut self, thing_name: &str, synthdef_name: &str) -> Result<i32>
pub async fn free_node(&mut self, thing_name: &str) -> Result<()>
pub async fn free_all_nodes(&mut self) -> Result<()>
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Replace smoke test with real event loop in main.rs</name>
  <files>src/main.rs</files>
  <action>
    Replace run_smoke_test and the existing tokio::select! block with a full event loop.
    Remove the SINE_SCSYNDEF const and run_smoke_test function entirely.
    Add module declarations: `mod events; mod state; mod reconciler; mod watcher; mod timeline;`

    Startup sequence (keep existing config load, scsynth connect, check_alive, scd_store load):
    1. Parse piece.hum into state.desired (use existing parser::parse_hum call, store in StateStore)
    2. Create mpsc channel: `let (tx, mut rx) = tokio::sync::mpsc::channel::<DaemonEvent>(64);`
    3. Watch paths: `watcher::start_watcher(&[piece_hum_path, scd_dir_path], tx.clone())?;`
       - piece_hum_path = PathBuf::from("piece.hum")
       - scd_dir_path = PathBuf::from("out/sc")
    4. Spawn timeline: `tokio::spawn(timeline::run_ticker(tx.clone(), 0.0));`
    5. Enter event loop (tokio::select!):

    ```rust
    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                match event {
                    DaemonEvent::FileChanged(path) => {
                        handle_file_change(&mut state, &mut client, &path).await;
                    }
                    DaemonEvent::Tick(pos) => {
                        handle_tick(&mut state, &mut client, pos).await;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("shutting down — freeing all nodes");
                client.free_all_nodes().await?;
                break;
            }
        }
    }
    ```

    Implement handle_file_change (in main.rs as async fn):
    - If path ends with ".hum": re-parse piece.hum from disk, update state.desired on Ok (log error on Err, do not clear desired), then call reconcile_now(&mut state, &mut client).await
    - If path ends with ".scd": call handle_scd_change (Pattern 6 from RESEARCH.md):
      1. Read file from disk directly (NOT from scd_store — store is load-once)
      2. call client.load_synthdef(bytes).await — awaits /synced
      3. If thing name (file stem) is in state.actual.nodes: call client.new_synth(stem, stem).await, update state.actual.nodes

    Implement handle_tick (in main.rs as async fn):
    - Compute old active key set from state.active_things(state.playback_pos)
    - Update state.playback_pos = pos
    - Compute new active set
    - If key sets differ: call reconcile_now
    - If key sets identical: no-op (do NOT call reconcile on every tick)

    Implement reconcile_now (in main.rs as async fn):
    - active = state.active_things(state.playback_pos)
    - ops = reconciler::diff(&active, &state.actual)
    - apply_ops(&mut state, &mut client, ops).await (Pattern 7 from RESEARCH.md)

    Implement apply_ops following Pattern 7 exactly (Add -> new_synth, Remove -> free_node).

    CRITICAL invariants:
    - state and client are owned by the event loop — zero Arc/Mutex
    - Do not free all nodes on .hum reload — only diff-based ops
    - Tick is a no-op when active set unchanged (prevents 20 OSC calls/sec)
    - On .scd change, read from disk, not scd_store
  </action>
  <verify>cargo build 2>&1 | grep -E "^error" | wc -l  # must be 0</verify>
  <done>
    - `cargo build` succeeds
    - main.rs no longer contains run_smoke_test or SINE_SCSYNDEF
    - All six new modules declared and wired
    - Event loop handles FileChanged(.hum), FileChanged(.scd), Tick, and Ctrl-C
  </done>
</task>

<task type="checkpoint:human-verify" gate="blocking">
  <what-built>
    Full Phase 3 daemon: file watcher + timeline ticker + reconciler wired into hum-rt.
    The daemon watches piece.hum and out/sc/, ticks playback position at 50ms, and sends minimal OSC deltas on file saves.
  </what-built>
  <how-to-verify>
    Prerequisite: scsynth running on Windows (or localhost:57110). Set SCSYNTH_HOST if needed.

    Test 1 — Startup and idle:
    1. Create a minimal piece.hum:
       ```yaml
       drone:
         at: "0s"
         like: a low sustained tone
       ```
    2. Run: `RUST_LOG=info cargo run`
    3. Expected: "parsed 1 things from piece.hum", connects to scsynth, enters event loop, no crash
    4. Expected: Tick events visible in logs at ~50ms (or every few seconds if /mnt/ path)

    Test 2 — .hum file save triggers diff (no scsynth required for diff logic):
    1. While daemon is running, add a second thing to piece.hum and save
    2. Expected log: file change detected, parse succeeded, reconciler Add op for the new thing
    3. Expected: No Remove for the existing thing (unchanged things are NOT restarted)

    Test 3 — Until time deactivation:
    1. Edit piece.hum to add: `until: "5s"`  to the drone thing
    2. Save and wait 5 seconds of playback
    3. Expected log: Tick triggered reconcile, Remove op for drone, free_node called

    Test 4 — /mnt/ path detection (WSL2 only):
    1. Check startup logs for "using PollWatcher" if piece.hum is under /mnt/
    2. Expected: PollWatcher warning present; inotify warning absent for native paths

    Type "approved" to complete Phase 3, or describe any issues seen.
  </how-to-verify>
  <resume-signal>Type "approved" or describe issues found</resume-signal>
</task>

</tasks>

<verification>
```
cargo build
cargo test
```
Build clean, all unit tests still green, daemon starts without crashing.
</verification>

<success_criteria>
1. `cargo build` succeeds with zero errors
2. hum-rt starts, connects to scsynth, loads SynthDefs, enters event loop
3. Editing piece.hum causes only diff-based OSC (Add/Remove, not free-all-recreate)
4. Editing a .scd file for an active thing triggers load_synthdef + new_synth (hot-swap)
5. Things with at: time activate at the right playback position; things with until: deactivate
6. Things with no until: remain active indefinitely
7. Saving from a /mnt/ path uses PollWatcher (confirmed in logs)
</success_criteria>

<output>
After completion, create `.planning/phases/03-state-reconciler-watcher/03-3-SUMMARY.md`
</output>

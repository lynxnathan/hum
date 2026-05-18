# Phase 03: State, Reconciler + File Watcher - Research

**Researched:** 2026-03-20
**Domain:** Rust file-watching, state reconciliation, tokio async event loop, SuperCollider OSC
**Confidence:** HIGH — core patterns verified from existing project research files, Cargo.toml, and official crate docs

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None — all implementation decisions are Claude's discretion.

### Claude's Discretion
- State store design — Kubernetes-style desired/actual reconciliation loop
- Diff algorithm — compute minimal delta (Add/Remove/Update) between two `Piece` values
- File watcher setup — notify crate with debounce; inotify for native paths, PollWatcher for /mnt/ NTFS paths
- Timeline tick — tokio::time::interval for advancing playback position; at:/until: transitions
- SynthDef crossfade on .scd change — reload SynthDef, crossfade from old to new without click
- Event bus design — tokio mpsc channel connecting file watcher, timeline ticker, and reconciler

### Deferred Ideas (OUT OF SCOPE)
None
</user_constraints>

---

## Summary

Phase 3 builds the core feedback loop: file changes drive OSC deltas in real time. The architecture from prior research is clear — a single tokio task owns all mutable state (desired + actual), all producers send events to one mpsc channel, and the reconciler computes minimal diffs. The existing `ScsynthClient`, `Piece`/`ThingDef` types, and `ScdStore` are the exact integration surface; no new data model design is needed.

The three non-obvious implementation problems are: (1) detecting /mnt/ paths and switching to PollWatcher at watcher construction time, (2) threading the timeline's at/until filtering correctly so the reconciler only sees the currently-active subset, and (3) the SynthDef crossfade sequence — load new def, await /synced, free old node, create new node — without creating a gap or double-play.

The event bus uses a single `DaemonEvent` enum over a bounded `tokio::sync::mpsc` channel. The event loop processes one event at a time with `tokio::select!`, owns `StateStore` directly (no Arc/Mutex), and calls `ScsynthClient` methods as the actuator layer.

**Primary recommendation:** Build in order: events.rs enum → StateStore → reconciler (unit-testable with no I/O) → watcher → timeline ticker → wire together in main event loop.

---

## Standard Stack

### Core (already in Cargo.toml or adding now)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| notify | 7.x | File system event watching (inotify/kqueue/etc.) | De-facto standard for Rust file watching |
| notify-debouncer-full | 0.4.x | Debounce + dedup wrapper over notify | Collapses 3-5 events per save into one; reduces reconciler thrash |
| tokio | 1 (already) | Async runtime, mpsc channels, interval timer | Already in project; provides all primitives needed |
| indexmap | 2 (already) | Ordered map for Piece | Already in project; iteration order matches file order |

### New dependencies to add

```toml
notify = "7"
notify-debouncer-full = "0.4"
```

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| notify-debouncer-full | notify-debouncer-mini | mini is simpler but merges all event kinds; full gives per-path deduplication |
| notify-debouncer-full | raw notify + manual tokio timer | More control, but re-implementing debounce logic that the crate already handles well |
| tokio::sync::mpsc | tokio::sync::broadcast | broadcast is for fan-out; mpsc is correct for many-producers → one-consumer (our pattern) |

---

## Architecture Patterns

### Recommended Module Structure

```
src/
├── events.rs            # DaemonEvent enum (FileChanged, Tick)
├── state.rs             # StateStore: desired Piece + actual HashMap<String,NodeState>
├── reconciler.rs        # diff(desired_active, actual) -> Vec<ReconcileOp>
├── watcher.rs           # notify watcher construction, /mnt/ detection, event → DaemonEvent
├── timeline.rs          # PlaybackState, parse_seconds(), at/until filtering, tick interval
└── main.rs              # wire all above; event loop with tokio::select!
```

(Phase 3 does NOT add transport.rs — that is Phase 4.)

---

### Pattern 1: Event Bus — DaemonEvent enum + bounded mpsc

**What:** Single enum covers all input sources. Single `tokio::sync::mpsc::channel(64)` — bounded to apply backpressure. One Sender clone per producer (watcher, timeline). One Receiver in the event loop.

```rust
// src/events.rs
use std::path::PathBuf;

pub enum DaemonEvent {
    /// A watched file changed. Path is the absolute path.
    FileChanged(PathBuf),
    /// Timeline tick. Payload is current playback position in seconds.
    Tick(f64),
}
```

Event loop pattern:

```rust
// src/main.rs (event loop skeleton)
loop {
    tokio::select! {
        Some(event) = rx.recv() => {
            match event {
                DaemonEvent::FileChanged(path) => {
                    handle_file_change(&mut state, &mut client, &scd_store_path, path).await;
                }
                DaemonEvent::Tick(pos) => {
                    handle_tick(&mut state, &mut client, pos).await;
                }
            }
        }
    }
}
```

**Key invariant:** `state` (StateStore) and `client` (ScsynthClient) are owned by the event loop task. Zero Arc/Mutex needed.

---

### Pattern 2: File Watcher — /mnt/ path detection + PollWatcher fallback

**What:** At watcher construction time, inspect the watch paths. If any path starts with `/mnt/`, the entire watcher must use `PollWatcher` — inotify is silent for NTFS-mounted paths in WSL2 (confirmed architectural limitation, not fixable).

```rust
// src/watcher.rs
use std::path::{Path, PathBuf};
use std::time::Duration;
use notify::{RecommendedWatcher, PollWatcher, RecursiveMode, Watcher, Config};
use notify_debouncer_full::{new_debouncer, new_debouncer_opt, DebounceEventResult};
use tokio::sync::mpsc::Sender;
use crate::events::DaemonEvent;

fn paths_need_poll(paths: &[&Path]) -> bool {
    paths.iter().any(|p| {
        p.to_str().map(|s| s.starts_with("/mnt/")).unwrap_or(false)
    })
}

pub fn start_watcher(
    paths: &[PathBuf],
    tx: Sender<DaemonEvent>,
) -> anyhow::Result<()> {
    let path_refs: Vec<&Path> = paths.iter().map(|p| p.as_path()).collect();
    let debounce = Duration::from_millis(80);

    let tx_clone = tx.clone();
    let callback = move |result: DebounceEventResult| {
        if let Ok(events) = result {
            for event in events {
                let _ = tx_clone.blocking_send(
                    DaemonEvent::FileChanged(event.path)
                );
            }
        }
    };

    if paths_need_poll(&path_refs) {
        // WSL2 /mnt/ path — use PollWatcher (inotify does not fire here)
        tracing::warn!("watching /mnt/ path — using PollWatcher (inotify unavailable for NTFS)");
        let config = Config::default().with_poll_interval(Duration::from_millis(500));
        let mut debouncer = new_debouncer_opt::<_, PollWatcher>(debounce, None, callback, config)?;
        for p in paths {
            debouncer.watcher().watch(p, RecursiveMode::NonRecursive)?;
        }
        // Leak debouncer — it must live for the process lifetime
        std::mem::forget(debouncer);
    } else {
        // Linux-native path — use inotify (RecommendedWatcher on Linux)
        let mut debouncer = new_debouncer(debounce, None, callback)?;
        for p in paths {
            debouncer.watcher().watch(p, RecursiveMode::NonRecursive)?;
        }
        std::mem::forget(debouncer);
    }

    Ok(())
}
```

**Notes:**
- `blocking_send` is used in the sync callback; the mpsc channel must not be full or events are dropped. 64-slot buffer is sufficient for typical save rates.
- `NonRecursive` is correct — we watch specific files and one directory (`out/sc/`), not trees.
- `std::mem::forget` keeps the debouncer alive. Production code should store it in a struct field instead; the forget is acceptable for a daemon that runs until process exit.

---

### Pattern 3: State Store — desired + actual

**What:** `StateStore` holds the full parsed `Piece` (desired) and a map from thing name to running node info (actual). The reconciler reads both and produces operations. Only the event loop mutates this struct.

```rust
// src/state.rs
use indexmap::IndexMap;
use crate::parser::types::{Piece, ThingDef};

/// What is currently playing in scsynth (the daemon's view of reality).
#[derive(Default)]
pub struct ActualState {
    /// thing_name -> scsynth node_id (already tracked inside ScsynthClient.nodes,
    /// but mirrored here for reconciler diffing without borrowing the client).
    pub nodes: IndexMap<String, i32>,
}

/// Full daemon state — owned by the event loop task.
pub struct StateStore {
    /// Latest successfully parsed piece.hum. None until first successful parse.
    pub desired: Option<Piece>,
    /// What scsynth currently has running.
    pub actual: ActualState,
    /// Current playback position in seconds.
    pub playback_pos: f64,
}

impl StateStore {
    pub fn new() -> Self {
        Self {
            desired: None,
            actual: ActualState::default(),
            playback_pos: 0.0,
        }
    }

    /// Return the subset of desired Things that are active at `pos` seconds.
    /// Active = at <= pos AND (until is absent OR pos < until).
    pub fn active_things(&self, pos: f64) -> IndexMap<String, &ThingDef> {
        let Some(piece) = &self.desired else { return IndexMap::new() };
        piece.iter()
            .filter(|(_, thing)| is_active(thing, pos))
            .map(|(name, thing)| (name.clone(), thing))
            .collect()
    }
}

fn is_active(thing: &ThingDef, pos: f64) -> bool {
    let at = thing.at.as_deref().and_then(parse_seconds).unwrap_or(0.0);
    if pos < at { return false; }
    if let Some(until_str) = &thing.until {
        if let Some(until) = parse_seconds(until_str) {
            if pos >= until { return false; }
        }
    }
    true
}

/// Parse "10s" -> Some(10.0), "0s" -> Some(0.0), anything else -> None.
pub fn parse_seconds(s: &str) -> Option<f64> {
    s.strip_suffix('s')?.parse().ok()
}
```

---

### Pattern 4: Reconciler — diff desired_active vs actual

**What:** Pure function. Takes active Things (desired) and ActualState. Returns a `Vec<ReconcileOp>` — no I/O, fully unit-testable.

```rust
// src/reconciler.rs
use indexmap::IndexMap;
use crate::parser::types::ThingDef;
use crate::state::ActualState;

pub enum ReconcileOp {
    /// Start a new synth for this thing (SynthDef must already be loaded).
    Add { thing_name: String, synthdef_name: String },
    /// Free the running synth for this thing.
    Remove { thing_name: String },
    /// SynthDef changed for a running thing — crossfade swap.
    Swap { thing_name: String, new_synthdef_name: String },
}

pub fn diff(
    active: &IndexMap<String, &ThingDef>,
    actual: &ActualState,
) -> Vec<ReconcileOp> {
    let mut ops = Vec::new();

    // Things active but not running -> Add
    for (name, _thing) in active {
        if !actual.nodes.contains_key(name.as_str()) {
            ops.push(ReconcileOp::Add {
                thing_name: name.clone(),
                synthdef_name: name.clone(), // thing name == synthdef name by convention
            });
        }
    }

    // Things running but not active -> Remove
    for name in actual.nodes.keys() {
        if !active.contains_key(name.as_str()) {
            ops.push(ReconcileOp::Remove { thing_name: name.clone() });
        }
    }

    ops
}
```

The `Swap` variant is emitted from a separate path — when `handle_file_change` detects an `.scd` file changed for a thing that already has a running node (see Pattern 6 below).

---

### Pattern 5: Timeline Tick — tokio::time::interval

**What:** A separate tokio task fires `DaemonEvent::Tick(pos)` at 50ms intervals when playing. The event loop updates `state.playback_pos`, recomputes the active set, and triggers reconciliation if the set changed.

```rust
// src/timeline.rs
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Sender;
use crate::events::DaemonEvent;

pub async fn run_ticker(tx: Sender<DaemonEvent>, start_pos: f64) {
    let start = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_millis(50));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let pos = start_pos + start.elapsed().as_secs_f64();
        if tx.send(DaemonEvent::Tick(pos)).await.is_err() {
            break; // event loop dropped — shut down
        }
    }
}
```

**Key:** `MissedTickBehavior::Skip` — if reconciliation takes longer than 50ms (shouldn't happen), skip the missed ticks rather than bursting. This keeps the event loop from being flooded during heavy I/O.

**Reconciler call on Tick:** Only re-reconcile if the active set actually changed (compare key sets). Otherwise Tick is a no-op — avoids 20 OSC sends per second when nothing changes.

```rust
// In event loop handle_tick:
async fn handle_tick(state: &mut StateStore, client: &mut ScsynthClient, pos: f64) {
    let old_active_keys: Vec<String> = state.active_things(state.playback_pos).into_keys().collect();
    state.playback_pos = pos;
    let new_active = state.active_things(pos);
    let new_active_keys: Vec<String> = new_active.keys().cloned().collect();
    if old_active_keys != new_active_keys {
        let ops = reconciler::diff(&new_active, &state.actual);
        apply_ops(state, client, ops).await;
    }
}
```

---

### Pattern 6: SynthDef Hot-Swap (Crossfade)

**What:** When an `.scd` file changes for a thing that is currently playing, we must: (1) load the new SynthDef bytes via `/d_recv` + await `/synced`, (2) free the old node, (3) create a new node with the new SynthDef. Steps must be in this order to prevent a gap or an orphaned node.

The ScsynthClient already handles steps 1 and 3 correctly: `load_synthdef` awaits `/synced` before returning, and `new_synth` frees any existing node for the thing name before creating a new one.

```rust
// In handle_file_change, when path is in out/sc/
async fn handle_scd_change(
    state: &mut StateStore,
    client: &mut ScsynthClient,
    scd_store: &mut ScdStore,
    changed_path: &Path,
) {
    // Reload the single changed file into the store
    let Some(stem) = changed_path.file_stem().and_then(|s| s.to_str()) else { return };
    let Ok(bytes) = std::fs::read(changed_path) else { return };

    // 1. Load new SynthDef — blocks until /synced (safe, async, <5s timeout)
    if let Err(e) = client.load_synthdef(bytes).await {
        tracing::error!("failed to load synthdef {}: {}", stem, e);
        return;
    }

    // 2. If this thing is currently active and running, swap the node.
    //    new_synth() frees the old node internally before creating the new one.
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
```

**True crossfade (amplitude ramp):** The minimal version above produces a brief silence between free and new_synth. For a click-free crossfade, the SynthDef must expose an `amp` control, and the sequence becomes: create new node at amp=0, ramp new node amp to 1 over N ms via `/n_set`, ramp old node amp to 0 via `/n_set`, then free old node. This requires the `.scd` SynthDef to have an `amp` arg. Defer to v1.x unless the SynthDef convention is established.

---

### Pattern 7: Applying ReconcileOps — the actuator layer

```rust
async fn apply_ops(
    state: &mut StateStore,
    client: &mut ScsynthClient,
    ops: Vec<ReconcileOp>,
) {
    for op in ops {
        match op {
            ReconcileOp::Add { thing_name, synthdef_name } => {
                match client.new_synth(&thing_name, &synthdef_name).await {
                    Ok(node_id) => {
                        state.actual.nodes.insert(thing_name, node_id);
                    }
                    Err(e) => tracing::error!("new_synth failed for '{}': {}", thing_name, e),
                }
            }
            ReconcileOp::Remove { thing_name } => {
                if let Err(e) = client.free_node(&thing_name).await {
                    tracing::error!("free_node failed for '{}': {}", thing_name, e);
                }
                state.actual.nodes.remove(&thing_name);
            }
            ReconcileOp::Swap { thing_name, new_synthdef_name } => {
                // load_synthdef must have already been called before emitting Swap
                match client.new_synth(&thing_name, &new_synthdef_name).await {
                    Ok(node_id) => {
                        state.actual.nodes.insert(thing_name, node_id);
                    }
                    Err(e) => tracing::error!("swap new_synth failed for '{}': {}", thing_name, e),
                }
            }
        }
    }
}
```

---

### Anti-Patterns to Avoid

- **Free all + recreate on every file save:** Causes audible gaps. Diff-based reconciliation exists precisely to prevent this.
- **PollWatcher everywhere:** 500ms poll interval on Linux-native paths defeats the <1s feedback target. Detect /mnt/ at startup; use inotify for ~/code/hum paths.
- **Arc<Mutex<StateStore>>:** The single event loop pattern eliminates the need entirely. Shared mutable state causes ordering bugs and deadlock risk.
- **Emitting Tick on every interval regardless of change:** Causes 20 reconciler calls/sec. Gate on active-set change.
- **Crossfade without awaiting /synced:** Will fire new_synth before the SynthDef is installed. ScsynthClient.load_synthdef already awaits /synced — always use it, never fire /s_new manually after /d_recv.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Debounce file events | Manual tokio sleep + channel drain | `notify-debouncer-full` | Editor saves generate 3-5 events; debouncer merges them correctly |
| Poll vs inotify selection | Custom filesystem type detection | Construct `PollWatcher` or `RecommendedWatcher` based on path prefix check | Path prefix check is 3 lines; the watcher construction handles the rest |
| Async timer | Hand-rolled Instant loop | `tokio::time::interval` with `MissedTickBehavior::Skip` | Correct drift handling, select!-compatible |
| Node lifecycle | Custom node ID allocation | `ScsynthClient.new_synth` (already handles free-before-create) | Already implemented in Phase 1; prevents double-node bugs |
| Sync handshake | Custom /done listener | `ScsynthClient.load_synthdef` (already implements /sync + /synced) | Already implemented; bypasses scsynth bug #4411 |

---

## Common Pitfalls

### Pitfall 1: inotify silent on /mnt/ paths
**What goes wrong:** Watcher constructed with RecommendedWatcher (inotify on Linux) on a /mnt/c/ path never fires. Daemon runs, saves happen, nothing reacts.
**How to avoid:** Check path prefix at watcher construction time. Any path starting with `/mnt/` forces PollWatcher for all watched paths.
**Warning signs:** Works from `vim` in WSL2, silent when editing from VS Code on Windows.

### Pitfall 2: Orphaned nodes on hot-swap
**What goes wrong:** New SynthDef loaded, new node created, old node still playing. CPU climbs after repeated saves.
**How to avoid:** `ScsynthClient.new_synth` already calls `free_node_by_id` if a node exists for the thing name. Rely on this — don't call `load_synthdef` + `new_synth` from outside paths that bypass this.
**Warning signs:** `scsynth` node tree shows duplicate entries for same thing name.

### Pitfall 3: Tick floods reconciler when nothing changes
**What goes wrong:** 20 reconciler diffs per second even when piece.hum hasn't changed and no things are transitioning.
**How to avoid:** Cache the previous active-key set. Only run diff+apply when the set changes.

### Pitfall 4: parse_seconds fails silently
**What goes wrong:** `at: 10` (no "s" suffix) parses as None, thing activates at t=0 unexpectedly.
**How to avoid:** `parse_seconds` returns `None` for non-"Xs" strings. Treat `None` as `0.0` (immediate). Document the required format in error messages.

### Pitfall 5: ScdStore not updated on .scd change
**What goes wrong:** Watcher detects .scd change, calls `load_synthdef` with stale cached bytes from the store loaded at startup.
**How to avoid:** On `.scd` file change event, re-read the file from disk directly (not from ScdStore). ScdStore is for initial bulk load; single-file hot-reload reads the changed file fresh.

---

## Code Examples

All examples above reference the actual project types from:
- `src/osc/bridge.rs` — `ScsynthClient` with `load_synthdef`, `new_synth`, `set_param`, `free_node`, `free_all_nodes`
- `src/parser/types.rs` — `Piece = IndexMap<String, ThingDef>`, `ThingDef.at`, `ThingDef.until`
- `src/scd/store.rs` — `ScdStore::load_dir`, `ScdStore::get`

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| Raw notify events | notify-debouncer-full | Correct handling of editor temp-rename save patterns |
| Manual sleep before /s_new | /sync + /synced handshake (already in ScsynthClient) | Eliminates SynthDef race condition |
| Free-all on reload | Diff-based reconciliation | Uninterrupted playback of unchanged things during edits |

---

## Open Questions

1. **Crossfade smoothness**
   - What we know: `new_synth` frees old node before creating new one — brief silence is possible
   - What's unclear: Whether SynthDefs generated by LLMs will expose an `amp` control arg
   - Recommendation: Ship v1 with free-then-create (brief silence acceptable); add ramp crossfade in v1.x once SynthDef convention is established

2. **at:/until: time format validation**
   - What we know: `ThingDef.at` and `ThingDef.until` are `Option<String>`; `parse_seconds` handles "Xs" format
   - What's unclear: Should the parser validate the format at parse time, or silently treat bad values as t=0?
   - Recommendation: Add validation in `parse_seconds` callers — log a warning but treat unparseable values as 0.0 rather than hard-failing

3. **ScdStore update strategy**
   - What we know: Current `ScdStore` is load-once from disk
   - What's unclear: Whether to add a `ScdStore::update(path, bytes)` method or read the changed file ad-hoc in the watcher handler
   - Recommendation: Read ad-hoc on change (avoids mutating the store); ScdStore remains read-only after startup

---

## Sources

### Primary (HIGH confidence)
- `src/osc/bridge.rs` — exact ScsynthClient API, /sync+/synced pattern already implemented
- `src/parser/types.rs` — exact ThingDef fields (at, until as Option<String>)
- `src/scd/store.rs` — exact ScdStore API
- `.planning/research/ARCHITECTURE.md` — event bus design, reconciler pattern, anti-patterns
- `.planning/research/PITFALLS.md` — inotify/WSL2 pitfall, orphaned nodes, /d_recv race

### Secondary (MEDIUM confidence)
- [notify-rs/notify GitHub](https://github.com/notify-rs/notify) — PollWatcher, RecommendedWatcher, Watcher trait
- [notify-debouncer-full docs.rs](https://docs.rs/notify-debouncer-full/latest/notify_debouncer_full/) — new_debouncer, new_debouncer_opt API
- WebSearch: notify-debouncer-full new_debouncer_opt accepts custom watcher type parameter (verified from search results and crate description)

### Tertiary (LOW confidence — flag for validation)
- Specific `new_debouncer_opt::<_, PollWatcher>` generic syntax — verify against current crate version during implementation; API may differ slightly between 0.3.x and 0.4.x

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — notify and notify-debouncer-full are the standard choices; confirmed in search results
- Architecture: HIGH — event bus, desired/actual diff pattern fully documented in prior research
- Pitfalls: HIGH — inotify/WSL2 and orphaned node issues verified from official sources
- Code examples: MEDIUM — patterns are correct; exact notify-debouncer-full generic syntax needs validation against installed version

**Research date:** 2026-03-20
**Valid until:** 2026-06-01 (notify-rs moves slowly; tokio is stable)

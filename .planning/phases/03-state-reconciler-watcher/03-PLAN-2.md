---
phase: 03-state-reconciler-watcher
plan: 2
type: execute
wave: 2
depends_on:
  - 03-PLAN-1
files_modified:
  - src/watcher.rs
  - src/timeline.rs
autonomous: true
requirements:
  - WATCH-01
  - WATCH-04
  - TIME-01
must_haves:
  truths:
    - "Saving piece.hum from a Linux-native path causes a DaemonEvent::FileChanged within ~1 second"
    - "Watcher on a /mnt/ path uses PollWatcher (inotify fallback confirmed in log)"
    - "Timeline ticker sends DaemonEvent::Tick(pos) at ~50ms intervals"
    - "Ticker pos advances monotonically from start_pos"
  artifacts:
    - path: "src/watcher.rs"
      provides: "start_watcher() — detect /mnt/ paths, debounce, send DaemonEvent::FileChanged"
      exports: ["start_watcher"]
    - path: "src/timeline.rs"
      provides: "run_ticker() — tokio::time::interval, MissedTickBehavior::Skip, sends Tick"
      exports: ["run_ticker"]
  key_links:
    - from: "src/watcher.rs"
      to: "src/events.rs"
      via: "DaemonEvent::FileChanged sent over mpsc Sender"
      pattern: "use crate::events::DaemonEvent"
    - from: "src/timeline.rs"
      to: "src/events.rs"
      via: "DaemonEvent::Tick sent over mpsc Sender"
      pattern: "use crate::events::DaemonEvent"
---

<objective>
Build the two event producers: file watcher (with /mnt/ PollWatcher fallback) and timeline ticker. Both send to the mpsc channel established in Plan 3 — this plan creates the functions, not the channel.

Purpose: File watching and timeline ticking are independent of each other and of main.rs wiring. Building them in isolation keeps each unit focused.
Output: src/watcher.rs, src/timeline.rs — compiling, correct /mnt/ detection logic.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/phases/03-state-reconciler-watcher/03-CONTEXT.md
@.planning/phases/03-state-reconciler-watcher/research/RESEARCH.md
@.planning/phases/03-state-reconciler-watcher/03-1-SUMMARY.md

<interfaces>
<!-- From Plan 1 output — use these exactly. -->

From src/events.rs:
```rust
pub enum DaemonEvent {
    FileChanged(std::path::PathBuf),
    Tick(f64),
}
```

notify-debouncer-full API (verify generic syntax against installed 0.4.x):
```rust
// inotify path:
let mut debouncer = new_debouncer(debounce_duration, None, callback)?;
debouncer.watcher().watch(path, RecursiveMode::NonRecursive)?;

// /mnt/ path (PollWatcher):
let config = notify::Config::default().with_poll_interval(Duration::from_millis(500));
let mut debouncer = new_debouncer_opt::<_, notify::PollWatcher>(debounce_duration, None, callback, config)?;
debouncer.watcher().watch(path, RecursiveMode::NonRecursive)?;
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: File watcher with /mnt/ PollWatcher detection</name>
  <files>src/watcher.rs</files>
  <action>
    Create src/watcher.rs implementing start_watcher() as documented in RESEARCH.md Pattern 2.

    Signature:
    ```rust
    pub fn start_watcher(
        paths: &[std::path::PathBuf],
        tx: tokio::sync::mpsc::Sender<crate::events::DaemonEvent>,
    ) -> anyhow::Result<()>
    ```

    Key implementation points:
    - paths_need_poll helper: any path starting with "/mnt/" forces PollWatcher for ALL watched paths
    - Debounce = 80ms (collapses editor's temp-rename save pattern into one event)
    - Callback uses blocking_send (sync context) — if channel full, event is dropped (logged at warn)
    - Watch mode: RecursiveMode::NonRecursive for all paths (we watch specific files and out/sc/ dir, not trees)
    - std::mem::forget(debouncer) — debouncer must outlive the function call; it runs until process exit
    - Log at tracing::warn when PollWatcher is selected: "watching /mnt/ path — using PollWatcher (inotify unavailable for NTFS)"
    - The DebounceEventResult callback: iterate events, send each event.path as DaemonEvent::FileChanged

    PITFALL: Verify the actual generic syntax for new_debouncer_opt with the installed notify-debouncer-full version.
    Run `cargo add notify-debouncer-full@0.4 notify@7` to add if not already in Cargo.toml (Plan 1 added them, but double-check).
    If the API differs from RESEARCH.md, adapt to the actual crate API — the path-prefix detection logic is stable regardless.
  </action>
  <verify>cargo check 2>&1 | grep -E "^error" | wc -l  # must be 0</verify>
  <done>src/watcher.rs compiles, start_watcher exported, /mnt/ detection present in code</done>
</task>

<task type="auto">
  <name>Task 2: Timeline ticker</name>
  <files>src/timeline.rs</files>
  <action>
    Create src/timeline.rs implementing run_ticker() as documented in RESEARCH.md Pattern 5.

    ```rust
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
                break; // receiver dropped — event loop shut down, exit cleanly
            }
        }
    }
    ```

    MissedTickBehavior::Skip is mandatory — prevents burst of ticks if reconciliation stalls.
    The ticker starts immediately (Phase 4 will add play/pause control). For Phase 3 the ticker runs from daemon startup at pos=0.0.
  </action>
  <verify>cargo check 2>&1 | grep -E "^error" | wc -l  # must be 0</verify>
  <done>src/timeline.rs compiles, run_ticker exported, MissedTickBehavior::Skip present</done>
</task>

</tasks>

<verification>
```
cargo check
cargo test
```
Zero errors, all Plan 1 tests still green.
</verification>

<success_criteria>
- `cargo check` clean
- start_watcher() accepts paths + mpsc Sender, detects /mnt/ prefix, selects correct watcher type
- run_ticker() uses tokio::time::interval at 50ms with MissedTickBehavior::Skip
- Both modules export their public function; no I/O or main.rs changes yet
</success_criteria>

<output>
After completion, create `.planning/phases/03-state-reconciler-watcher/03-2-SUMMARY.md`
</output>

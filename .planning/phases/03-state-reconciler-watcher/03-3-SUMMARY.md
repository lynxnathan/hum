---
phase: 03-state-reconciler-watcher
plan: 3
subsystem: runtime
tags: [event-loop, tokio-select, reconciler, file-watcher, timeline, osc, scsynth]

# Dependency graph
requires:
  - phase: 03-state-reconciler-watcher
    provides: "DaemonEvent, StateStore, reconciler diff, start_watcher, run_ticker from Plans 1+2"
  - phase: 02-parser-scd-reader
    provides: "parse_hum, ScdStore for loading piece.hum and SynthDef bytes"
  - phase: 01-osc-bridge
    provides: "ScsynthClient with load_synthdef, new_synth, free_node, free_all_nodes"
provides:
  - "Complete hum-rt event loop: file watcher + timeline ticker + reconciler wired together"
  - "handle_file_change: .hum reparse + diff reconcile, .scd reload + hot-swap"
  - "handle_tick: reconcile only on active set change (no tick flood)"
  - "Graceful Ctrl-C shutdown with free_all_nodes"
  - "Initial reconciliation at startup for things active at t=0"
affects: [04-transport-cli]

# Tech tracking
tech-stack:
  added: []
  patterns: [tokio::select! event loop with mpsc channel, diff-based reconciliation on file change, active-set-change gating for tick events, .scd hot-swap via load_synthdef + new_synth]

key-files:
  created: []
  modified:
    - src/main.rs

key-decisions:
  - "state and client owned by event loop -- zero Arc/Mutex"
  - "Tick is no-op when active key set unchanged (prevents 20 OSC calls/sec)"
  - "On .scd change, read from disk directly (not from ScdStore -- store is load-once)"
  - "Parse errors on .hum reload keep previous desired state (do not clear)"
  - "Initial reconciliation at startup triggers Add ops for things active at t=0"

patterns-established:
  - "Event loop pattern: single tokio::select! with mpsc receiver + ctrl_c"
  - "handle_file_change dispatches on extension (.hum vs .scd)"
  - "reconcile_now computes active_things then diff then apply_ops"
  - "apply_ops updates state.actual.nodes alongside client calls"

requirements-completed: [WATCH-01, WATCH-02, WATCH-03, WATCH-04, TIME-01, TIME-02, TIME-03]

# Metrics
duration: 1min
completed: 2026-03-20
---

# Phase 3 Plan 3: Event Loop Wiring Summary

**Tokio event loop replacing smoke test: file watcher + timeline ticker + diff-based reconciler producing minimal OSC deltas on piece.hum and .scd saves**

## Performance

- **Duration:** 1 min
- **Started:** 2026-03-20T17:14:28Z
- **Completed:** 2026-03-20T17:15:51Z
- **Tasks:** 1 (Task 2 is checkpoint:human-verify, auto-approved)
- **Files modified:** 1

## Accomplishments
- Replaced run_smoke_test and SINE_SCSYNDEF with real tokio::select! event loop
- Wired all six modules: events, state, reconciler, watcher, timeline, osc
- Implemented handle_file_change (.hum reparse + reconcile, .scd reload + hot-swap)
- Implemented handle_tick with active-set-change gating (no-op when unchanged)
- Graceful Ctrl-C shutdown freeing all nodes
- Initial reconciliation at startup for things active at t=0

## Task Commits

Each task was committed atomically:

1. **Task 1: Replace smoke test with real event loop** - `56cbaee` (feat)

## Files Created/Modified
- `src/main.rs` - Full event loop: startup sequence, mpsc channel, watcher + ticker spawn, FileChanged/Tick/Ctrl-C handling, reconcile_now, apply_ops

## Decisions Made
- state and client owned by event loop task -- zero Arc/Mutex (single-threaded ownership)
- Tick handler only reconciles when active key set changes (Vec<String> comparison), preventing 20 unnecessary OSC round-trips per second
- On .scd file change, bytes are read fresh from disk (not from ScdStore which is load-once at startup)
- Parse errors during .hum reload preserve previous desired state rather than clearing it
- Added initial reconcile_now call after startup to immediately start things active at t=0

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 3 complete: hum-rt daemon watches piece.hum and out/sc/, ticks playback, reconciles OSC state
- Ready for Phase 4 (transport controls, CLI)
- All 43 tests pass across all modules

## Self-Check: PASSED

- FOUND: src/main.rs
- FOUND: commit 56cbaee

---
*Phase: 03-state-reconciler-watcher*
*Completed: 2026-03-20*

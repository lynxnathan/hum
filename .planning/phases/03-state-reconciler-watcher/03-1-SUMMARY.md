---
phase: 03-state-reconciler-watcher
plan: 1
subsystem: runtime
tags: [reconciler, state-store, events, timeline, indexmap]

# Dependency graph
requires:
  - phase: 02-parser-scd-reader
    provides: "Piece and ThingDef types from parser"
provides:
  - "DaemonEvent enum (FileChanged, Tick) for event bus"
  - "StateStore with desired/actual/playback_pos and active_things filter"
  - "parse_seconds utility for at:/until: time parsing"
  - "ReconcileOp::Add/Remove/Swap enum and pure diff() function"
  - "ActualState (thing_name -> node_id map) for reconciler diffing"
affects: [03-state-reconciler-watcher, 04-transport-cli]

# Tech tracking
tech-stack:
  added: [notify 7, notify-debouncer-full 0.4]
  patterns: [kubernetes-style desired/actual reconciliation, pure diff function, seconds-based timeline]

key-files:
  created:
    - src/events.rs
    - src/state.rs
    - src/reconciler.rs
  modified:
    - Cargo.toml
    - src/main.rs

key-decisions:
  - "Thing name == SynthDef name by convention (name is the stable identity key)"
  - "parse_seconds returns None for non-Xs strings; absent at treated as 0.0 (immediate)"
  - "ActualState mirrors ScsynthClient.nodes for reconciler diffing without borrowing the client"
  - "Swap variant exists but diff() only emits Add/Remove; Swap comes from SCD hot-swap path"

patterns-established:
  - "Pure logic modules: no I/O, no async, fully unit-testable in isolation"
  - "active_things filter: at <= pos AND (no until OR pos < until)"
  - "Reconciler diff pattern: active but not running = Add, running but not active = Remove"

requirements-completed: [WATCH-02, TIME-01, TIME-02, TIME-03]

# Metrics
duration: 3min
completed: 2026-03-20
---

# Phase 3 Plan 1: State, Reconciler + File Watcher Summary

**Pure-logic core: DaemonEvent enum, StateStore with active_things timeline filter, and reconciler diff producing Add/Remove ops**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-20T17:03:37Z
- **Completed:** 2026-03-20T17:06:26Z
- **Tasks:** 1 (single feature with TDD)
- **Files modified:** 5

## Accomplishments
- Created events.rs with DaemonEvent::FileChanged(PathBuf) and DaemonEvent::Tick(f64) enum
- Created state.rs with StateStore, ActualState, active_things() filter, and parse_seconds() utility
- Created reconciler.rs with ReconcileOp enum and pure diff() function
- 15 new tests covering all behavior cases from plan spec (36 total pass)
- Added notify + notify-debouncer-full dependencies to Cargo.toml for Plan 2

## Task Commits

Each task was committed atomically:

1. **Task 1: Event enum + StateStore + reconciler diff** - `6448026` (feat)

## Files Created/Modified
- `src/events.rs` - DaemonEvent enum (FileChanged, Tick) for the event bus
- `src/state.rs` - StateStore with desired/actual state, active_things filter, parse_seconds
- `src/reconciler.rs` - ReconcileOp enum and pure diff() function
- `Cargo.toml` - Added notify 7 and notify-debouncer-full 0.4 dependencies
- `src/main.rs` - Wired events, state, reconciler modules

## Decisions Made
- Thing name == SynthDef name by convention (stable identity key for reconciler)
- parse_seconds returns None for non-"Xs" format; absent `at` treated as 0.0 (active from start)
- ActualState mirrors ScsynthClient.nodes (IndexMap<String, i32>) to allow reconciler diffing without borrowing the async client
- Swap variant defined but diff() only emits Add/Remove; Swap is emitted from the SCD hot-swap path externally
- Used `crate::parser::{Piece, ThingDef}` re-exports (types module is private in parser)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed private module path for parser types**
- **Found during:** Task 1 (initial compilation)
- **Issue:** Plan referenced `crate::parser::types::ThingDef` but the `types` module is private; types are re-exported from `crate::parser`
- **Fix:** Changed imports to use `crate::parser::{Piece, ThingDef}` in both state.rs and reconciler.rs
- **Files modified:** src/state.rs, src/reconciler.rs
- **Verification:** cargo check clean, all tests pass
- **Committed in:** 6448026

**2. [Rule 3 - Blocking] Added type annotations for iterator collect**
- **Found during:** Task 1 (initial compilation)
- **Issue:** Rust type inference couldn't determine the collect target type in active_things()
- **Fix:** Added explicit type annotation to the map closure: `-> (String, &ThingDef)`
- **Files modified:** src/state.rs
- **Verification:** cargo check clean
- **Committed in:** 6448026

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes necessary for compilation. No scope creep.

## Issues Encountered
None beyond the auto-fixed blocking issues above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- DaemonEvent, StateStore, ActualState, ReconcileOp, diff, parse_seconds all exported and tested
- Ready for Plan 2 (file watcher) and Plan 3 (event loop wiring)
- notify + notify-debouncer-full dependencies already in Cargo.toml

## Self-Check: PASSED

- FOUND: src/events.rs
- FOUND: src/state.rs
- FOUND: src/reconciler.rs
- FOUND: 03-1-SUMMARY.md
- FOUND: commit 6448026

---
*Phase: 03-state-reconciler-watcher*
*Completed: 2026-03-20*

---
phase: 03-state-reconciler-watcher
plan: 2
subsystem: runtime
tags: [file-watcher, notify, poll-watcher, timeline, ticker, tokio-interval, wsl2]

# Dependency graph
requires:
  - phase: 03-state-reconciler-watcher
    provides: "DaemonEvent enum (FileChanged, Tick) from Plan 1"
provides:
  - "start_watcher() with /mnt/ PollWatcher detection and 80ms debounce"
  - "run_ticker() async timeline ticker at 50ms with MissedTickBehavior::Skip"
affects: [03-state-reconciler-watcher, 04-transport-cli]

# Tech tracking
tech-stack:
  added: []
  patterns: [/mnt/ path prefix detection for WSL2 PollWatcher fallback, std::mem::forget for daemon-lifetime watchers, MissedTickBehavior::Skip for tick flood prevention]

key-files:
  created:
    - src/watcher.rs
    - src/timeline.rs
  modified:
    - src/main.rs

key-decisions:
  - "notify-debouncer-full 0.4.0 API: new_debouncer_opt takes 5 args with FileIdCache; use NoCache for PollWatcher"
  - "DebouncedEvent.event.paths (not .path) -- iterate all paths per debounced event"
  - "debouncer.watch() directly instead of deprecated .watcher().watch()"
  - "blocking_send in sync callback; warn+drop if channel full"

patterns-established:
  - "WSL2 /mnt/ detection: any path starting with /mnt/ forces PollWatcher for all watched paths"
  - "Daemon-lifetime resources: std::mem::forget to leak debouncer (runs until process exit)"
  - "Timeline ticker: wall-clock Instant::now + start_pos for monotonic position"

requirements-completed: [WATCH-01, WATCH-04, TIME-01]

# Metrics
duration: 3min
completed: 2026-03-20
---

# Phase 3 Plan 2: File Watcher + Timeline Ticker Summary

**File watcher with /mnt/ PollWatcher fallback (notify-debouncer-full 0.4) and 50ms timeline ticker using tokio::time::interval with Skip behavior**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-20T17:08:43Z
- **Completed:** 2026-03-20T17:11:56Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Created src/watcher.rs with start_watcher() that detects /mnt/ paths and switches to PollWatcher (inotify is silent for NTFS in WSL2)
- Created src/timeline.rs with run_ticker() sending DaemonEvent::Tick at 50ms intervals with MissedTickBehavior::Skip
- 7 new tests (4 watcher path detection + 3 async ticker tests), 43 total pass

## Task Commits

Each task was committed atomically:

1. **Task 1: File watcher with /mnt/ PollWatcher detection** - `dab538e` (feat)
2. **Task 2: Timeline ticker** - `01ba219` (feat)

## Files Created/Modified
- `src/watcher.rs` - start_watcher() with /mnt/ detection, debounce, DaemonEvent::FileChanged dispatch
- `src/timeline.rs` - run_ticker() async fn with 50ms interval, monotonic pos, clean shutdown
- `src/main.rs` - Wired watcher and timeline modules

## Decisions Made
- notify-debouncer-full 0.4.0 API differs from RESEARCH.md docs: new_debouncer_opt requires 5 args (timeout, tick_rate, handler, file_id_cache, config) with 3 generics; used NoCache for PollWatcher path
- DebouncedEvent wraps notify::Event (accessed via .event.paths), not a direct .path field
- Used debouncer.watch() directly -- .watcher() is deprecated in 0.4.0
- blocking_send in sync watcher callback with warn-level log on channel full/closed (drop event rather than block)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Adapted to actual notify-debouncer-full 0.4.0 API**
- **Found during:** Task 1 (initial compilation)
- **Issue:** RESEARCH.md documented new_debouncer_opt with 4 args and 2 generics; actual 0.4.0 API takes 5 args (adds FileIdCache param) with 3 generics, DebouncedEvent has .event.paths not .path, and .watcher() is deprecated
- **Fix:** Used NoCache for file_id_cache param, iterated event.paths for each DebouncedEvent, called debouncer.watch() directly
- **Files modified:** src/watcher.rs
- **Verification:** cargo check clean, all tests pass
- **Committed in:** dab538e

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** API adaptation necessary for compilation. Core logic (path detection, debounce, event dispatch) unchanged. No scope creep.

## Issues Encountered
None beyond the API deviation documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- start_watcher() and run_ticker() exported and tested, ready for Plan 3 event loop wiring
- Both produce DaemonEvent variants over mpsc::Sender -- Plan 3 wires the receiver loop
- 43 total tests pass across all modules

## Self-Check: PASSED

- FOUND: src/watcher.rs
- FOUND: src/timeline.rs
- FOUND: 03-2-SUMMARY.md
- FOUND: commit dab538e
- FOUND: commit 01ba219

---
*Phase: 03-state-reconciler-watcher*
*Completed: 2026-03-20*

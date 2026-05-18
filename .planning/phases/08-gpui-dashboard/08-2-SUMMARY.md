---
phase: 08-gpui-dashboard
plan: 2
subsystem: ui
tags: [gpui, dashboard, vu-meters, catppuccin, transport-monitor]

# Dependency graph
requires:
  - phase: 08-gpui-dashboard
    provides: TransportCmd::PlayFrom, TransportReply::Status.amplitudes, parse_time_arg
  - phase: 04-transport-e2e
    provides: TransportCmd/TransportReply protocol, unix socket server/client
provides:
  - hum watch GPUI window with live timeline, VU meters, and transport bar
  - WatchView render pipeline with Catppuccin Mocha color scheme
  - Background daemon poll thread at 20fps via Arc<Mutex> shared state
  - send_cmd_safe() non-exiting socket client for watch mode
affects: [future GPUI enhancements, waveform rendering, embedded terminal]

# Tech tracking
tech-stack:
  added: [gpui 0.2]
  patterns: [Arc<Mutex> shared state between std::thread and GPUI, App::spawn + Timer::after for periodic re-render, free-function rendering to avoid borrow issues]

key-files:
  created: [src/watch.rs]
  modified: [Cargo.toml, src/main.rs]

key-decisions:
  - "Arc<Mutex<WatchData>> for poll thread to GPUI communication -- avoids GPUI async executor complexity, simple and reliable"
  - "Background std::thread with own tokio current-thread runtime for daemon polling -- GPUI executor is not tokio, clean separation"
  - "App::spawn + Timer::after + window.refresh for 20fps re-render loop -- GPUI-native async scheduling"
  - "Free render functions instead of methods on WatchView -- avoids borrow conflicts with Arc<Mutex> data"

patterns-established:
  - "GPUI window entry: force X11 via remove_var(WAYLAND_DISPLAY), Application::new().run()"
  - "Async bridge: std::thread + tokio runtime for IO, Arc<Mutex> to pass data to GPUI view"
  - "GPUI re-render: App::spawn async loop with Timer::after + WindowHandle::update + window.refresh()"

requirements-completed: [TUI-01, TUI-02, TUI-03]

# Metrics
duration: 5min
completed: 2026-03-22
---

# Phase 8 Plan 2: GPUI Dashboard + Transport Fix Summary

**GPUI live dashboard window with transport bar, per-thing VU meters, and 20fps daemon polling via Arc<Mutex> bridge**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-22T03:19:55Z
- **Completed:** 2026-03-22T03:25:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- `hum watch` opens a 900x500 GPUI window with Catppuccin Mocha dark theme
- Transport bar shows live position counter and PLAYING/STOPPED state badge
- Per-thing rows with active-indicator dots and VU meter bars (color-coded: blue/green/red by amplitude)
- Background thread polls daemon Status at 20fps, GPUI re-render loop keeps display in sync
- Graceful error handling: shows "daemon not running" when daemon is offline

## Task Commits

Each task was committed atomically:

1. **Task 1: Add gpui dependency and watch subcommand wiring** - `dc4fa6b` (feat)
2. **Task 2: Build WatchView -- GPUI window with timeline, VU meters, transport bar** - `c6efbe6` (feat)

## Files Created/Modified
- `src/watch.rs` - WatchView GPUI window: data model, render pipeline, poll thread, send_cmd_safe
- `Cargo.toml` - Added gpui = "0.2" dependency
- `src/main.rs` - Added mod watch, "watch" CLI arm calling watch::run_watch()

## Decisions Made
- Used Arc<Mutex<WatchData>> for thread-to-GPUI communication instead of GPUI channels -- simpler, avoids GPUI async executor complexity
- Background std::thread with own tokio current-thread runtime -- clean separation from GPUI's smol-based executor
- App::spawn with Timer::after + window.refresh() for periodic re-renders -- GPUI-native approach
- Free render functions (render_transport_bar, render_thing_list, etc.) instead of WatchView methods -- avoids borrow conflicts with mutex-locked data

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] GPUI API discovery and adaptation**
- **Found during:** Task 2 (WatchView implementation)
- **Issue:** Plan used cx.spawn with closure syntax incompatible with GPUI 0.2's AsyncFnOnce API; also referenced smol::unblock (not a dependency), .opacity() and .overflow_y_scroll() (not in GPUI 0.2 API)
- **Fix:** Read actual gpui-0.2.2 source to discover correct signatures: App::spawn takes AsyncFnOnce(&mut AsyncApp), WindowHandle::update takes 3-arg closure (view, window, cx). Used Arc<Mutex> + std::thread instead of smol::unblock. Removed unavailable style methods.
- **Files modified:** src/watch.rs
- **Verification:** cargo build succeeds with 0 errors, cargo test passes 129/129
- **Committed in:** c6efbe6 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** GPUI API adaptation was necessary to match actual 0.2.2 API signatures. Architecture (poll + render) matches plan intent. No scope creep.

## Issues Encountered
- GPUI 0.2.2 uses AsyncFnOnce trait (Rust nightly feature) for spawn APIs -- required reading crate source to discover correct function signatures
- Multiple build-fix iterations needed to resolve type inference, closure argument counts, and trait bound errors

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Dashboard window fully functional for visual verification (checkpoint task)
- VU meter bars display but amplitude data is placeholder (0.0) until real /n_get OSC wiring
- Ready for user testing: start daemon, play, then run `hum watch`

## Self-Check: PASSED

- [x] src/watch.rs exists
- [x] Cargo.toml exists
- [x] src/main.rs exists
- [x] Commit dc4fa6b found
- [x] Commit c6efbe6 found
- [x] cargo build clean (0 errors)
- [x] cargo test: 129 passed, 0 failed

---
*Phase: 08-gpui-dashboard*
*Completed: 2026-03-22*

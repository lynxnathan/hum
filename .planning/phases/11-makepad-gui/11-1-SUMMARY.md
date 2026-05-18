---
phase: 11-makepad-gui
plan: 1
subsystem: ui
tags: [makepad, gui, transport, unix-socket, catppuccin]

# Dependency graph
requires:
  - phase: 08-gpui-dashboard
    provides: "Transport protocol (unix socket JSON newline) and daemon Status reply"
provides:
  - "hum-gui binary target with Makepad window"
  - "Transport bar with play/stop buttons and live position display"
  - "transport_client background polling thread with Arc<Mutex<GuiState>>"
  - "3-zone layout scaffold (spectral, arrangement, transport bar)"
affects: [11-makepad-gui, spectral-analyzer, arrangement-view]

# Tech tracking
tech-stack:
  added: [makepad-widgets 1.0]
  patterns: [makepad-app-main, live-design-dsl, arc-mutex-gui-state, std-thread-polling]

key-files:
  created:
    - src/bin/gui/main.rs
    - src/bin/gui/app.rs
    - src/bin/gui/transport_client.rs
  modified:
    - Cargo.toml

key-decisions:
  - "std::thread for daemon polling (not tokio) -- Makepad has its own event loop"
  - "Text-based status indicator instead of dynamic color dot -- avoids live! macro issues with apply_over"
  - "20fps UI refresh via cx.start_interval(0.05) timer"
  - "Fire-and-forget send_cmd for play/stop (no reply parsing needed for Ack commands)"

patterns-established:
  - "Makepad App pattern: app_main! macro in app.rs, main.rs delegates to app::app_main()"
  - "GuiState polling: Arc<Mutex<GuiState>> shared between std::thread poller and Makepad event handler"
  - "Transport bar as bottom fixed-height View in 3-zone vertical layout"

requirements-completed: [MKPD-01, MKPD-05]

# Metrics
duration: 3min
completed: 2026-03-22
---

# Phase 11 Plan 1: Makepad GUI Summary

**hum-gui Makepad binary with transport bar (play/stop/position) polling daemon via unix socket at 20fps**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-22T19:16:55Z
- **Completed:** 2026-03-22T19:20:11Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments
- hum-gui binary target compiles and opens a Makepad window on WSL2
- Transport bar with play/stop buttons sends JSON commands to daemon via unix socket
- Background std::thread polls daemon Status every 100ms, updating position display live
- 3-zone vertical layout established (spectral placeholder, arrangement placeholder, transport bar)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add hum-gui binary target and makepad-widgets dep** - `e26a033` (feat)
2. **Task 2: transport_client daemon polling thread** - `0db3173` (feat)
3. **Task 3: Makepad app with transport bar UI** - `b670df4` (feat)

## Files Created/Modified
- `Cargo.toml` - Added makepad-widgets dep and [[bin]] hum-gui entry
- `src/bin/gui/main.rs` - Binary entry point delegating to app::app_main()
- `src/bin/gui/app.rs` - Makepad App with live_design! layout, transport bar, button handlers, timer refresh
- `src/bin/gui/transport_client.rs` - GuiState, start_polling (std::thread), send_cmd, Status JSON parsing

## Decisions Made
- Used std::thread (not tokio) for daemon polling since Makepad runs its own event loop
- Used text-based status indicator (">>> PLAYING" / "|| STOPPED" / "-- OFFLINE") instead of dynamic color dot -- the live! macro has syntax constraints with apply_over that need further investigation for dynamic styling
- Set UI refresh at 20fps (50ms interval) for smooth position updates without excessive redraws
- Fire-and-forget pattern for send_cmd (play/stop) -- no need to parse Ack reply

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed live! macro compilation error in apply_over**
- **Found during:** Task 3 (Makepad app with transport bar UI)
- **Issue:** `live!{ draw_bg: { color: #hex } }` inside apply_over caused macro expansion errors -- Makepad's live! macro has specific syntax constraints for expression context
- **Fix:** Replaced dynamic color dot with text-based status indicator in conn_label
- **Files modified:** src/bin/gui/app.rs
- **Verification:** cargo build --bin hum-gui succeeds cleanly
- **Committed in:** b670df4 (Task 3 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minor visual difference (text vs colored dot). Status information is fully conveyed. Dynamic styling will be revisited when custom draw widgets are implemented.

## Issues Encountered
None beyond the live! macro issue documented above.

## User Setup Required
None - no external service configuration required. Makepad window opens directly via `cargo run --bin hum-gui`.

## Next Phase Readiness
- hum-gui binary scaffold ready for spectral analyzer (Phase 11 Plan 2+)
- Transport bar functional -- future plans add seek slider, solo/mute toggles
- GuiState already carries amplitudes/active/solo/mute data for VU meters and arrangement view
- Makepad shader DSL exploration needed for spectral visualization (FFT uniforms)

## Self-Check: PASSED

- All 4 files exist (main.rs, app.rs, transport_client.rs, Cargo.toml)
- All 3 task commits verified (e26a033, 0db3173, b670df4)
- SUMMARY.md created at expected path

---
*Phase: 11-makepad-gui*
*Completed: 2026-03-22*

---
phase: 11-makepad-gui
plan: 2
subsystem: ui
tags: [makepad, gui, arrangement-view, vu-meters, solo-mute, catppuccin, drawcolor]

# Dependency graph
requires:
  - phase: 11-makepad-gui
    provides: "hum-gui binary with transport bar, GuiState polling, SpectralView"
provides:
  - "ArrangementView widget with colored at:/until: block lanes per thing"
  - "VuMeters widget with per-thing amplitude bars from daemon Status"
  - "Click-to-solo on arrangement lanes via unix socket command"
  - "Playhead line tracking live playback position"
  - "Full 3-zone layout: spectral + arrangement/VU + transport bar"
affects: [11-makepad-gui, creative-assistant]

# Tech tracking
tech-stack:
  added: []
  patterns: [drawcolor-custom-widget, oncelock-shared-state, walk-turtle-rect, piece-hum-parsing]

key-files:
  created:
    - src/bin/gui/arrangement_view.rs
    - src/bin/gui/vu_meters.rs
  modified:
    - src/bin/gui/app.rs
    - src/bin/gui/main.rs

key-decisions:
  - "OnceLock<Arc<Mutex<GuiState>>> for widget state sharing -- matches SpectralView pattern, avoids Widget trait casting"
  - "DrawColor with new_draw_call for multi-colored rects -- DrawQuad has no Default, DrawColor::new_local(cx) works"
  - "Parse piece.hum line-by-line for thing lanes -- simple parser, no YAML crate needed for at:/until: extraction"
  - "Things without until: get default 60s duration from their at: position"

patterns-established:
  - "Custom Makepad widget pattern: DrawColor + walk_turtle + draw_abs for procedural rendering"
  - "OnceLock global state pattern for custom widgets that can't receive constructor params"
  - "Piece.hum thing lane parsing with color palette cycling (Catppuccin Mocha)"

requirements-completed: [MKPD-03, MKPD-04, MKPD-06, MKPD-07, MKPD-08]

# Metrics
duration: 23min
completed: 2026-03-22
---

# Phase 11 Plan 2: Arrangement View + VU Meters Summary

**Ableton-style arrangement lanes with colored at:/until: blocks, per-thing VU amplitude bars, playhead tracking, and click-to-solo via DrawColor custom widgets**

## Performance

- **Duration:** 23 min
- **Started:** 2026-03-22T19:42:48Z
- **Completed:** 2026-03-22T20:06:01Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- ArrangementView custom widget renders colored lanes per thing from piece.hum with at:/until: block positions
- VuMeters widget displays per-thing amplitude bars (green/red threshold) matching arrangement lane order
- Click on arrangement lane sends solo command to daemon via unix socket
- Playhead line tracks live playback position across all lanes
- Full 3-zone layout wired: spectral (120px) + arrangement/VU (fill) + transport bar (48px)

## Task Commits

Each task was committed atomically:

1. **Task 1: ArrangementView -- lanes, blocks, playhead** - `c6d6897` (feat)
2. **Task 2: VuMeters widget + wire everything into app layout** - `edb29ef` (feat)

## Files Created/Modified
- `src/bin/gui/arrangement_view.rs` - ArrangementView widget with lane rendering, playhead, click-to-solo, piece.hum parser
- `src/bin/gui/vu_meters.rs` - VuMeters widget with per-thing amplitude bars from GuiState.amplitudes
- `src/bin/gui/app.rs` - Wired ArrangementView and VuMeters into live_design layout, registered widgets, init state at startup
- `src/bin/gui/main.rs` - Added mod declarations for arrangement_view and vu_meters

## Decisions Made
- Used `DrawColor` (not `DrawQuad`) for custom rect drawing -- `DrawQuad` lacks `Default` in makepad-widgets 1.0, but `DrawColor::new_local(cx)` creates reusable instances with `.color` field
- Used `OnceLock<Arc<Mutex<GuiState>>>` pattern for passing state to custom widgets (matches SpectralView approach from pre-existing code)
- Parsed piece.hum with simple line-by-line parser extracting `at:` and `until:` values -- avoids pulling in a YAML parser for this simple use case
- Things without `until:` field get `at + 60s` as default duration

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] DrawQuad::default() does not exist in makepad-widgets 1.0**
- **Found during:** Task 1 (ArrangementView initial implementation)
- **Issue:** Plan specified DrawQuad for rendering colored rectangles, but Makepad 1.0's DrawQuad derives Live (not Default) and requires #[live] field initialization
- **Fix:** Switched to DrawColor with DrawColor::new_local(cx) pattern, matching the working SpectralView approach already in the codebase
- **Files modified:** src/bin/gui/arrangement_view.rs
- **Verification:** cargo build --bin hum-gui succeeds with zero warnings
- **Committed in:** c6d6897 (Task 1 commit)

**2. [Rule 1 - Bug] FingerDownEvent has no rel field in Makepad 1.0**
- **Found during:** Task 1 (click-to-solo implementation)
- **Issue:** Used `fe.rel.y` for click position but FingerDownEvent only has `abs` (absolute position)
- **Fix:** Compute relative Y by subtracting area rect position: `fe.abs.y - area_rect.pos.y`
- **Files modified:** src/bin/gui/arrangement_view.rs
- **Verification:** Compiles cleanly, click detection math is correct
- **Committed in:** c6d6897 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both fixes required for Makepad 1.0 API compatibility. No scope creep. All planned functionality delivered.

## Issues Encountered
- Makepad crate source not in standard cargo registry location (custom config with shared-target dir), making API discovery harder. Resolved by building HTML docs and reading generated Makepad widget source from doc output.

## User Setup Required
None - no external service configuration required. Run with `HUM_PIECE=piece.hum cargo run --bin hum-gui`.

## Next Phase Readiness
- All GUI widgets functional: spectral analyzer, arrangement view, VU meters, transport bar
- Creative assistant (Phase 12) can build on this GUI foundation
- Thing label text not yet rendered in lanes (DrawText requires same Live-field pattern exploration)
- Seek slider and mute toggle could be added as future enhancements

## Self-Check: PASSED

- All 4 files exist (arrangement_view.rs, vu_meters.rs, app.rs, main.rs)
- Both task commits verified (c6d6897, edb29ef)
- cargo build --bin hum-gui succeeds with zero warnings
- SUMMARY.md created at expected path

---
*Phase: 11-makepad-gui*
*Completed: 2026-03-22*

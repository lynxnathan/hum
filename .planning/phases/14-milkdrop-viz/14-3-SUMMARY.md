---
phase: 14-milkdrop-viz
plan: 3
subsystem: ui
tags: [makepad, fft, beat-detection, spectral-flux, visualizer, catppuccin]

# Dependency graph
requires:
  - phase: 14-milkdrop-viz
    provides: VisualizerView widget with 4 presets, FFT polling, FftState
provides:
  - BeatDetector with spectral flux onset detection (beat_energy 0..1)
  - Extended FftState with beat_energy and per_thing amplitude slots
  - Beat reactivity across all 4 visualizer presets
  - Per-thing amplitude indicator dots at bottom edge of visualizer
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns: [spectral flux onset detection, OnceLock GUI_STATE_REF for cross-thread per-thing amplitude reads, per-thing fixed-size slots with name bytes]

key-files:
  created: [src/bin/gui/beat_detector.rs]
  modified: [src/bin/gui/transport_client.rs, src/bin/gui/visualizer.rs, src/bin/gui/main.rs]

key-decisions:
  - "OnceLock GUI_STATE_REF instead of changing start_fft_polling signature -- avoids modifying app.rs (Plan 2 owns that file), matches codebase OnceLock pattern"
  - "Fixed-size (f32, [u8; 32]) x 8 array for per-thing slots -- avoids heap allocation in hot FFT polling path"
  - "Spectral flux with 43-frame rolling mean (~1.4s at 30fps) and 1.5x threshold -- balances sensitivity vs false positives"

patterns-established:
  - "GUI_STATE_REF OnceLock: set during start_polling(), read by FFT polling thread for per-thing amplitudes"
  - "Beat energy decay: 0.82 multiplier per frame (~200ms decay at 30fps), denormal floor at 0.001"

requirements-completed: [VIZ-04, VIZ-05]

# Metrics
duration: 5min
completed: 2026-03-22
---

# Phase 14 Plan 3: Beat Detection + Per-Thing Amplitude Summary

**Spectral flux beat detection with smooth decay driving 4 visualizer presets + per-thing Catppuccin-colored amplitude dots**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-22T21:57:42Z
- **Completed:** 2026-03-22T22:02:42Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- BeatDetector module with spectral flux onset detection, 43-frame rolling mean, adaptive threshold
- FftState extended with beat_energy (0..1 decaying) and per_thing amplitude array (8 slots)
- All 4 presets react to beats: waveform vertical scale, spectrum white flash, plasma luminance burst, tunnel center ring burst
- Per-thing amplitude dots at bottom edge with 8 Catppuccin accent colors, radius/alpha proportional to amplitude

## Task Commits

Each task was committed atomically:

1. **Task 1: BeatDetector + extend FftState + wire into polling** - `9e12aec` (feat)
2. **Task 2: Beat + per-thing uniforms in VisualizerView shader** - `e12f58a` (feat)

## Files Created/Modified
- `src/bin/gui/beat_detector.rs` - BeatDetector struct with spectral flux onset detection, circular energy history buffer, smooth decay
- `src/bin/gui/transport_client.rs` - Extended FftState with beat_energy and per_thing fields, GUI_STATE_REF OnceLock, BeatDetector wired into FFT polling loop, read_per_thing_amps helper
- `src/bin/gui/visualizer.rs` - All 4 presets receive beat parameter with distinct visual reactions, per-thing dot overlay with THING_COLORS palette, 5 new Catppuccin palette constants
- `src/bin/gui/main.rs` - Added `mod beat_detector` declaration

## Decisions Made
- **OnceLock for GuiState access:** Used `GUI_STATE_REF` OnceLock (set in `start_polling`) instead of adding a second parameter to `start_fft_polling`. This avoids modifying `app.rs` which Plan 2 owns, and matches the existing OnceLock pattern throughout the codebase.
- **Fixed-size per-thing array:** Used `[(f32, [u8; 32]); 8]` instead of `Vec<(String, f32)>` to avoid heap allocation in the hot FFT polling path. 8 slots is sufficient for typical compositions.
- **Beat detection parameters:** 43-frame history (~1.4s), 1.5x mean threshold, 0.82 decay rate. These values produce ~200ms decay time at 30fps polling rate.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] OnceLock pattern instead of changing start_fft_polling signature**
- **Found during:** Task 1
- **Issue:** Plan specified adding `Arc<Mutex<GuiState>>` parameter to `start_fft_polling` and updating `app.rs` call site, but user constraint says not to modify `app.rs` (Plan 2 owns it)
- **Fix:** Used `GUI_STATE_REF` OnceLock set inside `start_polling()` (which already receives GuiState), read by FFT polling thread
- **Files modified:** src/bin/gui/transport_client.rs
- **Verification:** `cargo check --bin hum-gui` passes
- **Committed in:** 9e12aec

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary to avoid app.rs conflicts with Plan 2. OnceLock approach is cleaner and consistent with codebase patterns.

## Issues Encountered
None - both tasks compiled cleanly on first attempt.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Beat detection and per-thing amplitude routing fully operational
- Visualizer now has complete audio reactivity: FFT bins, band energy, beat transients, and per-thing ownership
- spectral_view.rs still exists as dead code (cleanup deferred to separate task)

---
*Phase: 14-milkdrop-viz*
*Completed: 2026-03-22*

---
phase: 14-milkdrop-viz
plan: 1
subsystem: ui
tags: [makepad, shader, fft, visualizer, catppuccin]

# Dependency graph
requires:
  - phase: 11-makepad-gui
    provides: Makepad app shell, SpectralView FFT pipeline, transport bar
provides:
  - VisualizerView widget with 4 switchable presets (waveform, spectrum, plasma, tunnel)
  - AtomicU8 global preset selector pattern for cross-widget communication
  - Preset selector buttons in transport bar
affects: [14-milkdrop-viz plan 2 (feedback textures), 14-milkdrop-viz plan 3 (per-thing routing)]

# Tech tracking
tech-stack:
  added: []
  patterns: [CPU-rendered shader-like presets via DrawColor grid, AtomicU8 global for lock-free preset switching]

key-files:
  created: [src/bin/gui/visualizer.rs]
  modified: [src/bin/gui/app.rs, src/bin/gui/main.rs]

key-decisions:
  - "CPU-rendered presets using DrawColor grid (64x32 cells) instead of GPU shader uniforms -- avoids Makepad DrawShader API uncertainty"
  - "AtomicU8 global for preset switching instead of WidgetRef borrow_mut -- simpler, lock-free, matches codebase OnceLock pattern"
  - "Separate OnceLock (FFT_STATE_VIZ) for visualizer instead of re-using spectral_view's -- cleaner module boundary"

patterns-established:
  - "AtomicU8 global pattern: write from button handlers, read each frame in draw_walk -- avoids Makepad WidgetRef lifetime issues"
  - "CPU pixel grid rendering: 64x32 DrawColor cells for plasma/tunnel effects -- ~2048 draw calls per frame, acceptable at 30fps"

requirements-completed: [VIZ-01, VIZ-02]

# Metrics
duration: 5min
completed: 2026-03-22
---

# Phase 14 Plan 1: VisualizerView Summary

**4-preset FFT visualizer (waveform, spectrum, plasma, tunnel) with runtime preset switching via transport bar buttons**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-22T21:42:51Z
- **Completed:** 2026-03-22T21:48:09Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- VisualizerView widget with 4 distinct presets driven by FFT band energy (bass/mids/highs/volume)
- Replaced SpectralView (120px bars) with VisualizerView (220px animated presets)
- Preset selector buttons (WAVE, SPEC, PLASMA, TUNNEL) in transport bar with instant switching

## Task Commits

Each task was committed atomically:

1. **Task 1: VisualizerView widget with 4 shader presets** - `7a1dc6c` (feat)
2. **Task 2: Wire VisualizerView into app.rs with preset selector** - `9ac2ba1` (feat)

## Files Created/Modified
- `src/bin/gui/visualizer.rs` - VisualizerView widget with VisualizerPreset enum, 4 CPU-rendered presets, FFT band computation, AtomicU8 global preset selector
- `src/bin/gui/app.rs` - Replaced SpectralView with VisualizerView in layout, added 4 preset buttons to transport bar, wired button handlers to set_active_preset
- `src/bin/gui/main.rs` - Added `mod visualizer;` declaration

## Decisions Made
- **CPU rendering over GPU shaders:** Used DrawColor grid (64x32 cells for plasma/tunnel, 128 columns for waveform, 64 bars for spectrum) instead of custom Makepad shader uniforms. Makepad's DrawShader API for custom uniforms/textures is not well-documented and the DrawColor per-rect approach is proven in the existing codebase. This can be upgraded to GPU shaders in Plan 2.
- **AtomicU8 global for preset state:** Makepad's WidgetRef borrow_mut has lifetime constraints that make cross-widget method calls awkward. A global AtomicU8 (0-3 mapping to preset enum) is lock-free, simple, and matches the existing OnceLock pattern used for FFT state, VU state, and arrangement state.
- **Separate FFT OnceLock:** Created FFT_STATE_VIZ in visualizer.rs rather than re-exporting spectral_view's FFT_STATE. Keeps module boundaries clean and allows spectral_view.rs to be removed later.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] WidgetRef borrow_mut lifetime error**
- **Found during:** Task 2 (wiring preset buttons)
- **Issue:** `viz_ref.borrow_mut::<VisualizerView>()` produced a temporary that didn't live long enough due to Makepad's WidgetRef lifetime constraints
- **Fix:** Replaced WidgetRef borrow approach with AtomicU8 global preset selector -- button handlers write to atomic, VisualizerView reads each frame in draw_walk
- **Files modified:** src/bin/gui/visualizer.rs, src/bin/gui/app.rs
- **Verification:** `cargo build --bin hum-gui` succeeds
- **Committed in:** 9ac2ba1

**2. [Rule 1 - Bug] Removed unused re-export causing dead code**
- **Found during:** Task 2 (cleanup)
- **Issue:** `pub use crate::spectral_view::init_fft_state` re-export was unnecessary since visualizer has its own `init_viz_fft_state`, and `set_preset` method was dead code after switching to atomic pattern
- **Fix:** Removed unused re-export and set_preset method
- **Files modified:** src/bin/gui/visualizer.rs
- **Committed in:** 9ac2ba1

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both auto-fixes necessary for compilation. AtomicU8 approach is actually cleaner than the planned WidgetRef pattern. No scope creep.

## Issues Encountered
- Makepad cargo registry sources not cached in WSL2 environment, so could not inspect DrawShader API directly. Used the proven DrawColor approach from existing widgets instead.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- VisualizerView ready for Plan 2 (feedback textures, shader live-reload)
- Plan 2 can upgrade CPU rendering to GPU shaders if Makepad DrawShader API is confirmed
- spectral_view.rs still exists but is no longer registered in live_design -- can be removed in a cleanup task

## Self-Check: PASSED

- FOUND: src/bin/gui/visualizer.rs
- FOUND: commit 7a1dc6c (Task 1)
- FOUND: commit 9ac2ba1 (Task 2)

---
*Phase: 14-milkdrop-viz*
*Completed: 2026-03-22*

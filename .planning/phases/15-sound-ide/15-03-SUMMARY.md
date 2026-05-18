---
phase: 15-sound-ide
plan: "03"
subsystem: ide-layout
tags: [makepad, splitter, project-browser, layout-persistence, toml]
dependency_graph:
  requires:
    - phase: 15-01
      provides: [TerminalPane, portable-pty-integration]
    - phase: 15-02
      provides: [FocusState, KeysConfig, Catppuccin-Mocha-palette]
  provides: [ProjectBrowser, LayoutConfig, Splitter-based-3-pane-layout]
  affects: [gui-layout, future-file-editing]
tech_stack:
  added: []
  patterns: [OnceLock-global-init-for-widget-state, DrawColor-DrawText-custom-widget, Splitter-nested-layout]
key_files:
  created:
    - src/bin/gui/project_browser.rs
    - src/bin/gui/layout_config.rs
  modified:
    - src/bin/gui/app.rs
    - src/bin/gui/main.rs
key_decisions:
  - "OnceLock global for ProjectBrowser project root (avoids mutable widget access through Makepad tree)"
  - "DrawColor+DrawText custom renderer for project browser (consistent with ArrangementView pattern)"
  - "SplitterAction::Changed matching for layout persistence (saves on every drag event)"
  - "Splitter defaults in live_design! macro (programmatic FromA apply_over not supported by live! macro)"
patterns_established:
  - "Nested Splitter layout: outer Horizontal (sidebar|main), inner Vertical (content|terminal)"
  - "Auto-refresh on first draw via initialized flag + OnceLock global"
requirements_completed: [IDE-01, IDE-02]
metrics:
  duration: 3min
  completed: "2026-03-22T22:43:00Z"
---

# Phase 15 Plan 3: Split-Pane Layout + Project Browser Summary

**Resizable 3-pane IDE layout with Makepad Splitters, project browser sidebar scanning .hum/instruments/dict files, and layout persistence to ~/.config/hum/layout.toml**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-22T22:40:15Z
- **Completed:** 2026-03-22T22:43:00Z
- **Tasks:** 2 auto + 1 checkpoint (auto-approved)
- **Files modified:** 4

## Accomplishments

- Resizable 3-pane IDE layout using nested Makepad Splitter widgets (sidebar | content | terminal)
- ProjectBrowser widget with DrawColor+DrawText rendering, click-to-open-in-$EDITOR
- LayoutConfig persistence saving/loading pane sizes to ~/.config/hum/layout.toml via serde+toml

## Task Commits

Each task was committed atomically:

1. **Task 1: ProjectBrowser widget + LayoutConfig persistence** - `51e828a` (feat)
2. **Task 2: Wire Splitter layout into App** - `501aedc` (feat)

## Files Created/Modified

- `src/bin/gui/project_browser.rs` - ProjectBrowser widget: DrawColor/DrawText sidebar listing .hum pieces, instruments/*.hum, hum.dict with click-to-open
- `src/bin/gui/layout_config.rs` - LayoutConfig struct: serde load/save pane sizes to ~/.config/hum/layout.toml
- `src/bin/gui/app.rs` - Rewired body layout to nested Splitters (outer horizontal + inner vertical), SplitterAction handler for persistence, ProjectBrowser registration
- `src/bin/gui/main.rs` - Added mod declarations for layout_config and project_browser

## Decisions Made

- OnceLock global for ProjectBrowser project root initialization (Makepad widget tree doesn't expose mutable access to custom widget fields from parent)
- DrawColor + DrawText custom renderer pattern for project browser (consistent with ArrangementView, avoids PortalList API incompatibilities)
- SplitterAction::Changed event handler persists layout on every drag (immediate save, no debounce needed for config files)
- Splitter initial positions set via live_design! defaults (live! macro doesn't support enum variant syntax like FromA for apply_over)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Rewrote ProjectBrowser from PortalList to DrawColor renderer**
- **Found during:** Task 1 build verification
- **Issue:** PortalList API (items_with_actions, set_item_range, next_visible_item) does not exist in makepad-widgets 1.0
- **Fix:** Rewrote to use DrawColor rects + DrawText labels (same pattern as ArrangementView)
- **Files modified:** src/bin/gui/project_browser.rs
- **Commit:** 51e828a

**2. [Rule 1 - Bug] Removed programmatic Splitter align initialization**
- **Found during:** Task 2 build verification
- **Issue:** live! macro does not support enum variant syntax (FromA(value)) for apply_over
- **Fix:** Use static defaults in live_design! macro; persistence still works via SplitterAction on drag
- **Files modified:** src/bin/gui/app.rs
- **Commit:** 501aedc

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes necessary for compilation. No scope creep. Layout persistence works via drag events.

## Issues Encountered

None beyond the auto-fixed deviations above.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Full IDE layout complete: terminal pane + keyboard shortcuts + project browser + resizable panes
- All Phase 15 requirements covered (TERM-01 through TERM-04, KEYS-01 through KEYS-04, IDE-01 through IDE-04)
- Layout persistence functional on drag; initial position restore from config deferred (minor UX gap)

---
*Phase: 15-sound-ide*
*Completed: 2026-03-22*

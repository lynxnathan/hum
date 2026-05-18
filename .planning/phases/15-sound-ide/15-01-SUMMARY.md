---
phase: 15-sound-ide
plan: "01"
subsystem: terminal-pane
tags: [pty, terminal, makepad-widget, gui]
dependency_graph:
  requires: []
  provides: [TerminalPane, portable-pty-integration]
  affects: [app-layout, gui-main]
tech_stack:
  added: [portable-pty-0.8]
  patterns: [pty-reader-thread, arc-mutex-char-grid, ansi-strip, drawcolor-cell-renderer]
key_files:
  created:
    - src/bin/gui/terminal_pane.rs
  modified:
    - Cargo.toml
    - src/bin/gui/app.rs
    - src/bin/gui/main.rs
decisions:
  - TERM=dumb + PS1="$ " to minimize ANSI noise from shell
  - DrawColor cell renderer (colored rects) instead of DrawText for v1 char grid
  - ANSI strip via state-machine parser (handles CSI, OSC sequences)
  - Click-to-focus model with mouse selection for copy support
  - PTY started on first event dispatch rather than after_new_from_doc (avoids lifetime issues)
metrics:
  duration: 4min
  completed: "2026-03-22T22:33:00Z"
---

# Phase 15 Plan 1: PTY Terminal Pane Summary

PTY-backed terminal pane embedded in Makepad window using portable-pty, rendering char grid via DrawColor cells with keyboard input, scroll, and copy support.

## What Was Built

### Task 1: portable-pty dependency + TerminalPane skeleton
**Commit:** `509615b`

- Added `portable-pty = "0.8"` to Cargo.toml
- Created `src/bin/gui/terminal_pane.rs` (575 lines):
  - `TerminalPane` widget struct with `Live, LiveHook, Widget` derives
  - PTY spawn via `portable_pty::native_pty_system()` with `$SHELL`
  - Reader thread: reads PTY output, strips ANSI escapes (CSI + OSC), writes to `Arc<Mutex<Vec<Vec<(char, rgba)>>>>`
  - Scrollback capped at 1000 lines
  - ANSI stripping handles ESC[...letter, ESC]...BEL, and other escape sequences
- Declared `mod terminal_pane` in main.rs

### Task 2: Wire TerminalPane into app layout
**Commit:** `2ec42b1`

- Registered `terminal_pane::live_design(cx)` in App `LiveRegister`
- Added `use crate::terminal_pane::TerminalPane` to `live_design!` macro
- Placed `<TerminalPane>` widget below `mid_zone`, above `transport_bar` (height: 200)
- Widget receives events via standard Makepad dispatch tree

## Key Implementation Details

**Rendering:** DrawColor cell grid (80x24 visible cells). Each non-space character rendered as a colored rect. Cursor displayed as thin vertical bar when focused.

**Input handling:** Full keyboard map (a-z, 0-9, symbols, shift variants). Enter sends `\r`, Backspace sends `\x7f`, Ctrl+C sends `\x03` (or copies if selection active), Ctrl+D sends `\x04`. Arrow keys send ANSI escape sequences.

**Copy support (TERM-04):** Mouse click starts selection, drag extends it. Ctrl+C with active selection copies to clipboard via `cx.copy_to_clipboard()`.

**Scroll:** Mouse wheel scrolls by 3 lines, clamped to grid bounds. Default auto-scrolls to bottom.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed KeyCode variant names**
- **Found during:** Task 1 build verification
- **Issue:** `KeyCode::OpenBracket` and `KeyCode::CloseBracket` don't exist in makepad-widgets 1.0
- **Fix:** Changed to `KeyCode::LBracket` and `KeyCode::RBracket`
- **Files modified:** src/bin/gui/terminal_pane.rs
- **Commit:** 509615b

**2. [Rule 1 - Bug] Fixed borrow checker error in mouse move handler**
- **Found during:** Task 1 build verification
- **Issue:** Simultaneous mutable borrow of `self.selection` and immutable borrow of `self` via `pos_to_grid()`
- **Fix:** Moved `pos_to_grid()` call before the `if let Some(ref mut sel)` block
- **Files modified:** src/bin/gui/terminal_pane.rs
- **Commit:** 509615b

**3. [Rule 3 - Blocking] Added module declaration in Task 1**
- **Found during:** Task 1 verification
- **Issue:** Could not verify terminal_pane.rs compiles without `mod terminal_pane` in main.rs
- **Fix:** Added module declaration early (plan had it in Task 2)
- **Files modified:** src/bin/gui/main.rs
- **Commit:** 509615b

## Verification

- `cargo build --bin hum-gui` passes with zero errors
- No warnings from terminal_pane.rs or app.rs changes
- Terminal pane placed in layout between mid_zone and transport_bar

## Requirements Coverage

| Requirement | Status | How |
|-------------|--------|-----|
| TERM-01: PTY terminal in Makepad | Done | TerminalPane widget with portable-pty |
| TERM-02: Shell runs inside pane | Done | $SHELL spawned via CommandBuilder |
| TERM-03: Keyboard input to PTY | Done | Full keymap in handle_event |
| TERM-04: Copy from terminal | Done | Mouse selection + Ctrl+C clipboard |

## Self-Check: PASSED

- FOUND: src/bin/gui/terminal_pane.rs
- FOUND: commit 509615b (Task 1)
- FOUND: commit 2ec42b1 (Task 2)
- FOUND: portable-pty in Cargo.toml
- FOUND: terminal_pane wired in app.rs

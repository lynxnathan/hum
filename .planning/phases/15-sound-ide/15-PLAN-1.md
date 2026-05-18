---
phase: 15-sound-ide
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - Cargo.toml
  - src/bin/gui/terminal_pane.rs
  - src/bin/gui/main.rs
autonomous: true
requirements: [TERM-01, TERM-02, TERM-03, TERM-04]

must_haves:
  truths:
    - "A terminal pane renders inside the Makepad window showing shell output"
    - "The default shell ($SHELL) runs inside the pane via PTY"
    - "User can type into the terminal and commands execute"
    - "Text can be copied from terminal output (select + Ctrl-C)"
    - "Terminal pane is a Makepad widget registered via live_design!"
  artifacts:
    - path: "src/bin/gui/terminal_pane.rs"
      provides: "TerminalPane widget: PTY thread + char grid rendering"
      exports: ["TerminalPane", "live_design"]
    - path: "Cargo.toml"
      provides: "portable-pty dependency"
      contains: "portable-pty"
  key_links:
    - from: "src/bin/gui/terminal_pane.rs"
      to: "portable_pty::CommandBuilder"
      via: "spawn PTY with $SHELL"
      pattern: "CommandBuilder::new"
    - from: "src/bin/gui/terminal_pane.rs"
      to: "Makepad draw_text"
      via: "render char grid on draw_walk"
      pattern: "draw_walk"
---

<objective>
Embed a PTY-backed terminal pane inside the Makepad window.

Purpose: Users run Claude Code (and any shell command) from inside hum-gui without switching windows.
Output: TerminalPane widget using portable-pty, rendering a character grid in Makepad.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@src/bin/gui/app.rs
@src/bin/gui/transport_client.rs
@Cargo.toml

<interfaces>
<!-- Existing Makepad widget pattern from codebase -->

From src/bin/gui/app.rs (pattern for new widgets):
```rust
// Registration in LiveRegister:
crate::terminal_pane::live_design(cx);

// In live_design! macro:
use crate::terminal_pane::TerminalPane;
terminal = <TerminalPane> { width: Fill, height: 220 }

// Widget access:
self.ui.widget(id!(terminal))...
```

From Cargo.toml (add alongside makepad-widgets):
```toml
portable-pty = "0.8"
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add portable-pty dependency + TerminalPane skeleton</name>
  <files>Cargo.toml, src/bin/gui/terminal_pane.rs</files>
  <action>
    1. Add to Cargo.toml dependencies: `portable-pty = "0.8"`

    2. Create src/bin/gui/terminal_pane.rs with:
       - `TerminalPane` struct deriving `Live, LiveHook, Widget`
       - State fields (all `#[rust]`):
         - `pty_writer: Option<Box<dyn Write + Send>>` — stdin to PTY
         - `char_grid: Arc<Mutex<Vec<Vec<(char, [f32;4])>>>>`  — (char, rgba) grid, 80x24
         - `input_buf: String` — pending keystrokes
         - `scroll_offset: usize` — lines scrolled
         - `has_focus: bool`
       - `live_design!` block: declare `TerminalPane = {{TerminalPane}} {}` with a dark background `#1e1e2e`
       - `pub fn live_design(cx: &mut Cx)` registration function
       - `impl LiveRegister` stub
       - Spawn PTY thread in a `fn start_pty(&mut self)` method:
         ```
         let pty_system = portable_pty::native_pty_system();
         let pair = pty_system.openpty(PtySize { rows: 24, cols: 80, .. })?;
         let shell = std::env::var("SHELL").unwrap_or("/bin/bash".into());
         let cmd = CommandBuilder::new(shell);
         let _child = pair.slave.spawn_command(cmd)?;
         self.pty_writer = Some(pair.master.take_writer()?);
         // spawn reader thread → writes lines into char_grid Arc<Mutex<...>>
         ```
         The reader thread: reads bytes from `pair.master.try_clone_reader()`, does basic ANSI stripping (remove ESC sequences with regex `\x1b\[[0-9;]*[A-Za-z]`), appends to grid rows, caps at 1000 rows.
       - Call `start_pty` from `fn after_new_from_doc(&mut self, _cx: &mut Cx)` (LiveHook)
  </action>
  <verify>
    <automated>cargo build --bin hum-gui 2>&1 | grep -E "^error" | head -20</automated>
  </verify>
  <done>Cargo.toml has portable-pty, terminal_pane.rs compiles, TerminalPane struct exists</done>
</task>

<task type="auto">
  <name>Task 2: Implement character grid rendering and keyboard input</name>
  <files>src/bin/gui/terminal_pane.rs, src/bin/gui/app.rs, src/bin/gui/main.rs</files>
  <action>
    In terminal_pane.rs:

    **Rendering (impl Widget for TerminalPane):**
    - `draw_walk`: lock `char_grid`, iterate visible rows (scroll_offset .. scroll_offset+24), for each row call `cx.draw_text` with monospace font (size 12.0) rendering each character. Use foreground color from stored rgba or default TEXT_COLOR `#cdd6f4`. Draw cursor (a filled rect) at end of last row when `has_focus=true`.
    - `walk_from_doc`: return `Walk { width: Fill, height: 220.0 }` (resizable in plan 3)

    **Keyboard input (impl MatchEvent or handle_event on TerminalPane):**
    - On `Event::KeyDown` when `has_focus`:
      - Printable chars → append to `input_buf`, write to `pty_writer`
      - Enter → write `\r` to PTY, clear `input_buf`
      - Backspace → write `\x7f` to PTY
      - Ctrl+C → write `\x03`
      - Ctrl+D → write `\x04`

    **Copy support (TERM-04):**
    - Track `selection: Option<(usize, usize)>` (start/end char index in flat grid)
    - On Ctrl+C when text selected → `cx.set_clipboard(selected_text)`

    **Wire into App:**
    - In `src/bin/gui/main.rs` `LiveRegister::live_register`: add `crate::terminal_pane::live_design(cx);`
    - In `src/bin/gui/app.rs` `live_design!`:
      - Add `use crate::terminal_pane::TerminalPane;` at top of live_design! block
      - Add terminal pane to layout inside `body` View, below `mid_zone`, before `transport_bar`:
        ```
        terminal = <TerminalPane> {
            width: Fill
            height: 200
        }
        ```
    - In `App::handle_event`, forward `Event::KeyDown` to terminal when it has focus:
      `self.ui.widget(id!(terminal)).handle_event(cx, event, &mut Scope::empty());`

    Do NOT implement resizing (that is plan 3). Do NOT implement Escape focus toggle (that is plan 2).
  </action>
  <verify>
    <automated>cargo build --bin hum-gui 2>&1 | grep -E "^error" | head -20</automated>
  </verify>
  <done>hum-gui builds cleanly. Terminal pane appears in window. Shell prompt visible. Typing characters sends them to PTY. Output from commands appears in grid.</done>
</task>

</tasks>

<verification>
```
cargo build --bin hum-gui
WAYLAND_DISPLAY="" hum-gui   # visual check: terminal pane shows shell prompt
```
Type `echo hello` in terminal pane — output appears. Ctrl+C sends interrupt.
</verification>

<success_criteria>
- cargo build passes with zero errors
- Terminal pane visible in Makepad window showing $SHELL prompt
- Keystrokes reach PTY, output renders in char grid
- Copy via Ctrl+C when text selected works
</success_criteria>

<output>
After completion, create `.planning/phases/15-sound-ide/15-01-SUMMARY.md`
</output>

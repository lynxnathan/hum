---
phase: 15-sound-ide
plan: 03
type: execute
wave: 3
depends_on: [15-01, 15-02]
files_modified:
  - src/bin/gui/app.rs
  - src/bin/gui/project_browser.rs
  - src/bin/gui/layout_config.rs
autonomous: false
requirements: [IDE-01, IDE-02, TERM-03]

must_haves:
  truths:
    - "Three panes (visualizer, arrangement+terminal, project browser) separated by draggable Splitter widgets"
    - "Terminal pane height is resizable by dragging a divider"
    - "Project browser sidebar lists .hum pieces, instruments/, and hum.dict"
    - "Clicking a file in the project browser opens it (via $EDITOR or terminal cd)"
    - "Pane sizes persist across restarts in a layout config file"
  artifacts:
    - path: "src/bin/gui/project_browser.rs"
      provides: "ProjectBrowser widget: file tree of pieces, instruments, dict"
      exports: ["ProjectBrowser", "live_design"]
    - path: "src/bin/gui/layout_config.rs"
      provides: "LayoutConfig: load/save pane sizes to ~/.config/hum/layout.toml"
      exports: ["LayoutConfig"]
  key_links:
    - from: "src/bin/gui/app.rs"
      to: "src/bin/gui/project_browser.rs"
      via: "live_design! sidebar pane with Splitter"
      pattern: "ProjectBrowser"
    - from: "src/bin/gui/layout_config.rs"
      to: "~/.config/hum/layout.toml"
      via: "toml::to_string + fs::write on resize events"
      pattern: "layout.toml"
---

<objective>
Add resizable split-pane layout with a project browser sidebar. Pane sizes persist.

Purpose: IDE-quality layout — visualizer on top, arrangement+terminal center, project browser sidebar. Everything resizable.
Output: ProjectBrowser widget, LayoutConfig persistence, Makepad Splitter wiring in App layout.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@src/bin/gui/app.rs

<interfaces>
<!-- Makepad Splitter widget pattern -->
Makepad provides a `Splitter` widget for resizable panes. Usage in live_design!:
```
// Horizontal split (left|right)
<Splitter> {
    axis: Horizontal,
    align: FromStart(200.0),    // initial left width
    a: <LeftView> { ... },
    b: <RightView> { ... }
}

// Vertical split (top|bottom)
<Splitter> {
    axis: Vertical,
    align: FromStart(220.0),    // initial top height
    a: <TopView> { ... },
    b: <BottomView> { ... }
}
```
Handle resize in handle_actions:
```rust
if let Some(delta) = self.ui.splitter(id!(main_split)).dragged(actions) {
    // delta.x or delta.y contains new split position
    self.layout_cfg.browser_width = new_width;
    self.layout_cfg.save();
}
```

From plan 15-01 / 15-02:
- TerminalPane widget: `id!(terminal)`, `has_focus: bool`
- ProjectBrowser will be new, registered same way as TerminalPane
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: ProjectBrowser widget + LayoutConfig persistence</name>
  <files>src/bin/gui/project_browser.rs, src/bin/gui/layout_config.rs</files>
  <action>
    **src/bin/gui/layout_config.rs:**
    ```rust
    #[derive(Serialize, Deserialize, Clone)]
    pub struct LayoutConfig {
        pub browser_width: f64,      // default 200.0
        pub terminal_height: f64,    // default 200.0
        pub visualizer_height: f64,  // default 220.0
    }
    impl Default for LayoutConfig { /* use the three defaults above */ }
    impl LayoutConfig {
        pub fn load() -> Self {
            let path = dirs::config_dir()
                .unwrap_or_default()
                .join("hum/layout.toml");
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| toml::from_str(&s).ok())
                .unwrap_or_default()
        }
        pub fn save(&self) {
            let path = dirs::config_dir()
                .unwrap_or_default()
                .join("hum/layout.toml");
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(s) = toml::to_string(self) {
                let _ = std::fs::write(path, s);
            }
        }
    }
    ```

    **src/bin/gui/project_browser.rs:**
    - `ProjectBrowser` struct deriving `Live, LiveHook, Widget`
    - State: `entries: Vec<BrowserEntry>`, `selected: Option<usize>`
    - `BrowserEntry { label: String, path: PathBuf, kind: EntryKind }` where `EntryKind` is `Piece | Instrument | Dict`
    - `live_design!` block: dark sidebar bg `#181825`, list of `<Label>` rows, selected row highlighted with SURFACE0 `#313244`
    - `fn refresh(&mut self, project_root: &Path)`:
      - Scan project_root for `*.hum` files → Piece entries
      - Scan `project_root/instruments/*.hum` → Instrument entries
      - Check `project_root/hum.dict` → Dict entry if exists
    - On entry click (hit-test in `draw_walk` or via button per row): open file:
      ```rust
      let editor = std::env::var("EDITOR").unwrap_or("nano".into());
      let _ = std::process::Command::new(&editor)
          .arg(&entry.path)
          .spawn();  // fire-and-forget, opens in background terminal or X window
      ```
    - `pub fn live_design(cx: &mut Cx)` registration
  </action>
  <verify>
    <automated>cargo build --bin hum-gui 2>&1 | grep -E "^error" | head -20</automated>
  </verify>
  <done>project_browser.rs and layout_config.rs compile. LayoutConfig loads/saves layout.toml. ProjectBrowser scans project dir and renders entry list.</done>
</task>

<task type="auto">
  <name>Task 2: Wire Splitter layout into App — resizable 3-pane IDE</name>
  <files>src/bin/gui/app.rs, src/bin/gui/main.rs</files>
  <action>
    **Register new widgets in LiveRegister:**
    ```rust
    crate::project_browser::live_design(cx);
    ```

    **Rewrite live_design! App body layout** to use Splitter widgets for resizable panes.
    Load layout_cfg from LayoutConfig::load() and use values for initial split positions.
    New structure (replaces current flat Down flow):

    ```
    App body = outer horizontal Splitter (browser | main):
      LEFT (browser):
        <ProjectBrowser> { width: 200, height: Fill }
      RIGHT (main): vertical Splitter (top | bottom):
        TOP (visualizer+arrangement): vertical Splitter:
          TOP: <VisualizerView> { height: 220 }
          BOTTOM: mid_zone <View> flow:Right:
            <ArrangementView> { width: Fill }
            <VuMeters> { width: 80 }
        BOTTOM (terminal):
          <TerminalPane> { width: Fill, height: 200 }
    ```

    Makepad live_design! pseudocode:
    ```
    body = <Splitter> {
        axis: Horizontal
        align: FromStart(200.0)   // browser_width from LayoutConfig
        a: <View> {
            browser = <ProjectBrowser> { width: Fill, height: Fill }
        }
        b: <Splitter> {
            axis: Vertical
            align: FromStart(420.0)  // visualizer_height + arrangement height
            a: <View> {
                flow: Down
                visualizer = <VisualizerView> { width: Fill, height: 220 }
                mid_zone = <View> {
                    flow: Right, width: Fill, height: Fill
                    arrangement = <ArrangementView> { width: Fill, height: Fill }
                    vu_meters = <VuMeters> { width: 80, height: Fill }
                }
            }
            b: <View> {
                flow: Down
                terminal = <TerminalPane> { width: Fill, height: Fill }
                transport_bar = <View> { ... }   // keep existing transport bar
            }
        }
    }
    ```

    **Handle Splitter drag in handle_actions:**
    ```rust
    // outer_split drag → update browser_width + save
    if self.ui.splitter(id!(outer_split)).dragged(actions).is_some() {
        // read new position and persist
        self.layout_cfg.save();
    }
    // inner_split drag → update terminal_height + save
    if self.ui.splitter(id!(inner_split)).dragged(actions).is_some() {
        self.layout_cfg.save();
    }
    ```

    **App struct:** Add `#[rust] layout_cfg: LayoutConfig` field. Load in `handle_startup`.
    Also call `browser.refresh(project_root)` in `handle_startup` where `project_root = std::env::current_dir().unwrap_or_default()`.

    **Add human verify checkpoint** — layout needs visual confirmation.
  </action>
  <verify>
    <automated>cargo build --bin hum-gui 2>&1 | grep -E "^error" | head -20</automated>
  </verify>
  <done>Build passes. App launches with 3-pane layout: sidebar browser, center arrangement+visualizer, bottom terminal.</done>
</task>

<task type="checkpoint:human-verify" gate="blocking">
  <what-built>Full IDE layout: project browser sidebar, resizable panes via Splitters, terminal pane at bottom, layout persistence to ~/.config/hum/layout.toml. Combined with plan 01 terminal PTY and plan 02 keyboard shortcuts and Catppuccin theme.</what-built>
  <how-to-verify>
    1. Run: `WAYLAND_DISPLAY="" hum-gui` (or `cargo run --bin hum-gui`)
    2. Confirm three panes visible: left sidebar (browser), center (visualizer + arrangement), bottom-right (terminal)
    3. Drag the vertical divider between browser and main area — panes resize
    4. Drag the horizontal divider above terminal — terminal height changes
    5. Close and reopen hum-gui — pane sizes match what you left them at (layout.toml persisted)
    6. Press Escape — status bar shows [TERM], spacebar types into terminal
    7. Press Escape again — status bar shows [GUI], spacebar plays/stops
    8. Project browser lists .hum files in current directory
    9. Status bar shows daemon state, active count, focus indicator
  </how-to-verify>
  <resume-signal>Type "approved" or describe issues found</resume-signal>
</task>

</tasks>

<verification>
```
cargo build --bin hum-gui  # must pass
WAYLAND_DISPLAY="" hum-gui  # visual verify per checkpoint above
cat ~/.config/hum/layout.toml  # must exist after dragging panes
```
</verification>

<success_criteria>
- 3-pane IDE layout renders without crash
- Splitter dividers are draggable and panes resize
- layout.toml created and survives restart
- Project browser lists project files
- All Phase 15 success criteria from ROADMAP met (terminal runs shell, Escape toggles focus, spacebar plays, panes resize, status bar visible)
</success_criteria>

<output>
After completion, create `.planning/phases/15-sound-ide/15-03-SUMMARY.md`
</output>

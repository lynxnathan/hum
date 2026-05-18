---
phase: 15-sound-ide
plan: 02
type: execute
wave: 2
depends_on: [15-01]
files_modified:
  - src/bin/gui/app.rs
  - src/bin/gui/key_handler.rs
  - src/config.rs
autonomous: true
requirements: [KEYS-01, KEYS-02, KEYS-03, KEYS-04, IDE-03, IDE-04]

must_haves:
  truths:
    - "Spacebar plays/stops when GUI has focus"
    - "R starts record mode (sends record cmd to daemon)"
    - "M mutes the focused thing"
    - "Number keys 1-9 solo the nth active thing by index"
    - "Escape toggles keyboard focus between terminal pane and GUI"
    - "Keybindings are readable from hum.toml [keys] section"
    - "Status bar shows daemon state, playback state, active count"
    - "Catppuccin Mocha color palette applied via live_design! color constants"
  artifacts:
    - path: "src/bin/gui/key_handler.rs"
      provides: "FocusState enum, process_key() dispatch function"
      exports: ["FocusState", "process_key"]
    - path: "src/config.rs"
      provides: "[keys] section parsing from hum.toml"
      contains: "KeysConfig"
  key_links:
    - from: "src/bin/gui/app.rs"
      to: "src/bin/gui/key_handler.rs"
      via: "App::handle_event delegates to process_key()"
      pattern: "process_key"
    - from: "src/bin/gui/app.rs"
      to: "transport_client::send_cmd"
      via: "spacebar → send_cmd play/stop"
      pattern: "send_cmd"
---

<objective>
Add DAW-style keyboard shortcuts, focus management, keybinding config, status bar improvements, and Catppuccin Mocha theme.

Purpose: hum-gui becomes keyboard-driven like a real DAW. Escape toggles between terminal typing and GUI shortcuts. Theme is visually consistent Catppuccin.
Output: key_handler.rs module, [keys] config section, upgraded transport bar with active count, Mocha color constants.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@src/bin/gui/app.rs
@src/bin/gui/transport_client.rs
@Cargo.toml

<interfaces>
<!-- From plan 15-01 output -->
From src/bin/gui/terminal_pane.rs (created in plan 01):
```rust
pub struct TerminalPane {
    pub has_focus: bool,
    // ...
}
// Access via:
// self.ui.widget(id!(terminal)).downcast_mut::<TerminalPane>()
```

From src/bin/gui/transport_client.rs:
```rust
pub fn send_cmd(cmd: &str) -> Result<(), Box<dyn std::error::Error>>;
// GuiState.active: Vec<String>  — ordered list of active thing names
```

Catppuccin Mocha palette reference:
```
Base:    #1e1e2e    Surface0: #313244    Surface1: #45475a
Text:    #cdd6f4    Subtext1: #bac2de    Subtext0: #a6adc8
Blue:    #89b4fa    Green:    #a6e3a1    Red:      #f38ba8
Mauve:   #cba6f7    Peach:    #fab387    Yellow:   #f9e2af
Teal:    #94e2d5    Sky:      #89dceb    Lavender: #b4befe
Overlay0:#6c7086   Mantle:   #181825    Crust:    #11111b
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: FocusState + key_handler module + keybinding config</name>
  <files>src/bin/gui/key_handler.rs, src/config.rs</files>
  <action>
    **src/bin/gui/key_handler.rs:**

    ```rust
    #[derive(Debug, Clone, PartialEq)]
    pub enum FocusState { Gui, Terminal }

    pub struct KeysConfig {
        pub play_stop: KeyCode,   // default Space
        pub record: KeyCode,      // default R
        pub mute: KeyCode,        // default M
        // number keys 1-9 are always fixed (no config needed)
    }

    impl Default for KeysConfig {
        fn default() -> Self {
            Self {
                play_stop: KeyCode::Space,
                record: KeyCode::KeyR,
                mute: KeyCode::KeyM,
            }
        }
    }

    /// Dispatch a key event based on current focus state.
    /// Returns Some(cmd_json) if a daemon command should be sent, None otherwise.
    pub fn process_key(
        key: &KeyEvent,
        focus: &mut FocusState,
        active_things: &[String],
        keys_cfg: &KeysConfig,
    ) -> Option<String> {
        // Escape always toggles focus regardless of current state
        if key.key_code == KeyCode::Escape {
            *focus = match focus {
                FocusState::Gui => FocusState::Terminal,
                FocusState::Terminal => FocusState::Gui,
            };
            return None;
        }

        // Only handle DAW shortcuts when GUI has focus
        if *focus != FocusState::Gui { return None; }

        if key.key_code == keys_cfg.play_stop {
            return Some(r#"{"cmd":"toggle_play"}"#.into());
        }
        if key.key_code == keys_cfg.record {
            return Some(r#"{"cmd":"record"}"#.into());
        }
        if key.key_code == keys_cfg.mute {
            return Some(r#"{"cmd":"mute_focused"}"#.into());
        }

        // Number keys 1-9 → solo thing by index
        let digit = match key.key_code {
            KeyCode::Key1 => Some(0), KeyCode::Key2 => Some(1),
            KeyCode::Key3 => Some(2), KeyCode::Key4 => Some(3),
            KeyCode::Key5 => Some(4), KeyCode::Key6 => Some(5),
            KeyCode::Key7 => Some(6), KeyCode::Key8 => Some(7),
            KeyCode::Key9 => Some(8), _ => None,
        };
        if let Some(idx) = digit {
            if let Some(name) = active_things.get(idx) {
                return Some(format!(r#"{{"cmd":"solo","thing":"{}"}}"#, name));
            }
        }

        None
    }
    ```

    **src/config.rs — add [keys] section:**
    Find the existing HumConfig struct. Add a `keys: KeysConfig` field (with `#[serde(default)]`). Add `KeysConfig` struct with string fields mapping to key names:
    ```rust
    #[derive(Deserialize, Default)]
    pub struct KeysConfig {
        #[serde(default = "default_play_stop")]
        pub play_stop: String,   // "Space"
        #[serde(default = "default_record")]
        pub record: String,      // "R"
        #[serde(default = "default_mute")]
        pub mute: String,        // "M"
    }
    ```
    The GUI reads this and maps string → KeyCode in App. Keep it simple: just Space/R/M are the three configurable defaults; other keys stay hardcoded.
  </action>
  <verify>
    <automated>cargo build --bin hum-gui 2>&1 | grep -E "^error" | head -20</automated>
  </verify>
  <done>key_handler.rs compiles. process_key() dispatches Space→play, 1-9→solo, Escape→focus toggle. Config struct parses [keys] from hum.toml.</done>
</task>

<task type="auto">
  <name>Task 2: Wire focus + shortcuts into App, upgrade status bar, apply Catppuccin theme</name>
  <files>src/bin/gui/app.rs, src/bin/gui/main.rs</files>
  <action>
    **App struct additions:**
    ```rust
    #[rust] focus: FocusState,         // default FocusState::Gui
    #[rust] keys_cfg: KeysConfig,      // loaded from config on startup
    ```

    **handle_event in App:**
    Add to `AppMain::handle_event`:
    ```rust
    if let Event::KeyDown(key) = event {
        if let Some(cmd) = key_handler::process_key(key, &mut self.focus, &active, &self.keys_cfg) {
            transport_client::send_cmd(&cmd).ok();
        }
        // Forward all keys to terminal when terminal has focus
        if self.focus == FocusState::Terminal {
            self.ui.widget(id!(terminal)).handle_event(cx, event, &mut Scope::empty());
        }
    }
    ```
    Where `active` is read from `self.gui_state.lock().unwrap().active.clone()`.

    Also set `terminal.has_focus` based on `self.focus` in the timer update so the cursor blinks correctly.

    **Status bar — add active count label:**
    In `live_design!` transport_bar, add after `conn_label`:
    ```
    active_label = <Label> {
        text: "0 active"
        draw_text: { text_style: { font_size: 10.0 }, color: (SUBTEXT0) }
    }
    focus_label = <Label> {
        text: "[GUI]"
        draw_text: { text_style: { font_size: 10.0 }, color: (MAUVE) }
    }
    ```
    In `update_transport_ui`: set active_label to `"{N} active"` from `state.active.len()`.
    Set focus_label to `"[GUI]"` or `"[TERM]"` based on `self.focus`.

    **Catppuccin Mocha theme — update color constants in live_design!:**
    Replace the existing color definitions at the top of live_design! with the full Mocha palette:
    ```
    DARK_BG   = #1e1e2e    // Base
    MANTLE    = #181825    // Mantle (transport bar bg)
    SURFACE0  = #313244
    SURFACE1  = #45475a
    TEXT_COLOR = #cdd6f4   // Text
    SUBTEXT0  = #a6adc8
    SUBTEXT1  = #bac2de
    ACCENT    = #89b4fa    // Blue
    MAUVE     = #cba6f7
    GREEN     = #a6e3a1
    RED       = #f38ba8
    PEACH     = #fab387
    YELLOW    = #f9e2af
    TEAL      = #94e2d5
    SUBTLE    = #6c7086    // Overlay0
    ```
    Update transport_bar `draw_bg: { color: (MANTLE) }` and status_dot color usage.
    Add `IDE-04` comment above the palette block: `// Theme: Catppuccin Mocha (IDE-04) — switchable via config in future`.

    **Register key_handler in main.rs:** Add `mod key_handler;` to the gui module.
  </action>
  <verify>
    <automated>cargo build --bin hum-gui 2>&1 | grep -E "^error" | head -20</automated>
  </verify>
  <done>
    - Build passes.
    - Status bar shows "N active" count and "[GUI]"/"[TERM]" focus indicator.
    - Color constants reflect Catppuccin Mocha.
    - Space key sends play/stop cmd when GUI has focus.
    - Number keys 1-9 send solo commands.
    - Escape toggles focus label between [GUI] and [TERM].
  </done>
</task>

</tasks>

<verification>
```
cargo build --bin hum-gui
# With daemon running:
WAYLAND_DISPLAY="" hum-gui
# Spacebar → play/stop
# Press 1 → solos first active thing
# Press Escape → focus_label switches to [TERM], spacebar no longer triggers play
# Press Escape again → back to [GUI]
```
</verification>

<success_criteria>
- Spacebar play/stops when focus=[GUI], does nothing in terminal when focus=[TERM]
- Number keys 1-9 solo indexed things
- Escape toggles focus, status bar shows current focus state
- Status bar shows active thing count
- Catppuccin Mocha colors visible (blue accent, dark base)
</success_criteria>

<output>
After completion, create `.planning/phases/15-sound-ide/15-02-SUMMARY.md`
</output>

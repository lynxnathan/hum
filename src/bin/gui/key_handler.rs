//! DAW-style keyboard shortcut dispatch for hum-gui.
//!
//! FocusState tracks whether the GUI or terminal pane owns keyboard input.
//! process_key() maps key events to daemon commands when GUI has focus.
//! Covers requirements: KEYS-01 (play/stop), KEYS-02 (record/mute),
//! KEYS-03 (number-key solo), KEYS-04 (escape focus toggle).

use makepad_widgets::*;

/// Which part of the UI currently owns keyboard input.
#[derive(Debug, Clone, PartialEq)]
pub enum FocusState {
    /// DAW shortcuts active (spacebar = play/stop, etc.)
    Gui,
    /// Keystrokes forwarded to embedded terminal PTY.
    Terminal,
}

impl Default for FocusState {
    fn default() -> Self {
        FocusState::Gui
    }
}

/// Configurable key bindings loaded from hum.toml [keys] section.
/// Number keys 1-9 are always hardcoded to solo-by-index.
pub struct KeysConfig {
    pub play_stop: KeyCode,
    pub record: KeyCode,
    pub mute: KeyCode,
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

impl KeysConfig {
    /// Build from config strings (e.g. "Space", "R", "M").
    /// Falls back to defaults for unrecognized values.
    pub fn from_strings(play_stop: &str, record: &str, mute: &str) -> Self {
        Self {
            play_stop: str_to_keycode(play_stop).unwrap_or(KeyCode::Space),
            record: str_to_keycode(record).unwrap_or(KeyCode::KeyR),
            mute: str_to_keycode(mute).unwrap_or(KeyCode::KeyM),
        }
    }
}

/// Dispatch a key event based on current focus state.
/// Returns `Some(cmd_json)` if a daemon command should be sent, `None` otherwise.
///
/// Side-effect: Escape toggles `focus` between Gui and Terminal.
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
    if *focus != FocusState::Gui {
        return None;
    }

    // Play/stop toggle
    if key.key_code == keys_cfg.play_stop {
        return Some(r#"{"cmd":"toggle_play"}"#.into());
    }

    // Record
    if key.key_code == keys_cfg.record {
        return Some(r#"{"cmd":"record"}"#.into());
    }

    // Mute focused thing
    if key.key_code == keys_cfg.mute {
        return Some(r#"{"cmd":"mute_focused"}"#.into());
    }

    // Number keys 1-9 -> solo thing by index
    let digit = match key.key_code {
        KeyCode::Key1 => Some(0),
        KeyCode::Key2 => Some(1),
        KeyCode::Key3 => Some(2),
        KeyCode::Key4 => Some(3),
        KeyCode::Key5 => Some(4),
        KeyCode::Key6 => Some(5),
        KeyCode::Key7 => Some(6),
        KeyCode::Key8 => Some(7),
        KeyCode::Key9 => Some(8),
        _ => None,
    };
    if let Some(idx) = digit {
        if let Some(name) = active_things.get(idx) {
            return Some(format!(r#"{{"cmd":"solo","thing":"{}"}}"#, name));
        }
    }

    None
}

/// Map a config string like "Space", "R", "M" to a Makepad KeyCode.
fn str_to_keycode(s: &str) -> Option<KeyCode> {
    match s.trim() {
        "Space" => Some(KeyCode::Space),
        "R" | "r" => Some(KeyCode::KeyR),
        "M" | "m" => Some(KeyCode::KeyM),
        "P" | "p" => Some(KeyCode::KeyP),
        "S" | "s" => Some(KeyCode::KeyS),
        _ => None,
    }
}

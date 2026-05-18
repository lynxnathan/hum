//! Layout persistence for hum-gui IDE panes.
//!
//! Saves/loads pane sizes to ~/.config/hum/layout.toml so the IDE
//! remembers split positions across restarts.  (IDE-02)

use serde::{Deserialize, Serialize};

/// Persisted pane dimensions (in logical pixels).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LayoutConfig {
    /// Left sidebar (project browser) width.
    pub browser_width: f64,
    /// Bottom terminal pane split position (distance from top of the
    /// upper/lower vertical splitter).
    pub terminal_split: f64,
    /// Visualizer + arrangement area height (top region of inner split).
    pub visualizer_height: f64,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            browser_width: 200.0,
            terminal_split: 420.0,
            visualizer_height: 220.0,
        }
    }
}

impl LayoutConfig {
    /// Load from `~/.config/hum/layout.toml`, falling back to defaults.
    pub fn load() -> Self {
        let path = dirs::config_dir()
            .unwrap_or_default()
            .join("hum/layout.toml");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Persist current values to `~/.config/hum/layout.toml`.
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

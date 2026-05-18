use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_scsynth_host")]
    pub scsynth_host: String,
    #[serde(default)]
    pub keys: KeysConfig,
}

/// Configurable keybindings from [keys] section in hum.toml.
/// String values are mapped to Makepad KeyCode by the GUI at startup.
#[derive(Debug, Deserialize)]
pub struct KeysConfig {
    #[serde(default = "default_play_stop")]
    pub play_stop: String,
    #[serde(default = "default_record")]
    pub record: String,
    #[serde(default = "default_mute")]
    pub mute: String,
}

impl Default for KeysConfig {
    fn default() -> Self {
        Self {
            play_stop: default_play_stop(),
            record: default_record(),
            mute: default_mute(),
        }
    }
}

fn default_play_stop() -> String { "Space".to_string() }
fn default_record() -> String { "R".to_string() }
fn default_mute() -> String { "M".to_string() }

fn default_scsynth_host() -> String {
    "127.0.0.1:57110".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scsynth_host: default_scsynth_host(),
            keys: KeysConfig::default(),
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let mut cfg = load_from_file().unwrap_or_default();

        if let Ok(host) = std::env::var("SCSYNTH_HOST") {
            cfg.scsynth_host = host;
        }

        Ok(cfg)
    }
}

fn load_from_file() -> Option<Config> {
    let candidates: Vec<std::path::PathBuf> = {
        let mut v = vec![std::path::PathBuf::from("hum.toml")];
        if let Some(config_dir) = dirs::config_dir() {
            v.push(config_dir.join("hum/config.toml"));
        }
        v
    };
    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            match toml::from_str::<Config>(&content) {
                Ok(cfg) => {
                    tracing::debug!("config loaded from {}", path.display());
                    return Some(cfg);
                }
                Err(e) => {
                    tracing::warn!("config parse error in {}: {}", path.display(), e);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env var tests must not run in parallel — they share process-wide state
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn default_host_when_no_config() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("SCSYNTH_HOST");
        let cfg = Config::default();
        assert_eq!(cfg.scsynth_host, "127.0.0.1:57110");
    }

    #[test]
    fn env_var_overrides_default() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("SCSYNTH_HOST", "192.168.1.100:57110");
        let cfg = Config::load().unwrap();
        assert_eq!(cfg.scsynth_host, "192.168.1.100:57110");
        std::env::remove_var("SCSYNTH_HOST");
    }
}

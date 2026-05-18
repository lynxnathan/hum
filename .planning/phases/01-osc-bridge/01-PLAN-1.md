---
phase: 01-osc-bridge
plan: 1
type: execute
wave: 1
depends_on: []
files_modified:
  - Cargo.toml
  - src/main.rs
  - src/config.rs
autonomous: true
requirements:
  - OSC-01
  - OSC-02

must_haves:
  truths:
    - "cargo build succeeds with no warnings"
    - "SCSYNTH_HOST env var overrides the default host"
    - "hum.toml in project root overrides the default host"
    - "~/.config/hum/config.toml overrides the default host"
    - "Default host is 127.0.0.1:57110 when no config is present"
  artifacts:
    - path: "Cargo.toml"
      provides: "Rust workspace with rosc, tokio, toml, anyhow, thiserror, tracing dependencies"
      contains: "rosc"
    - path: "src/config.rs"
      provides: "Config struct and load() function with layered config"
      exports: ["Config", "load"]
    - path: "src/main.rs"
      provides: "Async entry point that loads config and prints resolved host"
  key_links:
    - from: "src/main.rs"
      to: "src/config.rs"
      via: "Config::load()"
      pattern: "Config::load"
    - from: "src/config.rs"
      to: "SCSYNTH_HOST env var"
      via: "std::env::var"
      pattern: "env::var.*SCSYNTH_HOST"
---

<objective>
Bootstrap the hum-rt Rust project and implement config-driven scsynth host resolution.

Purpose: Establishes the Cargo workspace and the layered config system (env var > local file > home file > compiled default) that all subsequent OSC work depends on.
Output: Compilable Rust binary that loads and prints the resolved scsynth host; config module reusable by Plan 2.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/phases/01-osc-bridge/01-CONTEXT.md
@.planning/phases/01-osc-bridge/research/RESEARCH.md
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Cargo project + dependencies</name>
  <files>Cargo.toml, src/main.rs</files>
  <behavior>
    - `cargo build` succeeds with zero errors
    - `cargo build` produces no unused import warnings
    - `cargo run` compiles and exits cleanly (no panic)
  </behavior>
  <action>
Initialize the Rust binary project at the repo root. Run `cargo init --name hum-rt` from ~/code/hum. Then edit Cargo.toml to add all Phase 1 dependencies:

```toml
[package]
name = "hum-rt"
version = "0.1.0"
edition = "2021"

[dependencies]
rosc = "0.11"
toml = "0.8"
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
thiserror = "2.0"
tracing = "0.1"
tracing-subscriber = "0.3"
serde = { version = "1", features = ["derive"] }
dirs = "5"
```

Replace the generated src/main.rs with a minimal async entry point:

```rust
use tracing_subscriber::EnvFilter;

mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cfg = config::Config::load()?;
    tracing::info!("scsynth host: {}", cfg.scsynth_host);
    println!("hum-rt: scsynth host = {}", cfg.scsynth_host);

    Ok(())
}
```

Run `cargo build` to pull deps. Fix any version conflicts before proceeding.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build 2>&1 | tail -5</automated>
  </verify>
  <done>cargo build exits 0 with "Finished" in output and no error lines.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Config module with layered resolution</name>
  <files>src/config.rs</files>
  <behavior>
    - When SCSYNTH_HOST=192.168.1.1:57110 is set, Config::load() returns that host
    - When hum.toml contains scsynth_host = "10.0.0.1:57110" and no env var, load() returns 10.0.0.1:57110
    - When no config exists, load() returns "127.0.0.1:57110"
    - Env var takes precedence over file; local hum.toml takes precedence over ~/.config/hum/config.toml
  </behavior>
  <action>
Create src/config.rs implementing the layered config pattern from RESEARCH.md Pattern 5:

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_scsynth_host")]
    pub scsynth_host: String,
}

fn default_scsynth_host() -> String {
    "127.0.0.1:57110".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self { scsynth_host: default_scsynth_host() }
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
```

Write a test module at the bottom of config.rs:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_host_when_no_config() {
        // Ensure no env var interferes
        std::env::remove_var("SCSYNTH_HOST");
        let cfg = Config::default();
        assert_eq!(cfg.scsynth_host, "127.0.0.1:57110");
    }

    #[test]
    fn env_var_overrides_default() {
        std::env::set_var("SCSYNTH_HOST", "192.168.1.100:57110");
        let cfg = Config::load().unwrap();
        assert_eq!(cfg.scsynth_host, "192.168.1.100:57110");
        std::env::remove_var("SCSYNTH_HOST");
    }
}
```
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo test config 2>&1</automated>
  </verify>
  <done>All config tests pass. `cargo run` prints "hum-rt: scsynth host = 127.0.0.1:57110" with no env var set. `SCSYNTH_HOST=10.0.0.1:57110 cargo run` prints the overridden address.</done>
</task>

</tasks>

<verification>
1. `cargo build` exits 0, no errors, no warnings about unused imports
2. `cargo test config` — all tests pass
3. `cargo run` outputs `hum-rt: scsynth host = 127.0.0.1:57110`
4. `SCSYNTH_HOST=172.29.224.1:57110 cargo run` outputs that host instead
5. Create a temp hum.toml with `scsynth_host = "10.0.0.1:57110"` and `cargo run` (no env) outputs that host; delete the file after
</verification>

<success_criteria>
- Cargo workspace builds cleanly with all Phase 1 dependencies
- Config::load() correctly resolves: env var > hum.toml > ~/.config/hum/config.toml > default
- All config unit tests pass
- Binary compiles and runs without panic
</success_criteria>

<output>
After completion, create `.planning/phases/01-osc-bridge/01-1-SUMMARY.md` summarizing:
- Files created
- Dependencies added to Cargo.toml
- Config resolution order implemented
- Any deviations from the plan
</output>

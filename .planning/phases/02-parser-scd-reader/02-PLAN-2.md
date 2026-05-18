---
phase: 02-parser-scd-reader
plan: 2
type: execute
wave: 1
depends_on: []
files_modified:
  - src/scd/mod.rs
  - src/scd/store.rs
  - src/main.rs
autonomous: true
requirements: [SCD-01, SCD-02, SCD-03]

must_haves:
  truths:
    - ".scd files in out/sc/ are discovered and read as raw bytes on startup"
    - "Each .scd file is associated with its thing name via filename stem convention (space-crackle.scd -> thing name 'space-crackle')"
    - "SynthDefs are loaded into scsynth via ScsynthClient::load_synthdef() on startup"
    - "Missing .scd for a thing name is a warning, not a fatal error — authoring workflow continues"
    - "If out/sc/ does not exist, startup continues without error"
  artifacts:
    - path: "src/scd/store.rs"
      provides: "ScdStore: filename-to-bytes map with load_dir() and get()"
      exports: ["ScdStore"]
    - path: "src/scd/mod.rs"
      provides: "pub use store::ScdStore"
      exports: ["ScdStore"]
  key_links:
    - from: "src/scd/store.rs"
      to: "std::fs::read"
      via: "stdlib"
      pattern: "fs::read"
    - from: "src/main.rs"
      to: "ScsynthClient::load_synthdef"
      via: "ScdStore iteration"
      pattern: "load_synthdef"
---

<objective>
Implement the SCD reader: discover .scd files in out/sc/, associate each with its thing name via filename stem, and load their bytes into scsynth via the existing ScsynthClient::load_synthdef() on startup.

Purpose: Without this, hum-rt has no SynthDefs loaded and cannot produce sound even with a valid piece.hum.
Output: `src/scd/` module with ScdStore, wired into main.rs startup sequence.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/phases/02-parser-scd-reader/02-CONTEXT.md
@.planning/phases/02-parser-scd-reader/research/RESEARCH.md

<interfaces>
<!-- ScsynthClient API from Phase 1 (src/osc/bridge.rs) -->

```rust
pub struct ScsynthClient { /* ... */ }

impl ScsynthClient {
    pub async fn connect(addr: &str) -> Result<Self>;
    pub async fn load_synthdef(&mut self, synthdef_bytes: Vec<u8>) -> Result<()>;
    pub async fn new_synth(&mut self, thing_name: &str, synthdef_name: &str) -> Result<i32>;
    pub async fn set_param(&self, thing_name: &str, param: &str, value: f32) -> Result<()>;
    pub async fn free_node(&mut self, thing_name: &str) -> Result<()>;
    pub async fn free_all_nodes(&mut self) -> Result<()>;
}
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Implement ScdStore</name>
  <files>src/scd/store.rs, src/scd/mod.rs</files>
  <behavior>
    - ScdStore::load_dir on a dir with "space-crackle.scd" and "bass-drone.scd" returns Ok with 2 entries
    - store.get("space-crackle") returns Some(bytes) where bytes is the file content
    - store.get("missing-thing") returns None (not Err)
    - ScdStore::load_dir on a non-existent directory returns Ok with 0 entries (not Err)
    - ScdStore::load_dir ignores files that don't have .scd extension (e.g., .txt, .scsyndef)
    - store.thing_names() returns the list of thing names loaded
  </behavior>
  <action>
Create src/scd/store.rs:
```rust
use std::collections::HashMap;
use std::path::Path;

/// Maps thing names to their compiled SynthDef bytes.
/// Thing name = filename stem (e.g. "space-crackle" from "space-crackle.scd").
pub struct ScdStore {
    defs: HashMap<String, Vec<u8>>,
}

impl ScdStore {
    /// Read all .scd files from the given directory.
    /// Returns empty store (not Err) if directory does not exist.
    /// Non-.scd files are silently ignored.
    pub fn load_dir(dir: &Path) -> Result<Self, std::io::Error> {
        let mut defs = HashMap::new();
        if !dir.exists() {
            return Ok(Self { defs });
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("scd") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let bytes = std::fs::read(&path)?;
                    defs.insert(stem.to_string(), bytes);
                }
            }
        }
        Ok(Self { defs })
    }

    /// Get SynthDef bytes for a thing name. Returns None if not loaded.
    pub fn get(&self, thing_name: &str) -> Option<&[u8]> {
        self.defs.get(thing_name).map(|v| v.as_slice())
    }

    /// Returns all loaded thing names.
    pub fn thing_names(&self) -> impl Iterator<Item = &str> {
        self.defs.keys().map(|s| s.as_str())
    }

    /// Iterate over all (thing_name, bytes) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.defs.iter().map(|(k, v)| (k.as_str(), v.as_slice()))
    }

    /// Number of loaded SynthDefs.
    pub fn len(&self) -> usize {
        self.defs.len()
    }
}
```

Create src/scd/mod.rs:
```rust
mod store;
pub use store::ScdStore;
```
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo test scd -- --nocapture 2>&1 | tail -20</automated>
  </verify>
  <done>All 6 behavior tests pass. ScdStore compiles clean. get() returns Option, load_dir() on missing path returns Ok.</done>
</task>

<task type="auto">
  <name>Task 2: Wire ScdStore into startup — load all SynthDefs into scsynth</name>
  <files>src/main.rs</files>
  <action>
Update src/main.rs to add `mod scd;` and wire ScdStore into the startup sequence, after the scsynth health check and before (or replacing) the smoke test.

Add startup SCD loading in main():
```rust
// Load all .scd files from out/sc/ and send to scsynth
let scd_store = scd::ScdStore::load_dir(std::path::Path::new("out/sc"))
    .unwrap_or_else(|e| {
        tracing::warn!("could not read out/sc/: {e}");
        // Return empty store — non-fatal, out/sc/ may not exist yet
        scd::ScdStore::load_dir(std::path::Path::new("/tmp/empty_nonexistent_xyzzy"))
            .unwrap_or_else(|_| scd::ScdStore { defs: std::collections::HashMap::new() })
    });
```

Actually, implement it cleanly — ScdStore::load_dir already returns Ok on missing dir, so:
```rust
let scd_store = match scd::ScdStore::load_dir(std::path::Path::new("out/sc")) {
    Ok(store) => {
        println!("hum-rt: found {} SynthDef(s) in out/sc/", store.len());
        store
    }
    Err(e) => {
        tracing::warn!("could not read out/sc/: {e}");
        // Continue with empty store — authoring workflow allows missing .scd
        // (user may add things to piece.hum before LLM compiles them)
        return Err(e.into());
    }
};

// Load each SynthDef into scsynth
for (thing_name, bytes) in scd_store.iter() {
    match client.load_synthdef(bytes.to_vec()).await {
        Ok(()) => {
            tracing::info!("loaded SynthDef for '{}'", thing_name);
            println!("hum-rt: loaded SynthDef '{}'", thing_name);
        }
        Err(e) => {
            tracing::warn!("failed to load SynthDef for '{}': {e}", thing_name);
            // Non-fatal: log and continue loading others
        }
    }
}
```

Note: The smoke test in run_smoke_test() uses a hardcoded include_bytes! SynthDef — keep it as-is for now (Phase 1 regression test). The SCD loading above is additive and runs before the smoke test.

Also add `mod scd;` at the top of main.rs module declarations.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build 2>&1 | tail -20</automated>
  </verify>
  <done>cargo build succeeds. main.rs compiles with mod scd declared and ScdStore wired. Running hum-rt with an empty out/sc/ (or missing directory) does not crash — prints "found 0 SynthDef(s)".</done>
</task>

</tasks>

<verification>
- `cargo test` passes
- `cargo build` produces no errors or warnings
- ScdStore::load_dir on a temp dir with a test .scd file returns that file's bytes under the stem key
- ScdStore::load_dir on a non-existent path returns Ok with 0 entries
- main.rs startup sequence loads SynthDefs from out/sc/ and calls client.load_synthdef() for each
</verification>

<success_criteria>
1. cargo test scd — all tests green
2. cargo build — clean compile
3. ScdStore::get("thing-name") returns Some(bytes) for a loaded file
4. ScdStore::get("missing") returns None — not Err, not panic
5. Startup with out/sc/ missing prints "found 0 SynthDef(s)" and continues without crash
6. Startup with a valid .scd file in out/sc/ loads it via load_synthdef() and logs the thing name
</success_criteria>

<output>
After completion, create `.planning/phases/02-parser-scd-reader/02-2-SUMMARY.md`
</output>

---
phase: 09-dictionary
plan: 1
type: execute
wave: 1
depends_on: []
files_modified:
  - src/dict.rs
  - src/parser/types.rs
  - src/main.rs
autonomous: true
requirements: [DICT-01, DICT-02, DICT-03, DICT-04, DICT-07]

must_haves:
  truths:
    - "hum-rt starts up and loads hum.dict from project root without crashing (missing file = empty store)"
    - "~/.config/hum/global.dict is merged under project dict (project entries win on conflict)"
    - "A thing with style: laser resolves the 'laser' entry from DictStore as its synth base"
    - "Editing hum.dict while the daemon is running triggers hot-reload within ~1 second"
  artifacts:
    - path: "src/dict.rs"
      provides: "DictStore: load, merge global+project, get by term"
      exports: ["DictStore", "DictEntry"]
    - path: "src/parser/types.rs"
      provides: "ThingDef::style field added"
      contains: "pub style: Option<String>"
    - path: "src/main.rs"
      provides: "DictStore loaded on startup, style: resolution in resolve_synth_block, hum.dict watched"
  key_links:
    - from: "src/main.rs (resolve_synth_block)"
      to: "src/dict.rs (DictStore::get)"
      via: "thing.style.as_deref() -> dict_store.get(term)"
      pattern: "dict_store\\.get"
    - from: "src/main.rs (handle_file_change)"
      to: "src/dict.rs (DictStore::load)"
      via: "path ends with hum.dict -> reload dict_store"
      pattern: "hum\\.dict"
---

<objective>
Create DictStore, add style: to ThingDef, wire style: resolution into the synth pipeline, and add hum.dict to the file watcher.

Purpose: Provides the core vocabulary layer — things can reference dict terms as their synth base, and the dict hot-reloads like piece.hum.
Output: src/dict.rs, updated ThingDef, updated main.rs startup + file change handler.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/phases/09-dictionary/09-CONTEXT.md
@.planning/v3-DICTIONARY-SYNC.md

<!-- Key source patterns the executor needs -->
</context>

<interfaces>
<!-- Existing patterns to reuse verbatim. Do not re-invent these. -->

From src/instruments.rs — DictStore mirrors this exactly:
```rust
pub struct InstrumentStore {
    instruments: HashMap<String, SynthBlock>,
}
impl InstrumentStore {
    pub fn load_dir(dir: &Path) -> anyhow::Result<Self> { ... }
    pub fn get(&self, name: &str) -> Option<&SynthBlock> { ... }
    pub fn merge(base: &SynthBlock, over: &SynthBlock) -> SynthBlock { ... }
}
```

From src/parser/types.rs — ThingDef struct (deny_unknown_fields, add style field here):
```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThingDef {
    // ... existing fields ...
    pub instrument: Option<String>,
    // ADD: pub style: Option<String>,
}
```

From src/main.rs — resolve_synth_block pattern (style: sits BELOW instrument: in priority):
```rust
fn resolve_synth_block(thing: &parser::ThingDef, store: &InstrumentStore) -> Option<SynthBlock> {
    match &thing.instrument {
        Some(inst_name) => { /* instrument merge */ }
        None => thing.synth.clone(),
    }
}
// New signature needed:
fn resolve_synth_block(
    thing: &parser::ThingDef,
    instrument_store: &InstrumentStore,
    dict_store: &DictStore,
) -> Option<SynthBlock>
```

From src/main.rs — watcher call (add hum.dict path to the slice):
```rust
watcher::start_watcher(&[piece_hum_path.clone(), scd_dir_path.clone()], tx.clone())?;
// becomes:
watcher::start_watcher(&[piece_hum_path.clone(), scd_dir_path.clone(), dict_path.clone()], tx.clone())?;
```

From src/main.rs — handle_file_change (extend "hum" match arm or add "dict" path check):
```rust
async fn handle_file_change(..., instrument_store: &InstrumentStore, stage_store: &StageStore) {
    // Currently only handles .hum and .scd extensions
    // Add: check if path == hum.dict -> reload dict_store
}
```
</interfaces>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: DictStore module</name>
  <files>src/dict.rs</files>
  <behavior>
    - Test 1: DictStore::load with a valid hum.dict YAML returns entries with correct SynthBlock fields
    - Test 2: DictStore::load with missing file returns empty store (not an error)
    - Test 3: DictStore::merge_with_global — global entries present in merged store; project entry wins on same key
    - Test 4: DictStore::get("laser") returns Some(SynthBlock) after loading dict with laser entry
    - Test 5: DictStore::get("nonexistent") returns None
    - Test 6: DictEntry has a `context` field (Option&lt;String&gt;) parsed from the dict YAML
  </behavior>
  <action>
Create `src/dict.rs` with:

```rust
use std::collections::HashMap;
use std::path::Path;
use serde::Deserialize;
use crate::ir::types::SynthBlock;

/// A single dictionary entry: synth params + optional human context.
#[derive(Debug, Clone, Deserialize)]
pub struct DictEntry {
    pub synth: SynthBlock,
    pub context: Option<String>,
    #[serde(rename = "learned-from")]
    pub learned_from: Option<String>,
}

/// The raw hum.dict file format: term -> DictEntry mapping.
type DictFile = HashMap<String, DictEntry>;

/// Stores vocabulary entries keyed by term name.
#[derive(Debug, Default, Clone)]
pub struct DictStore {
    entries: HashMap<String, DictEntry>,
}

impl DictStore {
    /// Load a hum.dict YAML file. Missing file = empty store (not error).
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            tracing::info!("dict: no file at {:?}, using empty store", path);
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let dict_file: DictFile = serde_saphyr::from_str(&content)
            .map_err(|e| anyhow::anyhow!("dict parse error in {:?}: {}", path, e))?;
        tracing::info!("dict: loaded {} entries from {:?}", dict_file.len(), path);
        Ok(Self { entries: dict_file })
    }

    /// Merge global dict (base) with project dict (project wins on conflict).
    pub fn merge_with_global(global: DictStore, project: DictStore) -> DictStore {
        let mut merged = global.entries;
        for (k, v) in project.entries {
            merged.insert(k, v); // project overrides global
        }
        DictStore { entries: merged }
    }

    /// Look up a term. Returns the SynthBlock base for that entry.
    pub fn get(&self, term: &str) -> Option<&DictEntry> {
        self.entries.get(term)
    }

    /// All terms in insertion-independent order.
    pub fn all_terms(&self) -> Vec<&str> {
        let mut terms: Vec<&str> = self.entries.keys().map(|s| s.as_str()).collect();
        terms.sort();
        terms
    }
}
```

Note: `serde_saphyr::from_str` is the YAML deserializer used throughout this codebase (see instruments.rs). Do NOT use `serde_yaml` — it is not in Cargo.toml.

Write tests in the same file under `#[cfg(test)]` covering the 6 behaviors above. Use `tempfile::tempdir()` as instruments.rs does for file-based tests. The dict YAML test fixture:
```yaml
laser:
  synth:
    osc: "sine"
  context: bright, cutting
warm:
  synth:
    filter: "lpf(cutoff: 800)"
  context: soft, intimate
```
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo test dict:: -- --nocapture 2>&1 | tail -20</automated>
  </verify>
  <done>All 6 dict:: tests pass; `cargo check` succeeds with dict module added to main.rs mod list</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: style: field + resolution + watcher + startup wiring</name>
  <files>src/parser/types.rs, src/main.rs</files>
  <behavior>
    - Test 1 (parser): ThingDef with `style: laser` deserializes with style = Some("laser")
    - Test 2 (parser): ThingDef without style: deserializes with style = None
    - Test 3 (parser): ThingDef with unknown field still rejected (deny_unknown_fields still enforced)
    - Test 4 (resolve): thing with style: laser + no synth: → resolves to dict entry's SynthBlock
    - Test 5 (resolve): thing with style: laser + synth: { amp: 0.5 } → dict SynthBlock with amp overridden to 0.5
    - Test 6 (resolve): thing with instrument: foo + style: laser → instrument merge wins (style is lowest priority)
  </behavior>
  <action>
**src/parser/types.rs**: Add `pub style: Option<String>` to ThingDef, after `instrument: Option<String>`. The field has no rename needed. Because `deny_unknown_fields` is set, adding the field here is sufficient to allow parsing.

**src/main.rs**:

1. Add `mod dict;` and `use dict::DictStore;` at the top with other module declarations.

2. In daemon startup (after `instrument_store` loading), load both dicts and merge:
```rust
// Load global dict (~/.config/hum/global.dict)
let global_dict_path = dirs::home_dir()
    .map(|h| h.join(".config/hum/global.dict"))
    .unwrap_or_default();
let global_dict = DictStore::load(&global_dict_path).unwrap_or_default();

// Load project dict (hum.dict)
let dict_path = PathBuf::from("hum.dict");
let project_dict = DictStore::load(&dict_path).unwrap_or_default();
let dict_store = DictStore::merge_with_global(global_dict, project_dict);
println!("hum-rt: dict loaded ({} terms)", dict_store.all_terms().len());
```

Note: `dirs` crate is NOT in Cargo.toml. Do NOT add it. Instead resolve the home dir manually:
```rust
let global_dict_path = std::env::var("HOME")
    .ok()
    .map(|h| PathBuf::from(h).join(".config/hum/global.dict"))
    .unwrap_or_else(|| PathBuf::from("/tmp/no-global.dict"));
```

3. Update `resolve_synth_block` signature to accept `dict_store: &DictStore` as third parameter. Resolution priority (highest to lowest):
   - .scd escape hatch (checked at call site before resolve_synth_block is called — unchanged)
   - `instrument:` merge (existing logic — unchanged)
   - `style:` dict lookup (NEW: if no instrument:, check style: before falling through to bare synth:)
   - `synth:` block (existing)

Updated logic when no `instrument:`:
```rust
None => {
    match &thing.style {
        Some(term) => {
            match dict_store.get(term) {
                Some(entry) => {
                    // dict entry is base, thing's synth: overrides
                    match &thing.synth {
                        Some(over) => Some(InstrumentStore::merge(&entry.synth, over)),
                        None => Some(entry.synth.clone()),
                    }
                }
                None => {
                    tracing::warn!("dict term '{}' not found, using thing's synth as-is", term);
                    thing.synth.clone()
                }
            }
        }
        None => thing.synth.clone(),
    }
}
```

4. Pass `&dict_store` to every call site of `resolve_synth_block` in main.rs (startup compilation loop and `handle_file_change`). Update `handle_file_change` signature to add `dict_store: &DictStore` parameter and thread it through.

5. Add `dict_path.clone()` to the `start_watcher` call:
```rust
watcher::start_watcher(
    &[piece_hum_path.clone(), scd_dir_path.clone(), dict_path.clone()],
    tx.clone()
)?;
```

6. In `handle_file_change`, add dict hot-reload. Check if the changed path ends with `hum.dict` — if so, reload both dicts and return (the next piece.hum change will recompile with new dict; or trigger a reconcile immediately):
```rust
// Dict hot-reload: check before the extension match
if path.ends_with("hum.dict") {
    let global_dict_path = std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".config/hum/global.dict"))
        .unwrap_or_else(|| PathBuf::from("/tmp/no-global.dict"));
    let global_dict = DictStore::load(&global_dict_path).unwrap_or_default();
    let project_dict = DictStore::load(path).unwrap_or_default();
    *dict_store = DictStore::merge_with_global(global_dict, project_dict);
    tracing::info!("dict: hot-reloaded ({} terms)", dict_store.all_terms().len());
    println!("hum-rt: dict reloaded ({} terms)", dict_store.all_terms().len());
    return;
}
```
This requires `dict_store` parameter to be `&mut DictStore` in `handle_file_change`. Update all call sites accordingly.

Write parser tests in `src/parser/types.rs` under existing `#[cfg(test)]` (or add one) for behaviors 1-3. Write resolve tests inline in main.rs or a test helper for 4-6.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo test 2>&1 | tail -30</automated>
  </verify>
  <done>All existing tests pass; new style/dict tests pass; `cargo build` succeeds; daemon starts up and prints "dict loaded (N terms)"</done>
</task>

</tasks>

<verification>
```bash
cd ~/code/hum && cargo test 2>&1 | tail -20
```

Manual smoke test (optional, covered by Plan 2's checkpoint):
- Create hum.dict with a `laser` entry
- Add `style: laser` to a thing in piece.hum
- Run hum-rt and verify it doesn't crash, logs show dict loaded
</verification>

<success_criteria>
- DictStore loads hum.dict and ~/.config/hum/global.dict at startup; project wins on conflicts
- ThingDef accepts style: field without parse errors
- style: laser resolves to dict SynthBlock as base; thing's synth: overrides
- hum.dict watched alongside piece.hum; changes trigger hot-reload
- All 180+ existing tests still pass
</success_criteria>

<output>
After completion, create `.planning/phases/09-dictionary/09-1-SUMMARY.md`
</output>

---
phase: 07-instruments-stage
plan: 1
type: execute
wave: 1
depends_on: []
files_modified:
  - src/instruments.rs
  - src/parser/types.rs
  - src/main.rs
autonomous: true
requirements: [INST-01, INST-02, INST-03]

must_haves:
  truths:
    - "A .hum file in instruments/ with type: instrument is loaded into InstrumentStore on startup"
    - "A thing with instrument: foo inherits foo's SynthBlock fields as the base"
    - "A thing's own synth: fields override the instrument's fields (field-level merge)"
  artifacts:
    - path: "src/instruments.rs"
      provides: "InstrumentStore: HashMap<String, SynthBlock>, load_dir(), merge_into()"
      exports: ["InstrumentStore"]
    - path: "src/parser/types.rs"
      provides: "ThingDef with instrument: Option<String> and type: Option<ThingType> fields"
      contains: "pub instrument: Option<String>"
    - path: "src/main.rs"
      provides: "startup loads instruments/ dir, apply merge before compile"
      contains: "InstrumentStore::load_dir"
  key_links:
    - from: "src/main.rs"
      to: "src/instruments.rs"
      via: "InstrumentStore::load_dir(\"instruments/\")"
      pattern: "InstrumentStore::load_dir"
    - from: "src/main.rs"
      to: "src/ir/compiler.rs"
      via: "merged SynthBlock passed to compile_synth_block"
      pattern: "merge_into"
---

<objective>
Load instrument definitions from the instruments/ directory on startup and merge them into things that reference them.

Purpose: Enables reusable sound definitions so composers write `instrument: berimbal-amp` instead of repeating the full synth block.
Output: InstrumentStore module + ThingDef parser additions + main.rs wiring.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/phases/07-instruments-stage/07-CONTEXT.md

<interfaces>
<!-- Key types the executor needs. No codebase exploration required. -->

From src/ir/types.rs:
```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthBlock {
    pub notes: Option<Vec<String>>,
    pub osc: Option<OscPrimitive>,
    pub filter: Option<FilterPrimitive>,
    pub env: Option<EnvPrimitive>,
    pub distort: Option<DistortPrimitive>,
    pub fx: Option<FxPrimitive>,
    pub pan: Option<PanPrimitive>,
    pub amp: Option<f32>,
    pub tempo: Option<String>,
}
```

From src/parser/types.rs (current — needs additions):
```rust
pub struct ThingDef {
    pub at: Option<String>,
    pub until: Option<String>,
    pub does: Option<DoesField>,
    #[serde(rename = "where")]
    pub location: Option<String>,
    pub has: Option<IndexMap<String, ThingDef>>,
    pub within: Option<String>,
    pub every: Option<String>,
    pub like: Option<String>,
    #[serde(rename = "ref")]
    pub reference: Option<String>,
    pub mood: Option<String>,
    pub synth: Option<SynthBlock>,
    // MISSING — must add:
    // pub instrument: Option<String>,
    // pub r#type: Option<ThingType>,
    // pub applies_to: Option<Vec<String>>,
}
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: InstrumentStore module + SynthBlock merge</name>
  <files>src/instruments.rs</files>
  <behavior>
    - load_dir("instruments/") reads all *.hum files, parses type: instrument things, stores name -> SynthBlock
    - File "instruments/berimbal-amp.hum" → key "berimbal-amp"
    - Non-instrument files (missing type: instrument) are skipped with a warning log, not fatal
    - merge(base: &SynthBlock, override: &SynthBlock) -> SynthBlock: each field in override wins if Some, else use base field
    - Test: merge where base has osc: saw, override has notes: [D4] → result has both osc: saw and notes: [D4]
    - Test: merge where both have amp → override amp wins
    - Test: load_dir on empty dir returns empty store, not error
  </behavior>
  <action>
    Create src/instruments.rs.

    InstrumentFile struct (for parsing): deserializes with `type: instrument` + `synth:` block. Since instrument files are NOT ThingDef maps (they are standalone files with a single type + synth), define:

    ```rust
    #[derive(Debug, Deserialize)]
    struct InstrumentFile {
        r#type: String,   // must equal "instrument"
        synth: SynthBlock,
    }
    ```

    InstrumentStore:
    ```rust
    pub struct InstrumentStore {
        instruments: HashMap<String, SynthBlock>,
    }

    impl InstrumentStore {
        pub fn load_dir(dir: &Path) -> Result<Self>;
        pub fn get(&self, name: &str) -> Option<&SynthBlock>;
        pub fn merge(base: &SynthBlock, over: &SynthBlock) -> SynthBlock;
    }
    ```

    load_dir: glob *.hum in dir, for each file parse as InstrumentFile, skip if type != "instrument" (warn), store by stem name (file stem without .hum extension).

    merge: construct new SynthBlock where each field = over.field.clone().or_else(|| base.field.clone()).

    Use serde_saphyr for YAML parsing (same as rest of codebase). Return anyhow::Result.
    Add #[cfg(test)] module with unit tests for merge behavior.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo test instruments:: 2>&1 | tail -20</automated>
  </verify>
  <done>cargo test passes for all instruments:: tests. merge field precedence correct.</done>
</task>

<task type="auto">
  <name>Task 2: Extend ThingDef + main.rs instrument wiring</name>
  <files>src/parser/types.rs, src/main.rs</files>
  <action>
    **src/parser/types.rs:**

    Add ThingType enum and new fields to ThingDef. ThingDef has `deny_unknown_fields`, so EVERY new YAML key needs a field:

    ```rust
    #[derive(Debug, Clone, Deserialize, PartialEq)]
    #[serde(rename_all = "kebab-case")]
    pub enum ThingType {
        Instrument,
        Stage,
    }

    // In ThingDef — add these fields:
    #[serde(rename = "type")]
    pub thing_type: Option<ThingType>,

    pub instrument: Option<String>,

    #[serde(rename = "applies-to")]
    pub applies_to: Option<Vec<String>>,
    ```

    Note: `type` is a Rust keyword — use `#[serde(rename = "type")]` on field named `thing_type`.

    **src/main.rs:**

    1. Add `mod instruments;` and `use instruments::InstrumentStore;`

    2. After scsynth connect and SCD loading, add:
    ```rust
    let instruments_path = PathBuf::from("instruments");
    let instrument_store = InstrumentStore::load_dir(&instruments_path)
        .unwrap_or_else(|e| {
            tracing::warn!("instruments/ load failed: {e}");
            InstrumentStore::default()
        });
    ```
    (Implement Default for InstrumentStore returning empty store.)

    3. In the reconcile/compile path where synth blocks are compiled: before calling compile_synth_block, check if thing has instrument field. If so, look up base from store, merge thing.synth over base, pass merged block to compiler. If instrument not found in store, log warning and use thing.synth as-is.

    Find the existing location in main.rs where ReconcileOp::Add/Update calls compile_synth_block and insert the merge there. The pattern is: `if let Some(synth) = &thing.synth { compile_synth_block(...) }` — extend to merge first if `thing.instrument` is Some.

    Do NOT break existing non-instrument things (instrument field is None → no change in behavior).
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build 2>&1 | tail -30</automated>
  </verify>
  <done>
    cargo build succeeds. ThingDef now has instrument, thing_type, applies_to fields. InstrumentStore loads on startup (missing instruments/ dir is non-fatal). A thing with instrument: name gets its synth merged with the instrument base.
  </done>
</task>

</tasks>

<verification>
1. `cargo test` passes — all existing tests + new instruments:: tests green
2. `cargo build` succeeds with no warnings on new fields
3. Create instruments/test-inst.hum with `type: instrument` + `synth: {osc: saw}`, piece.hum thing with `instrument: test-inst` + `synth: {amp: 0.3}` → compiled SynthBlock has osc: saw AND amp: 0.3
</verification>

<success_criteria>
- InstrumentStore loads *.hum files from instruments/ on startup (missing dir = warn + empty store, not crash)
- thing with instrument: foo gets foo's SynthBlock as base
- thing's own synth: fields override base field-by-field
- All 49 existing tests still pass
</success_criteria>

<output>
After completion, create `.planning/phases/07-instruments-stage/07-1-SUMMARY.md`
</output>

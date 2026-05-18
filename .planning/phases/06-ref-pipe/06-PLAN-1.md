---
phase: 06-ref-pipe
plan: 1
type: execute
wave: 1
depends_on: []
files_modified:
  - src/ir/ref_resolver.rs
  - src/ir/mod.rs
  - src/reconciler.rs
autonomous: true
requirements: [REF-01, REF-02, REF-03, REF-04]

must_haves:
  truths:
    - "A thing with ref: other-thing inherits all synth fields, local fields win"
    - "ref(thing).notes in synth: block resolves to that thing's notes vec"
    - "ref(thing) with no field accessor resolves to the full SynthBlock"
    - "A missing ref target returns a clear error, not a panic"
  artifacts:
    - path: "src/ir/ref_resolver.rs"
      provides: "resolve_refs() that walks the Piece and resolves ref: and ref(thing).field"
      exports: ["resolve_refs"]
    - path: "src/reconciler.rs"
      provides: "reconciler calls resolve_refs before compile_synth_block"
  key_links:
    - from: "src/reconciler.rs"
      to: "src/ir/ref_resolver.rs"
      via: "resolve_refs(&piece) called before synth compilation"
      pattern: "resolve_refs"
    - from: "src/ir/ref_resolver.rs"
      to: "src/instruments.rs"
      via: "InstrumentStore::merge for field inheritance"
      pattern: "InstrumentStore::merge"
---

<objective>
Implement ref resolution: `ref: other-thing` on a ThingDef and `ref(thing)` / `ref(thing).field` inside a synth: block.

Purpose: Motif reuse — things can be variations of other things without copy-pasting synth params.
Output: src/ir/ref_resolver.rs with resolve_refs() called by the reconciler before compilation.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md

<interfaces>
<!-- Key contracts the executor needs. No codebase exploration needed. -->

From src/parser/types.rs:
```rust
pub struct ThingDef {
    pub reference: Option<String>,   // YAML "ref:" field — thing name to inherit from
    pub synth: Option<SynthBlock>,
    // ... other fields
}
pub type Piece = IndexMap<String, ThingDef>;
```

From src/ir/types.rs:
```rust
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

From src/instruments.rs:
```rust
impl InstrumentStore {
    pub fn merge(base: &SynthBlock, over: &SynthBlock) -> SynthBlock {
        // over fields win when Some, base fills gaps
    }
}
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: ref_resolver module — resolve ref: and ref(thing).field</name>
  <files>src/ir/ref_resolver.rs, src/ir/mod.rs</files>
  <behavior>
    - resolve_refs(piece) with ref: glass returns ThingDef whose synth is merge(glass.synth, self.synth)
    - resolve_refs(piece) with ref: missing-thing returns Err with the missing name
    - ref(thing).notes in synth notes field: ["ref(glass).notes"] resolves to glass.synth.notes
    - ref(thing) (no field) in synth block: merges the entire referenced SynthBlock as base
    - Local field (e.g. local amp:) overrides inherited field after merge
    - Things without ref: pass through unchanged
  </behavior>
  <action>
Create `src/ir/ref_resolver.rs`.

**Two resolution passes:**

**Pass 1 — ThingDef-level ref: (REF-01, REF-04)**
Iterate the Piece. For each ThingDef where `thing.reference.is_some()`:
- Look up the referenced thing by name in the same Piece.
- If not found: return `Err(anyhow!("ref '{}' not found in piece", name))`.
- If found and both have a synth block: call `InstrumentStore::merge(ref_synth, local_synth)` where ref_synth is the base and local_synth overrides. Replace thing.synth with the merged result.
- If the referenced thing has no synth block: log a warning, leave thing.synth as-is.
- Do NOT recurse into chains (ref of a ref) in this phase — log a warning if ref target itself has a ref:.

**Pass 2 — ref(thing).field inside synth notes (REF-02, REF-03)**
After Pass 1, scan each SynthBlock's `notes` field for strings matching the regex `^ref\(([^)]+)\)(?:\.(\w+))?$`.
- `ref(glass)` (no field): merge glass.synth as base into this synth block (same as Pass 1 logic).
- `ref(glass).notes`: replace the entire `notes` vec with glass.synth.notes. If glass has no notes: Err.
- Other field accessors (`.osc`, `.amp`, etc.): not supported in this phase — return Err with helpful message.

Expose: `pub fn resolve_refs(piece: &mut Piece) -> anyhow::Result<()>`

Add `pub mod ref_resolver;` to `src/ir/mod.rs`.
  </action>
  <verify>
    <automated>cargo test -p hum-rt --lib ir::ref_resolver -- --nocapture 2>&1 | tail -20</automated>
  </verify>
  <done>
    All ref_resolver unit tests pass. resolve_refs signature exported from src/ir/mod.rs.
  </done>
</task>

<task type="auto">
  <name>Task 2: Wire resolve_refs into reconciler before synth compilation</name>
  <files>src/reconciler.rs</files>
  <action>
In `src/reconciler.rs`, find where ThingDefs are processed before `compile_synth_block` is called.

Call `ir::ref_resolver::resolve_refs(&mut piece)` on the parsed Piece immediately after parsing and before any synth compilation. If it returns Err, log the error with `tracing::error!` and skip compilation for that reconcile cycle (do not crash the daemon).

The reconciler already has access to the full Piece (all things). Pass a mutable reference. The resolver mutates ThingDefs in-place (merging synth blocks), so subsequent compilation sees the resolved blocks.

No new structs or files needed — just the call site in reconciler.rs.
  </action>
  <verify>
    <automated>cargo build 2>&1 | grep -E "^error" | head -20; echo "build exit: $?"</automated>
  </verify>
  <done>
    `cargo build` succeeds. A piece.hum with `ref: other-thing` compiles without error at runtime. Ref errors are logged, not panicked.
  </done>
</task>

</tasks>

<verification>
cargo test -p hum-rt --lib 2>&1 | tail -10
</verification>

<success_criteria>
1. Unit tests cover: ref: inheritance, local override wins, missing ref returns Err, ref(thing).notes accessor.
2. cargo build passes with no errors.
3. resolve_refs is called in reconciler before compile_synth_block.
</success_criteria>

<output>
After completion, create `.planning/phases/06-ref-pipe/06-1-SUMMARY.md`
</output>

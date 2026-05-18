---
phase: 06-ref-pipe
plan: 3
type: execute
wave: 2
depends_on: [06-PLAN-1, 06-PLAN-2]
files_modified:
  - src/pipe/executor.rs
  - src/pipe/mod.rs
  - src/reconciler.rs
autonomous: true
requirements: [PIPE-01, PIPE-02, PIPE-03, PIPE-04, PIPE-05, PIPE-06, PIPE-07, PIPE-08, PIPE-09]

must_haves:
  truths:
    - "A thing with pipe: block produces N compiled SynthBlocks sent to scsynth as separate nodes"
    - "replicate(3) |> each(i => shift(semitones: i*4)) creates 3 nodes with notes shifted by 0, 4, 8 semitones"
    - "Pipe source thing.notes resolves to that thing's notes vec before transforms"
    - "Pipe-expanded nodes get synthetic names: '{thing}-pipe-0', '{thing}-pipe-1', etc."
    - "Pipe things with no synth: on the source are rejected with a clear error"
  artifacts:
    - path: "src/pipe/executor.rs"
      provides: "expand_pipe(expr, piece) -> Result<Vec<(String, SynthBlock)>>"
      exports: ["expand_pipe"]
    - path: "src/reconciler.rs"
      provides: "pipe things are expanded before compile_synth_block, registered as multiple nodes"
  key_links:
    - from: "src/reconciler.rs"
      to: "src/pipe/executor.rs"
      via: "expand_pipe called for things where pipe.is_some()"
      pattern: "expand_pipe"
    - from: "src/pipe/executor.rs"
      to: "src/ir/compiler.rs"
      via: "each expanded SynthBlock fed to compile_synth_block"
      pattern: "compile_synth_block"
---

<objective>
Execute pipe expressions: expand a PipeExpr into N named SynthBlocks and wire them into the reconciler's compile-and-load loop.

Purpose: The pipe: syntax becomes audible — glass-swarm with replicate(3) spawns 3 independent scsynth nodes.
Output: src/pipe/executor.rs with expand_pipe(); reconciler handles pipe things as multi-node things.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/v2-PIPE-LANG.md
@.planning/phases/06-ref-pipe/06-1-SUMMARY.md
@.planning/phases/06-ref-pipe/06-2-SUMMARY.md

<interfaces>
<!-- Contracts from Plans 1 and 2 that this plan builds on -->

From src/pipe/types.rs (Plan 2):
```rust
pub enum PipeSource {
    Thing(String),
    Field(String, String),  // (thing_name, field_name)
}
pub enum Transform {
    Replicate { n: usize },
    Each { expr: String },
    Map { expr: String },
    Shift { semitones: i32 },
    Spread { lo: f32, hi: f32 },
    Tempo { seconds_per_note: f32 },
    Take { n: usize },
    Repeat { n: usize },
}
pub struct PipeExpr {
    pub source: PipeSource,
    pub transforms: Vec<Transform>,
}
```

From src/pipe/parser.rs (Plan 2):
```rust
pub fn parse_pipe_block(input: &str) -> anyhow::Result<PipeExpr>;
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

From src/ir/compiler.rs:
```rust
pub fn compile_synth_block(name: &str, block: &SynthBlock) -> anyhow::Result<Vec<u8>>;
```

From src/instruments.rs:
```rust
impl InstrumentStore {
    pub fn merge(base: &SynthBlock, over: &SynthBlock) -> SynthBlock;
}
```

Note parsing helper for semitone shift:
MIDI note numbers for note strings are already handled by the note sequencer (Phase 5).
For Shift transform: add `semitones` offset to each note's MIDI value. Notes are stored as strings like "D4", "Eb4", "-". Use a helper that converts note name → MIDI int, adds offset, converts back. Use a simple lookup table for the 12 pitch classes (C, C#/Db, D, D#/Eb, E, F, F#/Gb, G, G#/Ab, A, A#/Bb, B) + octave digit. "-" (rest) passes through unchanged.
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: pipe/executor.rs — expand_pipe</name>
  <files>src/pipe/executor.rs, src/pipe/mod.rs</files>
  <behavior>
    - Source Thing("glass") with glass having synth.osc=Sine: returns base SynthBlock cloned from glass.synth
    - Source Field("glass","notes") extracts only glass.synth.notes; base SynthBlock is otherwise default (no osc — error until osc comes from transforms or is required to be on local synth)
    - replicate(3) on a single SynthBlock produces Vec of 3 identical SynthBlocks
    - shift(semitones: 4) on notes ["D4","Eb4"] produces ["F#4","G4"]
    - shift on rest "-" passes through unchanged
    - spread(pan: -0.8~0.8) with 3 voices assigns pan values [-0.8, 0.0, 0.8] (linear interpolation)
    - take(2) on notes ["A","B","C","D"] produces ["A","B"]
    - repeat(2) on notes ["A","B"] produces ["A","B","A","B"]
    - tempo(0.35s/note) sets synth.tempo = "0.35s/note" on each voice
    - each(i => shift(semitones: i * 4)) with 3 voices: voice 0 shifts 0, voice 1 shifts 4, voice 2 shifts 8
    - Output: Vec<(String, SynthBlock)> where String is "{thing_name}-pipe-{i}"
    - Missing source thing returns Err
  </behavior>
  <action>
Create `src/pipe/executor.rs` with:

```rust
pub fn expand_pipe(
    thing_name: &str,
    expr: &PipeExpr,
    piece: &Piece,
) -> anyhow::Result<Vec<(String, SynthBlock)>>
```

**Step 1 — Resolve source:**
- `PipeSource::Thing(name)`: look up thing in piece, get its synth block. Error if not found or no synth.
- `PipeSource::Field(thing, "notes")`: look up thing, extract notes field into a temporary SynthBlock with only notes set. Other fields are None — the local thing's own synth: provides osc/filter/etc. (merge in Step 3).
- Other field names: `bail!("pipe field accessor '.{}' not yet supported", field)`.

**Step 2 — Start with 1 voice** = `vec![base_synth_block.clone()]`.

**Step 3 — Apply transforms in order:**

`Replicate { n }`: clone the single block n times, produce n voices. Must come early in the chain (before per-voice transforms). If multiple voices already exist: error.

`Shift { semitones }`: for each voice, shift all notes by semitones. Implement a `shift_note(note: &str, semitones: i32) -> String` helper:
- Parse note name (e.g. "D4"): letter(s) + optional '#'/'b' + octave digit.
- Convert to MIDI: pitch_class * 1 + octave * 12 + 12 (middle C = 60).
- Add semitones, clamp to 0–127.
- Convert back to note name using sharps (C, C#, D, D#, E, F, F#, G, G#, A, A#, B).
- "-" passes through unchanged, unrecognized formats pass through unchanged with a warning.

`Spread { lo, hi }`: assign pan to each voice. Linear interpolation: voice i gets `lo + (hi - lo) * i as f32 / (n-1) as f32`. Single voice gets `(lo + hi) / 2`. Set as a fixed pan value — add `PanPrimitive::Fixed { value: f32 }` if it doesn't exist, OR set pan to the closest existing primitive. Use `PanPrimitive::Center` for 0.0, otherwise store as a new variant `PanPrimitive::Fixed { value }` in ir/types.rs.

`Tempo { seconds_per_note }`: set synth.tempo = format!("{}s/note", seconds_per_note) on each voice.

`Take { n }`: on each voice, truncate notes to first n elements.

`Repeat { n }`: on each voice, repeat notes n times (extend vec by cycling).

`Each { expr }`: parse expr as a mini transform applied per-voice with index substitution. Support `shift(semitones: i * N)` where `i` is the voice index. Evaluate `i * N` by substituting the index integer. Only support this specific pattern in this phase — other `each` expressions log a warning and are skipped.

`Map { expr }`: log a warning "map() not yet implemented, skipping" and pass through. (Full map support is post-phase.)

**Step 4 — Return:** collect as `Vec<(String, SynthBlock)>` where name = `format!("{}-pipe-{}", thing_name, i)`.

Add `pub mod executor;` to `src/pipe/mod.rs`.
  </action>
  <verify>
    <automated>cargo test -p hum-rt --lib pipe::executor -- --nocapture 2>&1 | tail -30</automated>
  </verify>
  <done>All executor unit tests pass including replicate+each+shift combo, spread pan distribution, take/repeat note slicing.</done>
</task>

<task type="auto">
  <name>Task 2: Wire expand_pipe into reconciler</name>
  <files>src/reconciler.rs</files>
  <action>
In `src/reconciler.rs`, after `resolve_refs` is called (Plan 1), add pipe expansion logic:

For each ThingDef where `thing.pipe.is_some()`:
1. Parse the pipe string: `parse_pipe_block(pipe_str)`.
2. Call `expand_pipe(thing_name, &expr, &piece)`.
3. On success: for each `(node_name, synth_block)` in the result, treat it as an independent thing to compile and load — call `compile_synth_block(&node_name, &synth_block)` and send to scsynth exactly as a regular synth thing would be.
4. Register expanded node names in state so the reconciler can diff/free them correctly on next reload.
5. On error: `tracing::error!("pipe expansion failed for '{}': {}", thing_name, e)` and skip.
6. The original pipe thing itself does NOT produce a node — only its expanded children do.

If the pipe source references a thing that also has a `pipe:` block, log a warning "nested pipe source '{}' not supported" and skip.

For the state reconciler diff: expanded nodes should be tracked under a key like `"{thing_name}::pipe::{i}"` so that removing/changing the pipe thing frees all its nodes. Use whatever pattern already exists for tracking active nodes — follow the existing convention in reconciler.rs exactly.
  </action>
  <verify>
    <automated>cargo build 2>&1 | grep -E "^error" | head -20; echo "exit: $?"</automated>
  </verify>
  <done>
    cargo build succeeds. A .hum file with a pipe: block compiles without error. The daemon runs without panic on pipe things.
  </done>
</task>

</tasks>

<verification>
cargo test -p hum-rt --lib 2>&1 | tail -10
</verification>

<success_criteria>
1. expand_pipe unit tests cover: replicate+each+shift, spread pan, take, repeat, source field accessor.
2. cargo build passes.
3. Pipe thing in piece.hum spawns N scsynth nodes with synthetic names {thing}-pipe-{i}.
4. All existing tests still pass.
</success_criteria>

<output>
After completion, create `.planning/phases/06-ref-pipe/06-3-SUMMARY.md`
</output>

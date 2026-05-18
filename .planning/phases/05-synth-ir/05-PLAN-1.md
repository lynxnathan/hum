---
phase: 05-synth-ir
plan: 1
type: execute
wave: 1
depends_on: []
files_modified:
  - src/ir/mod.rs
  - src/ir/types.rs
  - src/ir/notes.rs
  - src/parser/types.rs
  - src/main.rs
autonomous: true
requirements: [IR-01, IR-02, IR-03, IR-04, IR-05, IR-06, IR-07, IR-08, IR-09]

must_haves:
  truths:
    - "A .hum file with a synth: block parses without error"
    - "All osc/filter/env/distort/fx/pan/amp/tempo/notes fields deserialize into typed Rust enums"
    - "Note names (D4, Eb4, -) convert to MIDI numbers and frequencies"
    - "Unknown synth: fields are rejected with a parse error"
  artifacts:
    - path: "src/ir/mod.rs"
      provides: "ir module re-exports"
    - path: "src/ir/types.rs"
      provides: "SynthBlock, OscPrimitive, FilterPrimitive, EnvPrimitive, DistortPrimitive, FxPrimitive, PanPrimitive enums"
      exports: [SynthBlock, OscPrimitive, FilterPrimitive, EnvPrimitive, DistortPrimitive, FxPrimitive, PanPrimitive]
    - path: "src/ir/notes.rs"
      provides: "note_to_midi(), midi_to_freq(), parse_note_list()"
      exports: [note_to_midi, midi_to_freq, parse_note_list]
    - path: "src/parser/types.rs"
      provides: "ThingDef.synth field added"
      contains: "pub synth: Option<SynthBlock>"
  key_links:
    - from: "src/parser/types.rs"
      to: "src/ir/types.rs"
      via: "ThingDef.synth: Option<SynthBlock>"
      pattern: "use crate::ir::types::SynthBlock"
---

<objective>
Define the SynthIR type system and parse `synth:` blocks from .hum YAML into strongly-typed Rust enums. Also add note-name-to-frequency conversion.

Purpose: Downstream compiler (Plan 2) works against typed enums, not raw strings. Plan 3 wires the full pipeline.
Output: `src/ir/` module with types + notes; `ThingDef` gains `synth: Option<SynthBlock>`; existing tests still pass.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/phases/05-synth-ir/05-CONTEXT.md
@.planning/phases/05-synth-ir/research/RESEARCH.md
@.planning/v2-IR-DESIGN.md
@src/parser/types.rs
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: SynthBlock types + serde parsing</name>
  <files>src/ir/mod.rs, src/ir/types.rs, src/ir/notes.rs</files>
  <behavior>
    - SynthBlock { notes, osc, filter, env, distort, fx, pan, amp, tempo } all Option<_>
    - osc field parses "sine", "saw", "pulse(width: 0.5)", "noise(type: white)" → OscPrimitive enum
    - filter parses "lpf(cutoff: 800)", "hpf", "bpf(freq: 2000, q: 0.3)" → FilterPrimitive enum
    - env parses "perc(attack: 0.01, release: 0.5)", "adsr(a: 0.01, d: 0.1, s: 0.8, r: 0.3)" → EnvPrimitive enum
    - distort parses "tanh(drive: 2.0)", "bitcrush(bits: 8)" → DistortPrimitive enum
    - fx parses "reverb(mix: 0.7, room: 0.95)", "delay(time: 0.3, feedback: 0.5)" → FxPrimitive enum
    - pan parses "center", "noise(rate: 0.1, range: -0.5~0.5)", "lfo(rate: 0.05)" → PanPrimitive enum
    - amp parses as f32
    - tempo parses "0.35s/note" → f32 seconds
    - note_to_midi("D4") = 62, note_to_midi("Eb4") = 63, note_to_midi("-") = None (rest)
    - midi_to_freq(69) = 440.0 (within f32 epsilon)
    - parse_note_list(["D4", "Eb4", "-"]) = [Some(62), Some(63), None]
    - Unknown synth: field causes serde error (deny_unknown_fields)
  </behavior>
  <action>
    Create `src/ir/mod.rs` re-exporting types and notes submodules.

    Create `src/ir/types.rs`:
    - `SynthBlock` struct with `#[serde(deny_unknown_fields)]`, all fields Option<_>
    - Parse `osc`, `filter`, `env`, `distort`, `fx`, `pan` as `Option<String>` (raw string from YAML) — store raw for now, parsed by compiler
    - Actually: define typed enums and implement `FromStr` + `Deserialize` via `#[serde(try_from = "String")]` for each primitive
    - OscPrimitive: Sine, Saw, Pulse { width: f32 }, Noise { noise_type: NoiseType } where NoiseType = White|Pink|Brown
    - FilterPrimitive: Lpf { cutoff: f32 }, Hpf { cutoff: f32 }, Bpf { freq: f32, q: f32 }
    - EnvPrimitive: Perc { attack: f32, release: f32 }, Adsr { attack: f32, decay: f32, sustain: f32, release: f32 }
    - DistortPrimitive: Tanh { drive: f32 }, Bitcrush { bits: u8 }
    - FxPrimitive: Reverb { mix: f32, room: f32 }, Delay { time: f32, feedback: f32 }
    - PanPrimitive: Center, Noise { rate: f32, range: (f32, f32) }, Lfo { rate: f32 }
    - Parse "primitive(key: val, key: val)" syntax via a small hand-written parser: split on '(', parse key-value pairs from the args substring. No regex needed — use str::split and str::trim.
    - Default values: pulse width defaults to 0.5, bpf q defaults to 1.0, noise rate defaults to 0.1

    Create `src/ir/notes.rs`:
    - `pub fn note_to_midi(note: &str) -> Option<u8>` — per the reference impl in RESEARCH.md
    - `pub fn midi_to_freq(midi: u8) -> f32` — `440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0)`
    - `pub fn parse_note_list(notes: &[String]) -> Vec<Option<u8>>` — maps note_to_midi over the list

    Add unit tests in each file covering the behavior cases listed above.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo test ir:: 2>&1 | tail -20</automated>
  </verify>
  <done>All ir:: tests pass; cargo check exits 0</done>
</task>

<task type="auto">
  <name>Task 2: Wire ThingDef.synth + declare ir mod</name>
  <files>src/parser/types.rs, src/main.rs</files>
  <action>
    In `src/parser/types.rs`:
    - Add `use crate::ir::types::SynthBlock;` import
    - Add `pub synth: Option<SynthBlock>,` field to ThingDef
    - Update `make_thing()` helper in reconciler tests to include `synth: None`

    In `src/main.rs`:
    - Add `mod ir;` declaration alongside existing mods

    Ensure existing reconciler tests still compile: the `make_thing()` helper in reconciler.rs needs `synth: None` added.

    Run `cargo test` — all existing tests must still pass. The synth field is optional so no existing .hum parsing breaks.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo test 2>&1 | tail -20</automated>
  </verify>
  <done>cargo test passes; ThingDef has synth field; piece.hum (no synth: block) still parses correctly</done>
</task>

</tasks>

<verification>
- `cargo test` passes with no regressions
- `cargo check` exits 0
- A .hum snippet with `synth: { osc: sine, amp: 0.1 }` parses into `SynthBlock { osc: Some(OscPrimitive::Sine), amp: Some(0.1), .. }`
- A .hum snippet with `synth: { unknown_field: x }` produces a serde error
</verification>

<success_criteria>
IR type module exists with all primitive enums. ThingDef carries synth field. Note name conversion tested. No existing tests broken.
</success_criteria>

<output>
After completion, create `.planning/phases/05-synth-ir/05-1-SUMMARY.md`
</output>

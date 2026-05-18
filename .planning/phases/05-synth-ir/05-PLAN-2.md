---
phase: 05-synth-ir
plan: 2
type: execute
wave: 2
depends_on: [05-1]
files_modified:
  - src/ir/compiler.rs
  - src/ir/encoder.rs
  - src/ir/mod.rs
autonomous: true
requirements: [IR-10]

must_haves:
  truths:
    - "SynthBlock compiles to valid SCgf v2 binary bytes"
    - "The binary is accepted by scsynth via /d_recv without error"
    - "All osc/filter/env/distort/fx/pan combinations produce a non-empty byte Vec"
    - "A SinOsc synth produces audible 440Hz sine when /s_new is called"
  artifacts:
    - path: "src/ir/compiler.rs"
      provides: "compile_synth_block(name, SynthBlock) -> Result<Vec<u8>>"
      exports: [compile_synth_block]
    - path: "src/ir/encoder.rs"
      provides: "encode_synthdef(name, UgenGraph) -> Vec<u8> (SCgf v2 binary)"
      exports: [encode_synthdef, UgenGraph, UgenSpec, InputSpec]
  key_links:
    - from: "src/ir/compiler.rs"
      to: "src/ir/encoder.rs"
      via: "builds UgenGraph then calls encode_synthdef()"
      pattern: "encode_synthdef"
    - from: "src/ir/compiler.rs"
      to: "src/ir/notes.rs"
      via: "resolves note frequencies for freq parameter"
      pattern: "midi_to_freq|note_to_midi"
---

<objective>
Implement the SynthIR → SCgf v2 binary compiler. Takes a `SynthBlock` + thing name, builds a UGen graph in topological order, serializes to the SCgf binary format that scsynth accepts via `/d_recv`.

Purpose: This is the core technical deliverable of Phase 5 — pure Rust SynthDef generation, no sclang.
Output: `compile_synth_block(name, block) -> Result<Vec<u8>>` ready to pass to `ScsynthClient.load_synthdef()`.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/phases/05-synth-ir/05-CONTEXT.md
@.planning/phases/05-synth-ir/research/RESEARCH.md
@.planning/phases/05-synth-ir/05-1-SUMMARY.md
@src/ir/types.rs
@src/ir/notes.rs

<interfaces>
<!-- Key types from Plan 1 that this plan builds against -->

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
    pub tempo: Option<f32>,
}

pub enum OscPrimitive { Sine, Saw, Pulse { width: f32 }, Noise { noise_type: NoiseType } }
pub enum FilterPrimitive { Lpf { cutoff: f32 }, Hpf { cutoff: f32 }, Bpf { freq: f32, q: f32 } }
pub enum EnvPrimitive { Perc { attack: f32, release: f32 }, Adsr { attack: f32, decay: f32, sustain: f32, release: f32 } }
pub enum DistortPrimitive { Tanh { drive: f32 }, Bitcrush { bits: u8 } }
pub enum FxPrimitive { Reverb { mix: f32, room: f32 }, Delay { time: f32, feedback: f32 } }
pub enum PanPrimitive { Center, Noise { rate: f32, range: (f32, f32) }, Lfo { rate: f32 } }
```

From src/ir/notes.rs:
```rust
pub fn note_to_midi(note: &str) -> Option<u8>;
pub fn midi_to_freq(midi: u8) -> f32;
```

From src/osc/bridge.rs:
```rust
pub async fn load_synthdef(&mut self, synthdef_bytes: Vec<u8>) -> Result<()>;
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: SCgf binary encoder</name>
  <files>src/ir/encoder.rs</files>
  <behavior>
    - encode_synthdef("sine_test", graph) produces bytes starting with b"SCgf" + 0x00000002 + 0x0001
    - pstring encoding: "SinOsc" encodes as [0x06, b'S', b'i', b'n', b'O', b's', b'c']
    - A graph with 2 constants [440.0, 0.1] encodes them as big-endian IEEE 754 float32
    - A graph with params [("freq", 440.0), ("amp", 0.1)] encodes param_names section correctly
    - UGen with calc_rate=2 (ar), special_index=0, 2 inputs, 1 output encodes the 7 fields correctly
    - InputSpec::Constant(0) encodes as [-1, 0] (two int32s)
    - InputSpec::UgenOutput { ugen: 0, output: 0 } encodes as [0, 0]
    - num_variants = 0 at end
    - Round-trip: decode the encoded bytes offset-by-offset matches the verified sine_test.scsyndef layout from RESEARCH.md
  </behavior>
  <action>
    Create `src/ir/encoder.rs` with:

    ```rust
    pub struct UgenGraph {
        pub constants: Vec<f32>,
        pub params: Vec<(String, f32)>,   // (name, initial_value)
        pub ugens: Vec<UgenSpec>,
    }

    pub struct UgenSpec {
        pub class_name: String,
        pub calc_rate: u8,            // 0=ir, 1=kr, 2=ar
        pub special_index: i16,
        pub inputs: Vec<InputSpec>,
        pub outputs: Vec<u8>,         // calc_rate per output
    }

    pub enum InputSpec {
        Constant(usize),
        UgenOutput { ugen: usize, output: usize },
    }
    ```

    Implement `pub fn encode_synthdef(name: &str, graph: &UgenGraph) -> Vec<u8>` following the SCgf v2 spec from RESEARCH.md exactly:
    - File header: b"SCgf" + i32 version=2 + i16 num_synthdefs=1
    - SynthDef name as pstring (1-byte length prefix, NOT null-terminated)
    - Constants array: i32 count + f32[] big-endian
    - Parameters: i32 count + f32[] initial values + i32 param_name_count + [pstring name + i32 index][]
    - UGens: i32 count + per-ugen [pstring class_name, u8 calc_rate, i32 num_inputs, i32 num_outputs, i16 special_index, [i32 ugen_or_neg1, i32 output_or_const_idx][] inputs, [u8 rate][] outputs]
    - i16 num_variants = 0

    All integers big-endian. Use `i32::to_be_bytes()`, `f32::to_be_bytes()`, `i16::to_be_bytes()`.

    Add unit tests verifying the byte-level encoding against the hex values from RESEARCH.md.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo test ir::encoder 2>&1 | tail -20</automated>
  </verify>
  <done>Encoder tests pass; encode_synthdef produces correct byte layout per SCgf v2 spec</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: IR compiler — SynthBlock to UgenGraph</name>
  <files>src/ir/compiler.rs, src/ir/mod.rs</files>
  <behavior>
    - compile_synth_block("test", SynthBlock { osc: Sine, amp: Some(0.1), .. }) returns Ok(bytes) where bytes[0..4] == b"SCgf"
    - A Sine synth graph contains UGens: Control.ir, SinOsc.ar, BinaryOpUGen.ar(*), Out.ar — in that order
    - A Saw osc uses Saw.ar UGen (not SinOsc)
    - A Pulse osc uses Pulse.ar UGen with width constant
    - A Noise(white) osc uses WhiteNoise.ar with no inputs
    - Adding FilterPrimitive::Lpf inserts LPF.ar after the osc, before amp multiply
    - Adding FilterPrimitive::Bpf inserts BPF.ar; rq = 1.0/q passed as constant
    - Adding EnvPrimitive::Perc inserts EnvGen.ar with flattened Env.perc constants; gate param added to Control
    - Adding FxPrimitive::Reverb inserts FreeVerb.ar BEFORE Pan2 (mono input per RESEARCH.md pitfall)
    - Adding FxPrimitive::Delay inserts CombN.ar
    - Adding PanPrimitive::Center inserts Pan2.ar with pos=0.0 constant; Out.ar gets 2 channels
    - Adding PanPrimitive::Lfo inserts SinOsc.kr for pan position before Pan2.ar
    - compile_synth_block returns Err if osc is None (no oscillator source)
    - amp defaults to 0.3 if not specified (prevents silence)
    - freq defaults to 440.0 as Control parameter with initial value
  </behavior>
  <action>
    Create `src/ir/compiler.rs`:

    `pub fn compile_synth_block(name: &str, block: &SynthBlock) -> anyhow::Result<Vec<u8>>`

    Build a UgenGraph following the fixed signal chain from RESEARCH.md:
    1. Always add `Control.ir` first — num_outputs = number of params. Start with params: [("freq", 440.0), ("amp", block.amp.unwrap_or(0.3))]. Add "gate" param (initial 1.0) if env is ADSR.
    2. Add oscillator UGen based on block.osc:
       - Sine → SinOsc.ar, inputs: [freq from Control[0], phase constant 0.0]
       - Saw → Saw.ar, inputs: [freq from Control[0]]
       - Pulse { width } → Pulse.ar, inputs: [freq from Control[0], width constant]
       - Noise { White } → WhiteNoise.ar, no inputs
       - Noise { Pink } → PinkNoise.ar, no inputs
       - None → return Err("synth: block requires osc:")
    3. If block.filter is Some, add filter UGen reading from the osc output:
       - Lpf { cutoff } → LPF.ar, inputs: [osc_output, cutoff constant]
       - Hpf { cutoff } → HPF.ar, inputs: [osc_output, cutoff constant]
       - Bpf { freq, q } → BPF.ar, inputs: [osc_output, freq constant, rq=1/q constant]
       - Track "current signal" index = last added UGen index, output 0
    4. If block.env is Some, add EnvGen.ar with pre-computed Env constants:
       - Perc { attack, release }: Env constants array = [0.0 (init), 2.0 (segs), -1.0 (rel_node), -1.0 (loop), 1.0, attack, 1.0, 0.0, 0.0, release, -4.0, 0.0]
       - Adsr { a, d, s, r }: standard adsr constants
       - EnvGen inputs: [Env_constants_as_inputs (all constant refs), gate from Control["gate"], level_scale=1.0, level_bias=0.0, time_scale=1.0, doneAction=2.0]
       - Note: doneAction=2 is CRITICAL — prevents node accumulation (RESEARCH.md anti-pattern)
       - After adding EnvGen, add BinaryOpUGen.ar(special_index=2 = multiply) with inputs: [current_signal, envgen_output]
       - Update current_signal to this multiply UGen
    5. If block.distort is Some:
       - Tanh { drive } → add BinaryOpUGen(*, drive constant) then UnaryOpUGen(special_index=17) on result
       - Bitcrush { bits } → step = 2f32.powi(1 - bits as i32), add Round.ar with inputs: [signal, step constant]
    6. If block.fx is Some (must be mono signal before fx per pitfall):
       - Reverb { mix, room } → FreeVerb.ar with inputs: [signal, mix constant, room constant, damp=0.1 constant]
       - Delay { time, feedback } → CombN.ar with inputs: [signal, maxdelay=time constant, delaytime=time constant, decaytime derived from feedback]
    7. If block.pan is Some:
       - Center → Pan2.ar with inputs: [signal, pos=0.0 constant]
       - Lfo { rate } → first add SinOsc.kr with inputs: [rate constant, phase=0.0 constant], then Pan2.ar with inputs: [signal, sineosc_output]
       - Noise { rate, range } → first add LFNoise1.kr with inputs: [rate constant], then scale with BinaryOpUGen and offset, then Pan2.ar
       - After Pan2.ar, Out.ar gets 2 inputs (L and R from Pan2 outputs 0 and 1)
       - Without pan: Out.ar gets 1 input (mono signal)
    8. Add amp multiply: BinaryOpUGen.ar(special_index=2) with inputs: [signal, amp from Control["amp"]]
    9. Add Out.ar: inputs: [bus constant 0, amplified signal] (or L, R if pan present)

    Helper to add a constant to the graph and return its index.
    Helper to find parameter index by name for Control references.

    Update `src/ir/mod.rs` to re-export `compile_synth_block`.

    Write unit tests: verify output starts with b"SCgf", verify length > 0 for each primitive combination, verify Err on missing osc.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo test ir:: 2>&1 | tail -30</automated>
  </verify>
  <done>All ir:: tests pass including compiler tests; compile_synth_block("test", sine_block) returns Ok(bytes) starting with b"SCgf"</done>
</task>

</tasks>

<verification>
- `cargo test` passes with no regressions
- compile_synth_block produces bytes[0..4] == b"SCgf" for all primitive combinations
- All 9 IR requirement types (sine/saw/pulse/noise, lpf/hpf/bpf, perc/adsr, tanh/bitcrush, reverb/delay, center/lfo/noise-pan) compile without error
</verification>

<success_criteria>
`compile_synth_block(name, &block) -> Result<Vec<u8>>` works for all primitive combinations. The binary is structurally valid SCgf v2. Ready for Plan 3 to wire into main.rs.
</success_criteria>

<output>
After completion, create `.planning/phases/05-synth-ir/05-2-SUMMARY.md`
</output>

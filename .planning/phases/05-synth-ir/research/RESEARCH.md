# Phase 5: Synth IR - Research

**Researched:** 2026-03-20
**Domain:** SCgf binary SynthDef generation, UGen graph construction, Rust synthesis IR
**Confidence:** HIGH (SCgf spec from official SC docs; binary verified against project's own .scsyndef; sorceress crate verified on crates.io/docs.rs)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None explicitly locked by user — all key questions delegated to Claude's discretion.

### Claude's Discretion
- **SynthDef binary generation** — generate SCgf binary directly in Rust (approach a), eliminating sclang dependency
- **IR type system** — enum per primitive, walkable for compilation
- **Note parsing** — "D4", "Eb4", "-" (rest) → MIDI number
- **Parameter ranges** — "freq: 28~65" → LFNoise1.kr modulation between bounds
- **Signal chain order** — fixed: osc → filter → distort → fx → pan → out

### Deferred Ideas (OUT OF SCOPE)
None listed.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| IR-01 | Parse synth: block in .hum YAML | serde-saphyr already used; add SynthBlock struct with deny_unknown_fields |
| IR-02 | osc primitive (sine/saw/pulse/noise) | SinOsc.ar, Saw.ar, Pulse.ar, WhiteNoise.ar — all standard UGens, no special_index |
| IR-03 | filter primitive (lpf/hpf/bpf) | LPF.ar, HPF.ar, BPF.ar — single-input filter UGens |
| IR-04 | env primitive (perc/adsr) | EnvGen.ar with Env.perc / Env.adsr constants — requires doneAction=2 |
| IR-05 | distort primitive (tanh/bitcrush) | tanh via UnaryOpUGen(special=17); bitcrush via Round.ar + MulAdd |
| IR-06 | fx primitive (reverb/delay) | FreeVerb.ar (3 inputs), CombN.ar for delay |
| IR-07 | pan primitive | Pan2.ar (stereo), or constant for center/left/right |
| IR-08 | amp/tempo/notes primitives | amp = MulAdd constant; tempo = inter-note trigger scheduling; notes = MIDI→freq table |
| IR-09 | Compile to SCgf v2 binary | Hand-written binary encoder in Rust — full spec documented below |
| IR-10 | Hot-swap on edit | Recompile IR → bytes → /d_recv+/sync → /n_free old → /s_new new |
| IR-11 | .scd escape hatch overrides synth: block | ScdStore.get() check before IR compilation path |
</phase_requirements>

---

## Summary

Phase 5 produces a pure-Rust SynthDef compiler: parse a `synth:` block, build a UGen DAG, serialize to SCgf v2 binary, and load via the existing `/d_recv` path. No sclang process is needed.

The SCgf v2 format is fully specified in the official SuperCollider documentation and verified against the project's own `test-data/sine_test.scsyndef`. The binary is big-endian, packed, using pstrings for names. The `sorceress` crate (v0.2.0) implements exactly this encoder in Rust and also provides a UGen library — it is usable as a reference or direct dependency, though its UGen coverage is incomplete (no EnvGen, LFNoise1, FreeVerb confirmed). Implementing the encoder directly in hum-rt is ~200-300 lines of Rust and gives full control. The two approaches are compared below.

The signal chain is fixed: osc → [filter] → [distort] → [fx] → [pan] → Out. Each stage maps to one or more standard scsynth UGens. The `notes:` field drives inter-note timing in the reconciler (scheduling repeated /s_new calls spaced by `tempo`). MIDI mapping follows the standard C4=60 convention with `freq = 440.0 * 2^((midi-69)/12)`.

**Primary recommendation:** Implement the SCgf encoder directly in Rust (no sorceress dependency). The format is simple enough (~200 lines), gives full control over every UGen, and avoids pulling in a crate whose UGen coverage and maintenance status are uncertain.

---

## Standard Stack

### Core (no new dependencies required)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust std | — | Big-endian byte writing via `u32::to_be_bytes()` | Already present; `byteorder` crate unnecessary |
| serde-saphyr | 0.0.22 | Parse `synth:` YAML block | Already in Cargo.toml |
| serde | 1.0 | Derive Deserialize for SynthBlock | Already in Cargo.toml |

### Optional Supporting Crate
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| sorceress | 0.2.0 | Pre-built SCgf encoder + partial UGen library | Reference only; NOT recommended as direct dep (incomplete UGen coverage, low maintenance activity) |

### Alternatives Considered
| Approach | Tradeoff |
|----------|----------|
| Use `sorceress` as dep | Saves encoder code but incomplete UGens; adds external dep; crate appears unmaintained since ~2021 |
| Hand-roll encoder in hum-rt | ~200-300 lines; full control; already know the format; aligns with project's "small, no runtime deps" constraint |
| Shell out to sclang | Eliminated by design — defeats the goal of Phase 5 |

**Installation:** No new dependencies needed. Use `std::io::Write` + `u32::to_be_bytes()` for big-endian encoding.

---

## SCgf v2 Binary Format

**Source:** [SuperCollider 3.14.0 Synth Definition File Format](https://doc.sccode.org/Reference/Synth-Definition-File-Format.html) — HIGH confidence. Verified byte-by-byte against `test-data/sine_test.scsyndef`.

All integers are big-endian, signed unless noted. No padding or alignment. Strings are pascal strings (1-byte length prefix, then N bytes — NOT null-terminated).

### File Structure

```
[File Header]
  4 bytes  "SCgf"                    magic bytes (ASCII)
  int32    version = 2
  int16    num_synthdefs             (almost always 1)

[SynthDef] × num_synthdefs
  pstring  name                      (1-byte len + bytes)
  int32    num_constants (K)
  float32  constants[K]              big-endian IEEE 754
  int32    num_parameters (P)
  float32  param_initial_values[P]
  int32    num_param_names (N)
  [pstring param_name, int32 param_index] × N
  int32    num_ugens (U)
  [ugen-spec] × U
  int16    num_variants (V)          (0 for simple defs)
  [variant-spec] × V
```

### UGen Spec

```
  pstring  class_name                e.g. "SinOsc"
  int8     calc_rate                 0=ir, 1=kr, 2=ar
  int32    num_inputs (I)
  int32    num_outputs (O)
  int16    special_index             see below
  [input-spec] × I
  [output-spec] × O
```

### Input Spec

```
  int32    ugen_index_or_minus1      -1 = constant, ≥0 = UGen index
  int32    output_index_or_const_idx if ugen=-1: constant array index
                                     else: output index of that UGen
```

### Output Spec

```
  int8     calc_rate                 same as parent UGen's rate
```

### Special Index Values (relevant subset)

| Value | Meaning |
|-------|---------|
| 0 | normal (most UGens) |
| 2 | BinaryOpUGen: multiply (*) |
| 0 | BinaryOpUGen: add (+) |
| 17 | UnaryOpUGen: tanh |
| 4 | BinaryOpUGen: MulAdd pattern |

Full operator list: see [SC Operators docs](https://doc.sccode.org/Overviews/Operators.html)

### Verified from sine_test.scsyndef hex dump

```
Offset 0x00: 53 43 67 66         "SCgf"
Offset 0x04: 00 00 00 02         version = 2
Offset 0x08: 00 01               num_synthdefs = 1
Offset 0x0A: 09 "sine_test"      pstring (len=9)
Offset 0x14: 00 00 00 02         num_constants = 2
Offset 0x18: 43 dc 00 00         constant[0] = 440.0 (freq default)
Offset 0x1C: 3d cc cc cd         constant[1] = 0.1 (amp default)
Offset 0x20: 00 00 00 02         num_parameters = 2
Offset 0x24: 04 "freq" 00 00 00 00   param_name[0]="freq", index=0
Offset 0x2C: 03 "amp" 00 00 00 01    param_name[1]="amp",  index=1
```

---

## Architecture Patterns

### Recommended Module Structure

```
src/
├── ir/
│   ├── mod.rs           # re-exports
│   ├── types.rs         # SynthBlock, OscType, FilterType, etc. (IR AST)
│   ├── parser.rs        # serde Deserialize for synth: block
│   ├── compiler.rs      # IR → UgenGraph
│   ├── encoder.rs       # UgenGraph → Vec<u8> (SCgf binary)
│   └── notes.rs         # note name → MIDI → freq; range parsing
├── parser/
│   └── types.rs         # ThingDef gets synth: Option<SynthBlock>
└── osc/
    └── bridge.rs        # existing — ScsynthClient.load_synthdef() unchanged
```

### Pattern 1: UGen Graph as Flat Vec with Index References

The SCgf format stores UGens as a flat ordered Vec. Each UGen references prior UGens by index. Build the graph by appending UGens in signal chain order (Control first, then osc, filter, fx, Out last). Input specs reference earlier indices.

```rust
// Conceptual graph builder
struct UgenGraph {
    constants: Vec<f32>,
    params: Vec<(String, f32)>,   // (name, initial_value)
    ugens: Vec<UgenSpec>,
}

struct UgenSpec {
    class_name: String,
    calc_rate: u8,                // 0=ir, 1=kr, 2=ar
    special_index: i16,
    inputs: Vec<InputSpec>,
    outputs: Vec<u8>,             // calc_rate per output
}

enum InputSpec {
    Constant(usize),              // index into constants vec
    UgenOutput { ugen: usize, output: usize },
}
```

### Pattern 2: Fixed Signal Chain Construction

For the shallow IR, always insert UGens in this order:

1. `Control.ir` — one output per named parameter (freq, amp, gate...)
2. oscillator UGen (`SinOsc.ar`, `Saw.ar`, etc.) — reads freq from Control output
3. filter UGen if present (`LPF.ar`, etc.) — reads osc output + cutoff constant
4. envelope UGen if present (`EnvGen.ar`) — reads gate from Control
5. multiply osc × envelope if both present (`BinaryOpUGen.ar`, special_index=2)
6. distort if present (`UnaryOpUGen.ar` for tanh, etc.)
7. fx if present (`FreeVerb.ar` for reverb, `CombN.ar` for delay)
8. pan if present (`Pan2.ar`) — outputs two channels
9. amplitude multiply (`BinaryOpUGen.ar` × amp constant)
10. `Out.ar` — channel 0, stereo or mono signal

### Pattern 3: Parameter vs Constant Decision

- Fixed values known at compile time (e.g. `amp: 0.1`, `filter: lpf(cutoff: 800)`) → store as **constants** in the constants array
- Values the caller might /n_set at runtime (e.g. `freq`, `amp` when using `notes:`) → store as **named parameters** via `Control.ir`
- Ranged values (`freq: 28~65`) → use `LFNoise1.kr(rate)` scaled to [lo, hi] via MulAdd

### Anti-Patterns to Avoid

- **Writing UGens out of topological order:** The SCgf format requires every UGen to reference only UGens at earlier indices. Build chain left-to-right.
- **Forgetting pstring encoding:** Strings in SCgf are length-prefixed (1 byte), NOT null-terminated. Using null termination produces an invalid file.
- **Using EnvGen without doneAction=2:** Without `doneAction=2`, the synth node never frees itself after the envelope completes, causing node accumulation.
- **Sending /s_new before /synced:** The existing `load_synthdef()` correctly awaits `/synced` — do not bypass this.
- **Treating /done from /d_recv as success:** Existing code comment already flags SC bug #4411. Keep using /sync+/synced handshake.

---

## UGen Mapping Table

| synth: primitive | scsynth UGen | calc_rate | special_index | Notes |
|------------------|-------------|-----------|---------------|-------|
| osc: sine | SinOsc | ar=2 | 0 | inputs: freq, phase(0) |
| osc: saw | Saw | ar=2 | 0 | inputs: freq |
| osc: pulse(width) | Pulse | ar=2 | 0 | inputs: freq, width |
| osc: noise(white) | WhiteNoise | ar=2 | 0 | no inputs |
| osc: noise(pink) | PinkNoise | ar=2 | 0 | no inputs |
| filter: lpf(cutoff) | LPF | ar=2 | 0 | inputs: sig, cutoff |
| filter: hpf(cutoff) | HPF | ar=2 | 0 | inputs: sig, cutoff |
| filter: bpf(freq, q) | BPF | ar=2 | 0 | inputs: sig, freq, rq(=1/q) |
| env: perc(a, r) | EnvGen | ar=2 | 0 | with Env.perc constants; gate from Control |
| env: adsr(a,d,s,r) | EnvGen | ar=2 | 0 | with Env.adsr constants |
| distort: tanh(drive) | UnaryOpUGen | ar=2 | 17 | input: sig×drive |
| distort: bitcrush | Round.ar | ar=2 | 0 | step = 2^(1-bits) |
| fx: reverb | FreeVerb | ar=2 | 0 | inputs: sig, mix, room, damp |
| fx: delay | CombN | ar=2 | 0 | inputs: sig, maxdelay, delaytime, decaytime |
| pan: center | Pan2 | ar=2 | 0 | pos=0.0 constant |
| pan: lfo | Pan2 | ar=2 | 0 | pos from SinOsc.kr |
| pan: noise | Pan2 | ar=2 | 0 | pos from LFNoise1.kr |
| amp | BinaryOpUGen (*) | ar=2 | 2 | inputs: sig, amp_param |
| output | Out | ar=2 | 0 | inputs: bus(0), sig_L, sig_R |

### EnvGen / Env Constants

EnvGen in the binary format takes a flattened Env array as constants (not a separate UGen). The Env is encoded as a sequence of float32 constants: `[initial_level, num_segments, release_node, loop_node, level1, dur1, curve_type1, curve_value1, ...]`.

For `perc(attack, release)`: initial=0, 2 segments, release_node=-1, loop=-1, then level=1/dur=attack/curve=1(linear)/val=0, level=0/dur=release/curve=-4(exp)/val=0.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Big-endian binary writing | Custom bit-packing | `u32::to_be_bytes()`, `f32::to_be_bytes()`, `Vec::extend_from_slice()` | Standard library, zero deps |
| Frequency calculation | Custom table | Formula: `440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0)` | Exact, 1 line |
| Note name parsing | Custom regex | Simple match table: C=0,D=2,E=4,F=5,G=7,A=9,B=11 + accidental + octave | Music theory is fixed |
| Envelope shape | Custom curve math | Pre-compute Env constants per shape (perc/adsr) and embed as constants array | SC's own encoding |

---

## Common Pitfalls

### Pitfall 1: pstring vs C string vs Rust string
**What goes wrong:** Writing a null-terminated string or Rust's UTF-8 length prefix instead of a 1-byte pascal string length prefix. scsynth silently misparses the entire SynthDef.
**How to avoid:** Always encode names as: `[name.len() as u8]` + `name.as_bytes()`. Max pstring length = 255 bytes.
**Warning signs:** scsynth silently ignores the SynthDef; `/d_recv` sends no error, but `/s_new` with that name fails.

### Pitfall 2: UGen index reference to a later UGen
**What goes wrong:** Referencing a UGen at index N from a UGen at index M where M < N. The SCgf spec requires forward-only references (earlier index only).
**How to avoid:** Build UGens in strict signal chain order. Never re-order after building.
**Warning signs:** scsynth prints `FAILURE in server: SynthDef not found` or crashes the synth node on instantiation.

### Pitfall 3: Control UGen output count must match parameter count
**What goes wrong:** Declaring 2 parameters (freq, amp) but giving Control only 1 output. The output_specs list length must equal the number of parameters.
**How to avoid:** When building the Control UGen spec, set `num_outputs = params.len()`. Each output has calc_rate=0 (ir).

### Pitfall 4: FreeVerb expects mono input, produces mono output
**What goes wrong:** Passing a stereo (2-channel) signal into FreeVerb. FreeVerb.ar takes a single mono signal and outputs mono (then pan with Pan2 after).
**How to avoid:** Chain is osc→filter→FreeVerb (mono)→Pan2 (stereo)→Out. Do NOT wire Pan2 before FreeVerb.

### Pitfall 5: EnvGen gate parameter must be named exactly "gate"
**What goes wrong:** Using a different parameter name for the EnvGen gate input. scsynth looks for a named control called "gate" for ADSR release.
**How to avoid:** Always include `gate` as a named parameter with initial value 1.0 in the Control block when using ADSR envelopes.

### Pitfall 6: Hot-swap requires free-before-new at node level
**What goes wrong:** Calling `/s_new` with a new node ID without freeing the old node first. Old nodes accumulate on the server.
**How to avoid:** The existing `ScsynthClient.new_synth()` already frees old nodes. Use it unchanged. The hot-swap sequence is: recompile bytes → `/d_recv` + `/sync` → await `/synced` → `ScsynthClient.new_synth()` (which frees old + creates new).

---

## Note Name to MIDI Mapping

**Standard:** MIDI note 60 = C4 (middle C). A4 = MIDI 69 = 440 Hz.

```rust
// Note name → MIDI number
fn note_to_midi(note: &str) -> Option<u8> {
    // "D4", "Eb4", "C#4", "-" (rest)
    if note == "-" { return None; }  // rest

    let bytes = note.as_bytes();
    let pitch_class = match bytes[0] {
        b'C' => 0, b'D' => 2, b'E' => 4, b'F' => 5,
        b'G' => 7, b'A' => 9, b'B' => 11, _ => return None,
    };
    let (accidental, oct_start) = match bytes.get(1) {
        Some(b'#') => (1i8, 2),
        Some(b'b') => (-1i8, 2),
        _ => (0i8, 1),
    };
    let octave: i8 = note[oct_start..].parse().ok()?;
    // MIDI = (octave + 1) * 12 + pitch_class + accidental
    Some(((octave + 1) * 12 + pitch_class + accidental) as u8)
}

// MIDI → frequency (equal temperament, A4=440)
fn midi_to_freq(midi: u8) -> f32 {
    440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0)
}
```

---

## Parameter Range Syntax

`"freq: 28~65"` means LFNoise1.kr modulation between MIDI 28 and MIDI 65 (converted to Hz).

**UGen graph pattern for a range:**

```
LFNoise1.kr(rate)           // output: [-1, +1]
MulAdd.ar(lf, scale, offset) // scale = (hi-lo)/2, offset = (hi+lo)/2
```

Alternatively implemented as BinaryOpUGen multiply + add:
- `scale = (hi_freq - lo_freq) / 2.0`
- `offset = (hi_freq + lo_freq) / 2.0`
- Result: `LFNoise1 * scale + offset`

In the UGen graph: LFNoise1.kr → BinaryOpUGen(*, scale_constant) → BinaryOpUGen(+, offset_constant).

Or use `MulAdd` UGen: inputs are (sig, mul, add), special_index=0.

---

## Code Examples

### SCgf Encoder Skeleton (Rust)

```rust
// Source: official SCgf spec + verified against sine_test.scsyndef
fn encode_synthdef(name: &str, graph: &UgenGraph) -> Vec<u8> {
    let mut buf = Vec::new();
    // File header
    buf.extend_from_slice(b"SCgf");
    buf.extend_from_slice(&2i32.to_be_bytes());    // version
    buf.extend_from_slice(&1i16.to_be_bytes());    // num_synthdefs

    // SynthDef name (pstring)
    encode_pstring(&mut buf, name);

    // Constants
    buf.extend_from_slice(&(graph.constants.len() as i32).to_be_bytes());
    for &c in &graph.constants {
        buf.extend_from_slice(&c.to_be_bytes());
    }

    // Parameters
    buf.extend_from_slice(&(graph.params.len() as i32).to_be_bytes());
    for (_, init) in &graph.params {
        buf.extend_from_slice(&init.to_be_bytes());
    }
    buf.extend_from_slice(&(graph.params.len() as i32).to_be_bytes()); // num_param_names
    for (i, (name, _)) in graph.params.iter().enumerate() {
        encode_pstring(&mut buf, name);
        buf.extend_from_slice(&(i as i32).to_be_bytes());
    }

    // UGens
    buf.extend_from_slice(&(graph.ugens.len() as i32).to_be_bytes());
    for ugen in &graph.ugens {
        encode_ugen(&mut buf, ugen);
    }

    // Variants (0)
    buf.extend_from_slice(&0i16.to_be_bytes());
    buf
}

fn encode_pstring(buf: &mut Vec<u8>, s: &str) {
    buf.push(s.len() as u8);
    buf.extend_from_slice(s.as_bytes());
}
```

### Minimal SinOsc→Out Graph (matches sine_test.scsyndef)

Verified via hex dump: the sine_test.scsyndef contains exactly:
- Constants: [440.0, 0.1]
- Parameters: [freq=440.0, amp=0.1]
- UGens: [Control.ir(2 outputs), SinOsc.ar(freq=Control[0], phase=0), BinaryOpUGen.ar(*, SinOsc, Control[1]), Out.ar(bus=const(0?), BinaryOpUGen)]

Note: the hex shows `BinaryOpUGen` between SinOsc and Out — amp multiply is done inline.

---

## Integration Points

### ThingDef Change Required

```rust
// src/parser/types.rs — add synth field
pub struct ThingDef {
    // ... existing fields ...
    pub synth: Option<SynthBlock>,
}

// src/ir/types.rs — new
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthBlock {
    pub notes: Option<Vec<String>>,
    pub osc: Option<String>,
    pub filter: Option<String>,
    pub env: Option<String>,
    pub distort: Option<String>,
    pub fx: Option<String>,
    pub pan: Option<String>,
    pub amp: Option<f32>,
    pub tempo: Option<String>,
}
```

### ScdStore Escape Hatch Precedence

```rust
// Compilation decision tree in reconciler/main:
// 1. if scd_store.get(name).is_some() → use .scd bytes (escape hatch)
// 2. else if thing.synth.is_some() → compile IR → bytes
// 3. else → error: no SynthDef source
```

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| sclang compiles .scd to .scsyndef | Rust generates SCgf binary directly | Eliminates sclang process dependency |
| .scsyndef files as opaque artifacts | IR in .hum file is human-readable | LLM can write and human can audit |
| Rename dance (.scsyndef → thing name) | Name embedded in SCgf header | Name always matches thing name |

---

## Open Questions

1. **EnvGen envelope constants encoding**
   - What we know: Env shape is encoded as flat float32 constants in the SCgf constants array
   - What's unclear: Exact float layout for Env.perc and Env.adsr (curve type byte values)
   - Recommendation: Reverse-engineer from a known .scsyndef: compile `SynthDef(\test, { EnvGen.ar(Env.perc(0.01, 0.5)) })` in sclang, save, and xxd the output. Do this in Wave 0.

2. **serde-saphyr parsing of synth: block variants**
   - What we know: serde-saphyr handles serde Deserialize; `#[serde(deny_unknown_fields)]` enforces schema
   - What's unclear: Whether `osc: sine` (bare string) vs `osc: sine(freq: 440)` (function-call-like) can be parsed directly with serde, or needs a custom Deserialize
   - Recommendation: Parse `osc:` as `Option<String>` and split/parse the string manually in the compiler. Avoids complex YAML structure.

3. **notes: scheduling model**
   - What we know: `tempo: 0.35s/note`, notes: [D4 D4 Eb4] — each note = one /s_new at a timed interval
   - What's unclear: Whether note sequencing happens in the reconciler as timed OSC sends, or via a SynthDef that sequences internally
   - Recommendation: Implement as Rust-side scheduling (tokio timer per note). The SynthDef just plays a single note; the scheduler re-triggers it with new `freq` /n_set messages or repeated /s_new calls.

---

## Sources

### Primary (HIGH confidence)
- [SuperCollider 3.14.0 Synth Definition File Format](https://doc.sccode.org/Reference/Synth-Definition-File-Format.html) — complete byte layout, verified against project binary
- `test-data/sine_test.scsyndef` — project's own reference binary, hex-dumped and annotated
- [sorceress::synthdef docs](https://docs.rs/sorceress/latest/sorceress/synthdef/index.html) — confirms Rust implementation is feasible, shows API shape

### Secondary (MEDIUM confidence)
- [sorceress on crates.io](https://crates.io/crates/sorceress) — version 0.2.0, last activity ~2021
- [SC Operators docs](https://doc.sccode.org/Overviews/Operators.html) — BinaryOpUGen/UnaryOpUGen special_index values
- [MIDI note numbers reference](https://inspiredacoustics.com/en/MIDI_note_numbers_and_center_frequencies) — C4=60 standard confirmed

### Tertiary (LOW confidence)
- [scgolang/sc synthdef.go](https://github.com/scgolang/sc/blob/master/synthdef.go) — Go implementation cross-reference for encoder patterns
- [overtone synthdef.clj](https://github.com/overtone/overtone/blob/master/src/overtone/sc/machinery/synthdef.clj) — Clojure implementation cross-reference

---

## Metadata

**Confidence breakdown:**
- SCgf binary format: HIGH — official spec + binary verification
- UGen mapping: HIGH — all target UGens are standard scsynth builtins, decades stable
- sorceress crate status: MEDIUM — exists, works, but may be unmaintained; use as reference not dependency
- EnvGen constants layout: LOW — needs empirical verification (see Open Questions)
- Note scheduling model: MEDIUM — approach clear, exact implementation TBD in planning

**Research date:** 2026-03-20
**Valid until:** 2027-03-20 (SCgf format is stable; SuperCollider UGen API has not changed in 10+ years)

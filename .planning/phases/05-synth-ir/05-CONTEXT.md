# Phase 5: Synth IR - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

hum-rt parses `synth:` blocks inside .hum thing definitions and compiles them directly to scsynth OSC messages — no sclang, no .scd files needed. Covers: IR type system, YAML deserialization, all synth primitives (osc, filter, env, distort, fx, pan, amp, tempo, notes), SynthDef binary generation, hot-swap on edit, and .scd escape hatch precedence. Does NOT cover: pipe language, ref resolution, instruments, stages, or TUI.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion

User trusts Claude's judgment. Key open areas:

- **SynthDef binary generation** — hum-rt must produce valid .scsyndef binary format that scsynth accepts via /d_recv. Two approaches: (a) generate the binary directly in Rust by implementing the SCgf format, or (b) generate SC source and shell out to sclang. Approach (a) eliminates sclang dependency entirely.
- **IR type system** — How to represent the synth: block as Rust types. Enum per primitive? Trait-based? The IR needs to be walkable for compilation.
- **Note parsing** — "D4", "Eb4", "-" (rest) → MIDI numbers. Standard music theory mapping.
- **Parameter ranges** — "freq: 28~65" means LFNoise1.kr modulation between those bounds. How to represent ranges vs fixed values.
- **Signal chain order** — osc → filter → distort → fx → pan → out. Fixed order or user-specified?

</decisions>

<specifics>
## Specific Ideas

- The SCgf (SynthDef) binary format is documented: magic bytes "SCgf", version 2, then UGen graph
- Each synth: primitive maps to one or more scsynth UGens
- osc: sine → SinOsc.ar, saw → Saw.ar, pulse → Pulse.ar, noise → WhiteNoise.ar etc.
- filter: lpf → LPF.ar, hpf → HPF.ar, bpf → BPF.ar
- The SynthDef name must match the thing name (existing convention from v1)
- Hot-swap: on synth: change, recompile IR → binary, load via /d_recv+/sync, free old node, create new
- Escape hatch: if out/sc/<thing>.scd exists, skip synth: compilation for that thing

</specifics>

<code_context>
## Existing Code

### Key Integration Points
- `src/parser/types.rs` — ThingDef struct needs new `synth:` field (Option<SynthBlock>)
- `src/scd/store.rs` — ScdStore.get() checks for .scd override; IR compiler is the fallback
- `src/osc/bridge.rs` — ScsynthClient.load_synthdef(bytes) accepts raw .scsyndef bytes
- `src/main.rs` — startup loads ScdStore then parses piece.hum; needs IR compilation step between parse and reconcile
- `src/reconciler.rs` — diff() produces Add/Remove ops; Add needs SynthDef bytes from IR or ScdStore

### Existing Dependencies
- serde-saphyr (YAML parsing)
- rosc (OSC encoding)
- tokio, anyhow, thiserror, tracing

</code_context>

<deferred>
## Deferred Ideas

None

</deferred>

---
*Phase: 05-synth-ir*
*Context gathered: 2026-03-20*

# Phase 7: Instruments + Stage Effects - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Reusable instrument definitions loaded from instruments/ directory, and group-level stage effects that route multiple things through a shared bus with fx chain. Does NOT cover: pipe language, ref resolution, TUI, or transport.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion

- **Instrument file format** — .hum files with `type: instrument` + synth: block
- **Instrument loader** — scan instruments/ on startup, build InstrumentStore (name → SynthBlock)
- **Instrument override** — thing's synth: fields merge over instrument's synth: fields
- **Stage routing** — scsynth groups + shared bus. Stage thing's fx: applied as effect nodes on the group bus.
- **applies-to resolution** — static list of thing names, or pattern matching?

</decisions>

<specifics>
## Key Design from v2 Docs

instruments/berimbal-amp.hum:
```yaml
type: instrument
synth:
  osc: saw
  filter: bpf(freq: 800, q: 2.0)
  distort: tanh(drive: 4.0)
  env: adsr(0.01, 0.1, 0.6, 0.3)
```

Used in piece:
```yaml
glass:
  instrument: berimbal-amp
  synth:
    notes: [D4 D4 Eb4 D4]
    tempo: 0.35
```

Stage:
```yaml
haunted-stage:
  type: stage
  applies-to: [ghost-machine, glass, bass-drop]
  fx: reverb(mix: 0.7, room: 0.95)
```

Stage maps to scsynth: create Group node, route applies-to things into that group, add effect SynthDef on the group bus tail.

</specifics>

<code_context>
## Existing Code

- `src/ir/types.rs` — SynthBlock with all primitive fields
- `src/ir/compiler.rs` — compile_synth_block(name, &SynthBlock) → Vec<u8>
- `src/parser/types.rs` — ThingDef (needs `instrument:` and `type:` fields)
- `src/main.rs` — startup loads piece.hum, compiles IR, reconciles
- `src/osc/bridge.rs` — ScsynthClient (needs group creation for stages)

</code_context>

<deferred>
## Deferred Ideas

None

</deferred>

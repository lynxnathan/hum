# Phase 6: Ref Resolution + Pipe Language - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Things can reference other things' synth fields via `ref:`, and `pipe:` blocks enable Elixir-style functional composition of sound transforms. Does NOT cover: instruments (done), stage effects (done), TUI (done), or transport.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion

- **Ref resolver** — when to resolve refs (parse time vs compile time). Parse time is simpler.
- **Pipe parser** — custom tokenizer for `|>` chain syntax inside YAML multiline string
- **Pipe evaluator** — how pipes expand to concrete SynthBlocks or multiple synth nodes
- **replicate(n)** — creates n copies of a thing with different node IDs, each gets `each(i =>)` transforms applied
- **Field accessor** — `ref(thing).notes` syntax, how to parse the dot notation
- **Pipe output** — does a pipe produce one SynthBlock or multiple? replicate(3) implies multiple nodes

</decisions>

<specifics>
## Key Design from v2-PIPE-LANG.md

```yaml
# Ref: inherit + override
glass-drum:
  ref: glass
  synth:
    notes: ref(glass)
    osc: noise(type: white)
    env: perc(0.001, 0.05)
    tempo: 0.117

# Pipe: functional composition
glass-swarm:
  at: 20s
  pipe: |
    glass
    |> replicate(3)
    |> each(i => shift(semitones: i * 4))
    |> spread(pan: -0.8~0.8)
```

Pipe transforms:
- replicate(n) — clone into n voices
- each(i => expr) — per-voice with index
- map(n => expr) — transform each note
- shift(semitones) — pitch shift
- spread(pan: range) — distribute stereo
- tempo(duration) — change speed
- take(n), repeat(n) — sequence ops

</specifics>

<code_context>
## Existing Code

- `src/ir/types.rs` — SynthBlock with all fields including notes, tempo
- `src/ir/compiler.rs` — compile_synth_block(name, &SynthBlock) → Vec<u8>
- `src/ir/notes.rs` — note_to_midi, midi_to_freq
- `src/parser/types.rs` — ThingDef with ref field (Option<String>), synth (Option<SynthBlock>)
- `src/instruments.rs` — SynthBlock::merge(base, override) already exists
- `src/main.rs` — resolve_synth_block helper, startup IR compilation

### Key: SynthBlock::merge already works for instruments
The same merge logic applies to ref: resolution. ref(thing) pulls the referenced thing's SynthBlock as base, local synth: fields override.

</code_context>

<deferred>
## Deferred Ideas

- `shuffle` and `reverse` pipe transforms — v3
- Nested pipes (pipe output as pipe input) — v3
- Pattern matching in pipe sources — v3

</deferred>

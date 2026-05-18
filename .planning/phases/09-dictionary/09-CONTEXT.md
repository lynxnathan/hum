# Phase 9: Dictionary - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

hum-rt loads and exposes a shared vocabulary (hum.dict) that maps vibes/words to synth parameter sets. Project-level + user-level dicts merge. Things can reference dict entries via `style:` field. CLI introspection. Hot-reload. Does NOT cover: translation sync, Makepad GUI, or creative assistant.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion

- Dict file format (YAML with term → synth mapping)
- DictStore data structure (HashMap<String, SynthBlock> or similar)
- Merge strategy (project overrides global, or vice versa)
- How `style: laser` resolves — merge dict SynthBlock as base, thing's synth: overrides (same pattern as instruments)
- CLI subcommands: `hum dict list`, `hum dict show <term>`
- Hot-reload: watch hum.dict alongside piece.hum

</decisions>

<specifics>
## From v3-DICTIONARY-SYNC.md

```yaml
# hum.dict
laser:
  synth:
    osc: "sine + fm(ratio: 3.01)"
  context: bright, cutting, sci-fi
  learned-from: glass (2026-03-20)

warm:
  synth:
    filter: "lpf(cutoff: 800)"
    osc-prefer: sine
  context: soft, intimate, analog
```

Hierarchy: ~/.config/hum/global.dict (personal) < ./hum.dict (project)

Used in piece:
```yaml
glass:
  style: laser
  synth:
    notes: [D4, D4, Eb4, D4]
    tempo: 0.35
```

`style: laser` pulls the dict entry's synth fields as base, thing's synth: overrides.

</specifics>

<code_context>
## Existing Code

- `src/instruments.rs` — InstrumentStore + SynthBlock::merge (REUSE for dict merge)
- `src/parser/types.rs` — ThingDef (needs `style:` field)
- `src/ir/types.rs` — SynthBlock
- `src/transport.rs` — TransportCmd/Reply (needs Dict variants for CLI)
- `src/main.rs` — startup, event loop, hot-reload
- `src/watcher.rs` — start_watcher (needs to watch hum.dict)

Key insight: DictStore is structurally identical to InstrumentStore — both map a name to a SynthBlock. Could even share the same base type.

</code_context>

<deferred>
None
</deferred>

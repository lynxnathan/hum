# Phase 10: Translation Sync - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Detect divergence between .hum layers (like:/pipe:/synth:), mark stale layers with comments, capture new vocabulary from approved sounds, and suggest dictionary entries from recurring patterns. Does NOT cover: Makepad GUI, creative assistant, or dict loading (done in Phase 9).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion

- How to detect synth: manual edits (hash comparison? timestamp?)
- Comment format for divergence markers (# synth: manually tuned, pipe: may be stale)
- hum dict add <term> — captures current synth: block of a thing as a new dict entry
- hum dict suggest — pattern analysis (find repeated synth: param combinations across things)
- SYNC-01 (LLM regenerates pipe/synth from like:) is an LLM responsibility, not hum-rt code — hum-rt just needs to detect when like: changed vs synth: changed

</decisions>

<specifics>
## Key Design from v3-DICTIONARY-SYNC.md

Sync protocol:
- if like: changed → LLM regenerates pipe: and synth: (external)
- if pipe: changed → synth: regenerated from pipe expansion (hum-rt does this already)
- if synth: changed directly → mark divergence comment
- Divergence is OK — manual tuning is valid, just document it

hum dict add <term> workflow:
1. User tunes a sound they like
2. `hum dict add warm-bass` — captures the thing's resolved synth: block as a dict entry
3. Entry added to hum.dict with learned-from metadata

hum dict suggest workflow:
1. Scan all things in piece.hum
2. Find repeated synth: patterns (same osc type, similar filter values)
3. Output: "3 things use sine + lpf(~800) — consider adding 'mellow' to dict"

</specifics>

<code_context>
## Existing Code

- src/dict.rs — DictStore (load, get, merge)
- src/parser/types.rs — ThingDef with synth:, pipe:, style:, like:
- src/pipe/executor.rs — expand_pipe
- src/transport.rs — TransportCmd (needs DictAdd, DictSuggest variants)
- src/main.rs — CLI dispatch, event loop

</code_context>

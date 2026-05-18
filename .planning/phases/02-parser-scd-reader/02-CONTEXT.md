# Phase 2: Parser + SCD Reader - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

hum-rt parses .hum YAML files with strict schema validation and reads compiled .scd/.scsyndef files from out/sc/, associating them with thing names from the piece. Does NOT cover: file watching, state reconciliation, timeline, or transport.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion

User trusts Claude's judgment on all Phase 2 decisions. Open areas:

- **YAML crate choice** — serde_yaml is deprecated. Research identified yaml_serde 0.10 or serde-saphyr as alternatives. Pick whichever compiles cleanly with deny_unknown_fields.
- **Data model design** — How to structure the Piece/Thing types. Runtime-actionable fields (at, until, does, where) vs LLM-facing (like, ref, mood).
- **Error formatting** — How to present parse errors with line/field/suggestion. Level of detail.
- **SCD association strategy** — How to map .scd/.scsyndef files to thing names (filename convention vs manifest).
- **has/within nesting** — How deep nesting goes, how sub-things are represented in the data model.

</decisions>

<specifics>
## Specific Ideas

- .hum format is YAML where top-level keys are thing names
- Fields: at, until, does (trajectories), where (stereo position), has (sub-components), within (parent relationship), every (repetition), mood, like, ref
- Strict schema: deny_unknown_fields — reject anything not in the spec
- "Your names are the names" — space-crackle stays space-crackle, never becomes synth_pad_01
- .scd files in out/sc/ are compiled SynthDefs — binary .scsyndef format
- Phase 1 proved include_bytes! works for loading .scsyndef into scsynth

</specifics>

<code_context>
## Existing Code

### From Phase 1
- `src/config.rs` — Config::load() with layered resolution
- `src/osc/bridge.rs` — ScsynthClient with load_synthdef(bytes), new_synth(thing_name, synthdef_name)
- `src/main.rs` — smoke test wiring, will be updated to use parser output
- `Cargo.toml` — already has serde, tokio, rosc, toml, anyhow, thiserror, tracing

### Integration Points
- Parser output feeds into ScsynthClient::load_synthdef() and new_synth()
- Thing names from .hum must match .scsyndef filenames in out/sc/

</code_context>

<deferred>
## Deferred Ideas

None

</deferred>

---
*Phase: 02-parser-scd-reader*
*Context gathered: 2026-03-20*

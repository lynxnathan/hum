# Phase 3: State, Reconciler + File Watcher - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

The core feedback loop: hum-rt watches piece.hum and out/sc/ for changes, diffs desired vs current audio state, and sends minimal OSC deltas to scsynth. Things activate/deactivate based on their at:/until: times as playback advances. Does NOT cover: transport controls (play/stop/seek/loop/solo/mute) or CLI — that's Phase 4.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion

User trusts Claude's judgment on all Phase 3 decisions. Open areas:

- **State store design** — How to track desired state (from parser) vs actual state (what scsynth nodes exist). Kubernetes-style reconciliation loop.
- **Diff algorithm** — How to compute minimal delta between old and new piece.hum parse results. Add/remove/update detection.
- **File watcher setup** — notify crate with debounce. inotify for native paths, PollWatcher for /mnt/ NTFS paths.
- **Timeline tick** — How to advance playback position and trigger at:/until: transitions. Tick interval, precision.
- **SynthDef crossfade on .scd change** — How to reload a SynthDef and crossfade from old to new sound without audible click.
- **Event bus design** — tokio mpsc channel connecting file watcher, timeline ticker, and reconciler.

</decisions>

<specifics>
## Specific Ideas

- From architecture research: single event loop owns all state, all producers send to one mpsc receiver
- Debounce is non-negotiable — editors write temp files and rename (3-5 inotify events per save)
- inotify is broken for /mnt/c/ paths — must detect and use PollWatcher
- Reconciler pattern: hold desired state + actual state, diff, emit minimal OSC delta
- Named thing identity is load-bearing — thing name is the stable key across reloads
- Phase 1 ScsynthClient already has: load_synthdef, new_synth, set_param, free_node, free_all_nodes
- Phase 2 parser already has: parse_hum returning IndexMap<String, ThingDef>
- Phase 2 ScdStore already has: load_dir, get(thing_name) -> bytes

</specifics>

<code_context>
## Existing Code

### From Phase 1
- `src/osc/bridge.rs` — ScsynthClient (connect, check_alive, load_synthdef, new_synth, set_param, free_node, free_all_nodes)
- `src/config.rs` — Config::load()

### From Phase 2
- `src/parser/types.rs` — Piece (IndexMap<String, ThingDef>), ThingDef with all .hum fields
- `src/parser/mod.rs` — parse_hum(yaml_str) -> Result<Piece>
- `src/scd/store.rs` — ScdStore::load_dir(path), get(thing_name) -> Option<&[u8]>

### Integration Points
- Parser output (Piece) becomes the "desired state"
- ScsynthClient methods become the "actuators" for reconciliation
- ScdStore provides SynthDef bytes for loading

</code_context>

<deferred>
## Deferred Ideas

None

</deferred>

---
*Phase: 03-state-reconciler-watcher*
*Context gathered: 2026-03-20*

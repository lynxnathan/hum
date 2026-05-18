---
phase: 06-ref-pipe
plan: 3
subsystem: pipe
tags: [pipe, executor, note-shifting, spread-pan, reconciler, hot-reload]

requires:
  - phase: 06-ref-pipe plan 1
    provides: ref resolution (resolve_refs)
  - phase: 06-ref-pipe plan 2
    provides: pipe types (PipeExpr, PipeSource, Transform)
  - phase: 05-synth-ir
    provides: compile_synth_block, note_to_midi, NoteSequencer
provides:
  - expand_pipe(thing_name, expr, piece) -> Vec<(String, SynthBlock)>
  - parse_pipe_block (full implementation, was stub)
  - PanPrimitive::Fixed variant for spread() pan distribution
  - pipe: field on ThingDef
  - Pipe expansion wired into startup + hot-reload paths
affects: [08-gpui-dashboard]

tech-stack:
  added: []
  patterns: [pipe-expansion-before-compilation, synthetic-thing-injection]

key-files:
  created:
    - src/pipe/executor.rs
  modified:
    - src/pipe/parser.rs
    - src/pipe/mod.rs
    - src/ir/types.rs
    - src/ir/compiler.rs
    - src/parser/types.rs
    - src/main.rs
    - src/ir/ref_resolver.rs
    - src/reconciler.rs
    - src/state.rs

key-decisions:
  - "shift_note with 0 semitones passes through unchanged to preserve original enharmonic spelling"
  - "PanPrimitive::Fixed added for spread() rather than repurposing existing Center/Lfo/Noise variants"
  - "Pipe-expanded nodes injected into piece as synthetic ThingDefs so reconciler diffs them normally"
  - "Original pipe thing removed from piece after expansion -- only children exist for reconciliation"

patterns-established:
  - "Pipe expansion runs after ref resolution but before IR compilation in both startup and hot-reload"
  - "Synthetic thing naming: {thing_name}-pipe-{i} for pipe-expanded nodes"

requirements-completed: [PIPE-01, PIPE-02, PIPE-03, PIPE-04, PIPE-05, PIPE-06, PIPE-07, PIPE-08, PIPE-09]

duration: 8min
completed: 2026-03-22
---

# Phase 06 Plan 3: Pipe Executor Summary

**expand_pipe() expands pipe expressions into N named SynthBlocks with note shifting, pan spread, and tempo/take/repeat transforms, wired into reconciler for startup and hot-reload**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-22T04:57:29Z
- **Completed:** 2026-03-22T05:05:41Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments
- Created pipe executor with expand_pipe() supporting all 8 transform types (replicate, shift, spread, tempo, take, repeat, each, map)
- Implemented shift_note() helper for semitone transposition using note_to_midi + midi_to_note_name
- Wired pipe expansion into both startup IR compilation and hot-reload paths in main.rs
- Pipe-expanded synthetic ThingDefs injected into piece for reconciler visibility

## Task Commits

Each task was committed atomically:

1. **Task 1: pipe/executor.rs -- expand_pipe** - `bfe3953` (feat)
2. **Task 2: Wire expand_pipe into reconciler** - `11d9e81` (feat)

## Files Created/Modified
- `src/pipe/executor.rs` - Pipe executor: expand_pipe(), shift_note(), midi_to_note_name(), apply_each()
- `src/pipe/parser.rs` - Full parse_pipe_block implementation (was stub)
- `src/pipe/mod.rs` - Added executor module export
- `src/ir/types.rs` - Added PanPrimitive::Fixed variant for spread pan
- `src/ir/compiler.rs` - Added Fixed pan handling in UGen graph builder
- `src/parser/types.rs` - Added pipe: Option<String> field to ThingDef
- `src/main.rs` - Pipe expansion in startup + hot-reload, expand_pipe_things/inject_pipe_nodes helpers
- `src/ir/ref_resolver.rs` - Added pipe: None to test ThingDef constructor
- `src/reconciler.rs` - Added pipe: None to test ThingDef constructor
- `src/state.rs` - Added pipe: None to test ThingDef constructor

## Decisions Made
- shift_note with 0 semitones passes through unchanged to preserve original enharmonic spelling (Eb4 stays Eb4, not D#4)
- Added PanPrimitive::Fixed as new variant rather than repurposing existing variants -- cleaner separation of concerns
- Pipe-expanded nodes are injected into the piece as synthetic ThingDefs, replacing the original pipe thing, so the reconciler handles them with normal diff logic
- Nested pipe sources (pipe thing referencing another pipe thing) are warned and skipped

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Implemented parse_pipe_block (was stub from Plan 2)**
- **Found during:** Task 1 (executor depends on parser)
- **Issue:** parse_pipe_block was a stub returning error, needed for expand_pipe to work
- **Fix:** Full implementation of pipe block parser with source + transform chain parsing
- **Files modified:** src/pipe/parser.rs
- **Verification:** 8 parser tests pass
- **Committed in:** bfe3953

**2. [Rule 2 - Missing Critical] Added pipe: field to ThingDef**
- **Found during:** Task 1 (needed for detecting pipe things)
- **Issue:** ThingDef had no pipe field -- required for the pipe feature to work end-to-end
- **Fix:** Added pipe: Option<String> to ThingDef, updated all test constructors
- **Files modified:** src/parser/types.rs, src/ir/ref_resolver.rs, src/reconciler.rs, src/state.rs
- **Verification:** All 180 tests pass
- **Committed in:** bfe3953

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 missing critical)
**Impact on plan:** Both fixes were prerequisites for the plan's functionality. No scope creep.

## Issues Encountered
- shift_note test initially failed because Eb4 round-tripped through MIDI became D#4 (enharmonic). Fixed by passing through unchanged when semitones is 0.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Pipe language is fully functional: parse -> expand -> compile -> reconcile
- Glass-swarm style replicate+shift+spread patterns work end-to-end
- Hot-reload correctly re-expands pipe things on .hum file changes

---
*Phase: 06-ref-pipe*
*Completed: 2026-03-22*

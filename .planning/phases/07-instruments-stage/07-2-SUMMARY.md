---
phase: 07-instruments-stage
plan: 2
subsystem: stage-effects
tags: [stage, groups, fx, routing, scsynth, osc]

requires:
  - phase: 07-instruments-stage
    plan: 1
    provides: ThingDef with thing_type/applies_to fields, InstrumentStore
  - phase: 05-synth-ir
    provides: SynthBlock type, IR compiler, SCgf encoder
provides:
  - StageStore: maps stage names to StageConfig (group_id, applies_to, fx)
  - ScsynthClient group methods: create_group, start_synth_in_group, start_effect_at_tail
  - Stage-aware reconciler routing (things in stage -> group, others -> default)
  - compile_stage_effect helper for stage fx SynthDef compilation
affects: [any future stage features, fx chain enhancements, live stage reconfiguration]

tech-stack:
  added: []
  patterns: [scsynth group routing via /g_new + /s_new with addAction targeting, stage-aware reconciler]

key-files:
  created: [src/stage.rs]
  modified: [src/parser/types.rs, src/osc/bridge.rs, src/main.rs, src/state.rs, src/reconciler.rs]

key-decisions:
  - "StageStore as standalone HashMap wrapper -- simple, no Arc needed (single event loop)"
  - "Stage effect uses compile_synth_block with minimal SynthBlock (sine osc + fx) as carrier"
  - "Stage things filtered from active_things in state.rs -- structural, not playable synths"
  - "Hot-reload warns on stage changes instead of live reconfiguration (acceptable for v2)"
  - "effect_node_id is Option<i32> since stages without fx get group only"

patterns-established:
  - "Stage routing: group_for_thing lookup determines /s_new target group"
  - "Effect at tail pattern: /s_new with addAction=1 (addToTail) for group fx processing"

requirements-completed: [STAGE-01, STAGE-02, STAGE-03]

duration: 5min
completed: 2026-03-22
---

# Phase 7 Plan 2: Stage Effects Summary

**StageStore with scsynth group routing, effect SynthDef compilation at group tail, and stage-aware reconciler for shared fx chains**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-22T04:40:11Z
- **Completed:** 2026-03-22T04:45:27Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- StageStore maps stage names to StageConfig with group_id, applies_to, fx, effect_node_id
- ScsynthClient gains create_group (/g_new), start_synth_in_group (addToHead), start_effect_at_tail (addToTail)
- ThingDef extended with top-level fx: Option<FxPrimitive> for stage things
- Startup detects type: stage things, creates scsynth groups, compiles + loads effect SynthDefs
- apply_ops routes staged things into their group instead of default group 1
- active_things filters out stage things so they are not reconciled as playable synths
- Hot-reload warns that stage reconfiguration requires restart (v2 acceptable)
- 5 new unit tests for StageStore routing and effect compilation, all 141 tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: StageStore + ScsynthClient group methods** - `33d1233` (feat)
2. **Task 2: Stage detection + main.rs wiring** - `f95cd99` (feat)

## Files Created/Modified
- `src/stage.rs` - StageStore, StageConfig, compile_stage_effect, 5 unit tests
- `src/parser/types.rs` - Added fx: Option<FxPrimitive> to ThingDef, imported FxPrimitive
- `src/osc/bridge.rs` - create_group, start_synth_in_group, start_effect_at_tail methods
- `src/main.rs` - Stage startup lifecycle, stage-aware apply_ops, hot-reload warning
- `src/state.rs` - Filter stage things from active_things
- `src/reconciler.rs` - Updated test helper with fx field

## Decisions Made
- StageStore as standalone HashMap wrapper -- single event loop, no Arc needed
- Stage effect compiled via standard compile_synth_block with minimal SynthBlock carrier (sine + fx)
- Stage things are structural (not playable) -- filtered from active_things to prevent reconciler from trying to spawn them as synth nodes
- Hot-reload logs warning for stage changes rather than attempting live reconfiguration
- effect_node_id is Option<i32> to support stages that only create groups without fx

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed missing fx field in test helpers**
- **Found during:** Task 1 (cargo build after adding fx to ThingDef)
- **Issue:** reconciler.rs and state.rs test helpers construct ThingDef manually, missing new fx field
- **Fix:** Added `fx: None` to both make_thing() helpers
- **Files modified:** src/reconciler.rs, src/state.rs
- **Committed in:** 33d1233 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary fix for test compilation. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Stage routing fully wired: composers can add `type: stage` things with `applies-to` and `fx` fields
- Things in a stage's applies-to list automatically route through the stage's scsynth group
- Effect node processes group output at the tail
- Non-staged things continue unaffected in default group 1

---
*Phase: 07-instruments-stage*
*Completed: 2026-03-22*

---
phase: 07-instruments-stage
plan: 1
subsystem: synth-ir
tags: [instruments, synthblock, merge, serde, yaml]

requires:
  - phase: 05-synth-ir
    provides: SynthBlock type and IR compiler
provides:
  - InstrumentStore: load instruments/ dir, parse type: instrument .hum files
  - SynthBlock merge: base + override field-level precedence
  - ThingDef instrument/type/applies-to fields
  - Startup + hot-reload instrument merge wiring
affects: [07-instruments-stage plan 2 (stage effects), any future instrument features]

tech-stack:
  added: []
  patterns: [instrument file format with type: instrument + synth block, field-level SynthBlock merge]

key-files:
  created: [src/instruments.rs]
  modified: [src/parser/types.rs, src/parser/mod.rs, src/main.rs, src/state.rs, src/reconciler.rs]

key-decisions:
  - "InstrumentFile as separate struct (not ThingDef map) for standalone instrument .hum parsing"
  - "SynthBlock merge uses or_else chaining -- override.field.or(base.field) per field"
  - "resolve_synth_block helper centralizes merge logic for both startup and hot-reload paths"
  - "Missing instruments/ dir is non-fatal warning, returns empty store"

patterns-established:
  - "Instrument file format: type: instrument + synth: block in instruments/*.hum"
  - "Field-level merge pattern: InstrumentStore::merge(base, over) -> SynthBlock"

requirements-completed: [INST-01, INST-02, INST-03]

duration: 4min
completed: 2026-03-22
---

# Phase 7 Plan 1: Instruments + Stage Effects - InstrumentStore Summary

**InstrumentStore loads reusable instrument .hum files from instruments/ dir, merges SynthBlock fields into things at startup and hot-reload**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-22T04:33:31Z
- **Completed:** 2026-03-22T04:38:03Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- InstrumentStore scans instruments/ for type: instrument .hum files, stores name -> SynthBlock
- SynthBlock::merge gives override fields precedence, base fills gaps (field-level)
- ThingDef extended with thing_type (Instrument/Stage), instrument, applies_to fields
- Startup and hot-reload paths resolve instrument merges before IR compilation
- 7 new unit tests for merge behavior and directory loading, all 136 tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: InstrumentStore module + SynthBlock merge** - `664425a` (feat)
2. **Task 2: Extend ThingDef + main.rs instrument wiring** - `2952c01` (feat)

## Files Created/Modified
- `src/instruments.rs` - InstrumentStore: load_dir, get, merge + InstrumentFile parsing
- `src/parser/types.rs` - ThingType enum, instrument/thing_type/applies_to fields on ThingDef
- `src/parser/mod.rs` - Re-export ThingType
- `src/main.rs` - InstrumentStore loading at startup, resolve_synth_block helper, hot-reload merge
- `src/state.rs` - Updated test helper with new ThingDef fields
- `src/reconciler.rs` - Updated test helper with new ThingDef fields

## Decisions Made
- InstrumentFile as separate struct (not reusing ThingDef/Piece map) since instrument files are standalone with type + synth, not the piece map format
- SynthBlock merge via or_else chaining per field -- simple, correct, no macro magic
- resolve_synth_block as free function in main.rs -- shared by startup and hot-reload paths
- Missing instruments/ dir returns empty store with warning (non-fatal)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed missing ThingDef fields in test helpers**
- **Found during:** Task 2 (cargo test after adding fields)
- **Issue:** state.rs and reconciler.rs test helpers construct ThingDef manually, missing new thing_type/instrument/applies_to fields
- **Fix:** Added the three new fields (all None) to both make_thing() helpers
- **Files modified:** src/state.rs, src/reconciler.rs
- **Verification:** cargo test passes all 136 tests
- **Committed in:** 2952c01 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary fix for test compilation. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- InstrumentStore and ThingDef fields ready for Plan 2 (stage effects)
- ThingType::Stage and applies_to fields already in place for stage routing
- Composers can now create instruments/*.hum files and reference them with instrument: field

---
*Phase: 07-instruments-stage*
*Completed: 2026-03-22*

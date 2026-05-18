---
phase: 05-synth-ir
plan: 1
subsystem: ir
tags: [synth-ir, serde, midi, enums, fromstr]

# Dependency graph
requires:
  - phase: 02-parser-scd-reader
    provides: ThingDef struct, serde-saphyr YAML parsing
provides:
  - SynthBlock struct with 9 optional fields
  - Typed enums for all synth primitives (osc, filter, env, distort, fx, pan)
  - note_to_midi(), midi_to_freq(), parse_note_list() conversion functions
  - ThingDef.synth field wired into parser
affects: [05-synth-ir plan 2 (compiler), 05-synth-ir plan 3 (encoder)]

# Tech tracking
tech-stack:
  added: []
  patterns: [FromStr-based enum parsing, serde Deserialize via macro delegation, "name(key: val)" primitive call syntax]

key-files:
  created:
    - src/ir/mod.rs
    - src/ir/types.rs
    - src/ir/notes.rs
  modified:
    - src/parser/types.rs
    - src/main.rs
    - src/reconciler.rs
    - src/state.rs

key-decisions:
  - "Parse primitives via FromStr + serde macro delegation rather than custom Deserialize impls"
  - "Hand-written 'name(key: val)' parser using str::split — no regex dependency needed"
  - "All synth primitive fields are Option — minimal SynthBlock is just one field"

patterns-established:
  - "FromStr + impl_deserialize_from_str! macro for typed enum deserialization from YAML strings"
  - "parse_primitive_call() parser for 'name(key: val, key: val)' syntax reusable across all primitives"
  - "Range syntax 'lo~hi' via parse_range() for modulation parameters"

requirements-completed: [IR-01, IR-02, IR-03, IR-04, IR-05, IR-06, IR-07, IR-08]

# Metrics
duration: 3min
completed: 2026-03-21
---

# Phase 5 Plan 1: Synth IR Types Summary

**Typed SynthBlock with 6 primitive enums (osc/filter/env/distort/fx/pan), FromStr parsing of "name(key: val)" syntax, and note-to-MIDI conversion**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-21T07:05:47Z
- **Completed:** 2026-03-21T07:08:52Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- SynthBlock struct with all 9 optional fields and deny_unknown_fields
- 6 typed enums (OscPrimitive, FilterPrimitive, EnvPrimitive, DistortPrimitive, FxPrimitive, PanPrimitive) with FromStr and serde Deserialize
- note_to_midi/midi_to_freq/parse_note_list functions for MIDI conversion
- ThingDef.synth field wired in, all 90 tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: SynthBlock types + serde parsing** - `8b21293` (feat) — 41 new tests
2. **Task 2: Wire ThingDef.synth + declare ir mod** - `be9da63` (feat) — 0 regressions

## Files Created/Modified
- `src/ir/mod.rs` - Module re-exports for types and notes
- `src/ir/types.rs` - SynthBlock struct, all 6 primitive enums, FromStr impls, serde Deserialize via macro
- `src/ir/notes.rs` - note_to_midi(), midi_to_freq(), parse_note_list() with MIDI standard mapping
- `src/parser/types.rs` - Added synth: Option<SynthBlock> to ThingDef
- `src/main.rs` - Added mod ir declaration
- `src/reconciler.rs` - Updated make_thing() test helper with synth: None
- `src/state.rs` - Updated make_thing() test helper with synth: None

## Decisions Made
- Used FromStr + a macro `impl_deserialize_from_str!` to bridge serde Deserialize for all primitive enums — keeps each enum's parsing logic in one place
- Hand-wrote `parse_primitive_call()` for "name(key: val, key: val)" syntax using str::split — avoids regex dependency, ~20 lines
- All SynthBlock fields are Option — a valid block can be just `osc: sine` with everything else defaulted

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed missing synth field in state.rs make_thing() helper**
- **Found during:** Task 2 (Wire ThingDef.synth)
- **Issue:** Plan only mentioned reconciler.rs make_thing(), but state.rs has an identical test helper that also constructs ThingDef
- **Fix:** Added `synth: None` to state.rs make_thing() helper
- **Files modified:** src/state.rs
- **Verification:** cargo test passes all 90 tests
- **Committed in:** be9da63 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary for compilation. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- IR type system complete, ready for Plan 2 (IR compiler: types -> UGen graph)
- SynthBlock deserializes from YAML, all primitives parsed into typed enums
- note_to_midi provides frequency conversion needed by the compiler

---
*Phase: 05-synth-ir*
*Completed: 2026-03-21*

---
phase: 09-dictionary
plan: 1
subsystem: runtime
tags: [dictionary, yaml, hot-reload, synth-resolution, vocabulary]

# Dependency graph
requires:
  - phase: 04-synth-ir
    provides: SynthBlock IR types and compilation pipeline
  - phase: 05-instruments
    provides: InstrumentStore pattern (HashMap<String, SynthBlock>, merge, get)
provides:
  - DictStore module for loading hum.dict YAML vocabulary files
  - DictEntry type with synth + context + learned-from fields
  - style: field on ThingDef for dict term references
  - Dict-aware resolve_synth_block with priority chain
  - hum.dict hot-reload via file watcher
affects: [09-dictionary-plan-2, tui-dict-commands]

# Tech tracking
tech-stack:
  added: []
  patterns: [dict-store-pattern, style-resolution-priority-chain]

key-files:
  created: [src/dict.rs]
  modified: [src/main.rs, src/parser/types.rs, src/parser/mod.rs, src/pipe/executor.rs, src/state.rs, src/reconciler.rs, src/ir/ref_resolver.rs]

key-decisions:
  - "DictStore mirrors InstrumentStore pattern: HashMap<String, DictEntry> with load/get/merge"
  - "Resolution priority: .scd > instrument: > style: > bare synth: (style is lowest runtime priority)"
  - "Dict hot-reload checks filename (hum.dict or global.dict) before extension match in handle_file_change"
  - "DictEntry wraps SynthBlock + context + learned-from (not flattened) for clean separation"

patterns-established:
  - "Dict vocabulary pattern: YAML file with term -> {synth, context, learned-from} mapping"
  - "Style resolution: dict entry as base, thing's synth: overrides via InstrumentStore::merge"

requirements-completed: [DICT-01, DICT-02, DICT-03, DICT-04, DICT-07]

# Metrics
duration: 5min
completed: 2026-03-22
---

# Phase 9 Plan 1: Dictionary Summary

**DictStore with YAML vocabulary loading, style: field resolution in synth pipeline, and hum.dict hot-reload via file watcher**

## Performance

- **Duration:** 4m 31s
- **Started:** 2026-03-22T18:23:26Z
- **Completed:** 2026-03-22T18:27:57Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- DictStore module: load hum.dict YAML, merge global+project dicts (project wins on conflict), get by term
- ThingDef style: field + resolve_synth_block updated with dict-aware priority chain
- hum.dict added to file watcher with hot-reload support in handle_file_change
- 12 new tests (6 dict unit, 3 parser style, 3 resolve priority)

## Task Commits

Each task was committed atomically:

1. **Task 1: DictStore module** - `72a1c89` (feat)
2. **Task 2: style: field + resolution + watcher + startup wiring** - `985b621` (feat)

## Files Created/Modified
- `src/dict.rs` - DictStore: load YAML dict, merge global+project, get by term, DictEntry type
- `src/parser/types.rs` - Added style: Option<String> to ThingDef
- `src/parser/mod.rs` - 3 new tests for style: field parsing
- `src/main.rs` - Dict startup loading, resolve_synth_block with dict, watcher wiring, hot-reload, 3 resolve tests
- `src/pipe/executor.rs` - Added style: None to ThingDef constructor in tests
- `src/state.rs` - Added style: None to ThingDef constructor in tests
- `src/reconciler.rs` - Added style: None to ThingDef constructor in tests
- `src/ir/ref_resolver.rs` - Added style: None to ThingDef constructor in tests

## Decisions Made
- DictStore mirrors InstrumentStore pattern (HashMap-based, load/get/merge) for consistency
- Resolution priority: .scd > instrument: > style: > bare synth: -- style is the lowest runtime priority
- Dict hot-reload detects filename match (hum.dict / global.dict) before extension-based dispatch
- DictEntry wraps SynthBlock rather than flattening -- keeps context and learned-from metadata separate

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added style: None to 5 ThingDef constructors across codebase**
- **Found during:** Task 2 (style field addition)
- **Issue:** Adding style: field to ThingDef broke compilation in 5 files that construct ThingDef literals in tests
- **Fix:** Added `style: None` to all ThingDef struct literals in pipe/executor.rs, state.rs, reconciler.rs, main.rs, ir/ref_resolver.rs
- **Files modified:** src/pipe/executor.rs, src/state.rs, src/reconciler.rs, src/main.rs, src/ir/ref_resolver.rs
- **Verification:** cargo test passes (192 tests)
- **Committed in:** 985b621 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Standard struct field propagation. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- DictStore is ready for CLI introspection commands (hum dict list, hum dict show)
- Style resolution tested and wired into both startup and hot-reload paths
- Dict format supports context and learned-from metadata for future LLM integration

## Self-Check: PASSED

- FOUND: src/dict.rs
- FOUND: 72a1c89 (Task 1 commit)
- FOUND: 985b621 (Task 2 commit)

---
*Phase: 09-dictionary*
*Completed: 2026-03-22*

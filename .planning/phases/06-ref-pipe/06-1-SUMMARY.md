---
phase: 06-ref-pipe
plan: 1
subsystem: ir
tags: [ref-resolution, synth-merge, motif-reuse, regex]

# Dependency graph
requires:
  - phase: 05-synth-ir
    provides: SynthBlock type, InstrumentStore::merge, compile_synth_block
provides:
  - resolve_refs() function that resolves ref: and ref(thing).field in a Piece
  - Two-pass resolution: ThingDef-level inheritance + notes field accessor
affects: [06-ref-pipe plan 2 (pipe language), any future ref chaining]

# Tech tracking
tech-stack:
  added: [regex]
  patterns: [two-pass ref resolution, clone-before-mutate for IndexMap borrow safety]

key-files:
  created: [src/ir/ref_resolver.rs]
  modified: [src/ir/mod.rs, src/main.rs, Cargo.toml]

key-decisions:
  - "Reused InstrumentStore::merge for ref field inheritance — same base/override semantics"
  - "Two-pass design: Pass 1 for ThingDef-level ref:, Pass 2 for ref(thing).field in notes"
  - "No ref chaining in this phase — warns if ref target itself has a ref:"
  - "Only .notes field accessor supported; other accessors return helpful error"

patterns-established:
  - "Ref resolution runs after parse, before synth compilation — mutates Piece in-place"
  - "Errors logged and skipped (no daemon crash) — tracing::error on ref resolution failure"

requirements-completed: [REF-01, REF-02, REF-03, REF-04]

# Metrics
duration: 4min
completed: 2026-03-22
---

# Phase 6 Plan 1: Ref Resolution Summary

**Two-pass ref resolver: ThingDef-level ref: inheritance + ref(thing).notes accessor, reusing InstrumentStore::merge**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-22T04:53:48Z
- **Completed:** 2026-03-22T04:57:51Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- ref_resolver module with two-pass resolution: ThingDef-level ref: and ref(thing).field in synth notes
- 8 unit tests covering inheritance, local override, missing ref errors, notes accessor, unsupported field accessor
- Wired into both startup and hot-reload paths in main.rs, errors logged without crashing daemon

## Task Commits

Each task was committed atomically:

1. **Task 1: ref_resolver module with TDD tests** - `7d4d982` (feat)
2. **Task 2: Wire resolve_refs into reconciler/main** - `9cd945e` (feat)

## Files Created/Modified
- `src/ir/ref_resolver.rs` - Two-pass ref resolver with resolve_refs() entry point and 8 unit tests
- `src/ir/mod.rs` - Added pub mod ref_resolver
- `src/main.rs` - Wired resolve_refs at startup parse and hot-reload file change handler
- `Cargo.toml` - Added regex dependency for ref(thing).field pattern matching

## Decisions Made
- Reused InstrumentStore::merge for ref inheritance — same base/override semantics already proven
- Two-pass design separates concerns: Pass 1 handles ThingDef-level ref:, Pass 2 handles inline ref() in notes
- No ref chaining (ref of a ref) in this phase — logs warning if detected
- Only `.notes` field accessor supported; `.osc`, `.amp` etc return clear error message

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added regex dependency**
- **Found during:** Task 1
- **Issue:** regex crate not in Cargo.toml, needed for ref(thing).field pattern matching
- **Fix:** cargo add regex
- **Files modified:** Cargo.toml, Cargo.lock
- **Verification:** cargo build succeeds
- **Committed in:** 7d4d982

**2. [Rule 3 - Blocking] Fixed private module imports**
- **Found during:** Task 1 (test compilation)
- **Issue:** Tests imported crate::parser::types::ThingDef but types module is private; types are re-exported from crate::parser
- **Fix:** Changed imports to use crate::parser::ThingDef and crate::parser::Piece
- **Files modified:** src/ir/ref_resolver.rs
- **Verification:** All 8 tests pass
- **Committed in:** 7d4d982

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both necessary for compilation. No scope creep.

## Issues Encountered
None beyond the auto-fixed blocking issues above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- resolve_refs() is available for any future phase that needs ref resolution
- Pipe language (Plan 2) can build on this foundation
- Ref chaining (ref of a ref) deferred to future phase

---
*Phase: 06-ref-pipe*
*Completed: 2026-03-22*

---
phase: 02-parser-scd-reader
plan: 2
subsystem: scd
tags: [scsyndef, synthdef, filesystem, scsynth, osc]

# Dependency graph
requires:
  - phase: 01-osc-bridge
    provides: ScsynthClient::load_synthdef() for loading bytes into scsynth
provides:
  - ScdStore: filename-to-bytes map with load_dir() and get()
  - Startup SynthDef loading from out/sc/ via ScsynthClient
affects: [03-file-watcher, 04-timeline]

# Tech tracking
tech-stack:
  added: [tempfile (dev)]
  patterns: [filename-stem convention for thing name association, non-fatal missing directory pattern]

key-files:
  created: [src/scd/mod.rs, src/scd/store.rs]
  modified: [src/main.rs, Cargo.toml]

key-decisions:
  - "ScdStore::empty() constructor for fallback when out/sc/ is unreadable"
  - "Non-fatal error handling: missing dir returns Ok(empty), failed individual load warns and continues"
  - "Filter strictly on .scd extension only (not .scsyndef) per project convention"

patterns-established:
  - "Filename stem = thing name: space-crackle.scd maps to thing name 'space-crackle'"
  - "Missing resources are warnings, not fatal errors, to support iterative authoring workflow"

requirements-completed: [SCD-01, SCD-02, SCD-03]

# Metrics
duration: 3min
completed: 2026-03-20
---

# Phase 2 Plan 2: SCD Reader Summary

**ScdStore reads .scd files from out/sc/ by filename stem and loads each into scsynth via load_synthdef() on startup**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-20T16:48:07Z
- **Completed:** 2026-03-20T16:51:07Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- ScdStore with load_dir(), get(), thing_names(), iter(), len() -- full TDD with 6 tests
- Wired into main.rs startup: loads out/sc/*.scd into scsynth after health check
- Missing out/sc/ directory is graceful (0 SynthDefs, continues)
- Failed individual SynthDef loads warn and continue (non-fatal)

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement ScdStore (RED)** - `e06b2d2` (test)
2. **Task 1: Implement ScdStore (GREEN)** - `5bc6a26` (feat)
3. **Task 2: Wire ScdStore into startup** - `429d5df` (feat)

_TDD Task 1 has RED + GREEN commits. No refactor needed._

## Files Created/Modified
- `src/scd/mod.rs` - Module declaration, re-exports ScdStore
- `src/scd/store.rs` - ScdStore: HashMap<String, Vec<u8>> with load_dir, get, iter, thing_names, len, empty
- `src/main.rs` - Added mod scd, SCD loading after health check, per-thing load_synthdef calls
- `Cargo.toml` - Added tempfile dev-dependency for test fixtures

## Decisions Made
- Added ScdStore::empty() constructor for clean fallback when directory read fails (plan used a workaround with load_dir on a known-nonexistent path; empty() is cleaner)
- Kept .scd extension filter (not .scsyndef) per the plan's convention -- the project uses .scd for compiled SynthDef files in out/sc/

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- ScdStore is ready for file watcher integration (Phase 3) -- reload on .scd file changes
- Parser (Plan 1) + ScdStore (Plan 2) together provide the full read path: parse piece.hum for thing names, load .scd bytes by matching stems
- 17 tests pass across all modules (6 SCD + 11 parser/other)

## Self-Check: PASSED

All files exist, all commits verified:
- src/scd/mod.rs: FOUND
- src/scd/store.rs: FOUND
- 02-2-SUMMARY.md: FOUND
- e06b2d2 (RED): FOUND
- 5bc6a26 (GREEN): FOUND
- 429d5df (Task 2): FOUND

---
*Phase: 02-parser-scd-reader*
*Completed: 2026-03-20*

---
phase: 02-parser-scd-reader
plan: 1
subsystem: parser
tags: [serde, serde-saphyr, yaml, indexmap, deny_unknown_fields, untagged-enum]

# Dependency graph
requires:
  - phase: 01-osc-bridge
    provides: "Cargo.toml with serde/thiserror/anyhow, src/main.rs entry point"
provides:
  - "parse_hum(content) -> Result<Piece, HumParseError>"
  - "ThingDef struct with all 10 .hum fields and deny_unknown_fields"
  - "DoesField untagged enum (Single/Multi) with as_vec() helper"
  - "HumParseError with InvalidSchema and Io variants"
affects: [03-file-watcher, 04-timeline, 02-parser-scd-reader plan 2]

# Tech tracking
tech-stack:
  added: [serde-saphyr 0.0.22, indexmap 2.x]
  patterns: [deny_unknown_fields strict schema, untagged enum for polymorphic YAML fields, serde rename for Rust keyword fields]

key-files:
  created: [src/parser/mod.rs, src/parser/types.rs, src/parser/error.rs]
  modified: [Cargo.toml, src/main.rs]

key-decisions:
  - "serde-saphyr for YAML parsing (avoids deprecated serde_yaml and unsound serde_yml)"
  - "IndexMap preserves thing declaration order from .hum files"
  - "Rust keyword fields (where, ref) renamed to location/reference with #[serde(rename)]"
  - "piece.hum validation at startup is non-fatal (file may not exist yet)"

patterns-established:
  - "Strict schema: deny_unknown_fields on inner struct, dynamic keys via IndexMap at top level"
  - "Keyword rename: #[serde(rename = 'where')] pub location, #[serde(rename = 'ref')] pub reference"
  - "Polymorphic YAML: untagged enum for fields that accept string or list"

requirements-completed: [PARSE-01, PARSE-02, PARSE-03, PARSE-04]

# Metrics
duration: 2min
completed: 2026-03-20
---

# Phase 2 Plan 1: Parser + SCD Reader Summary

**Strict .hum YAML parser with serde-saphyr, deny_unknown_fields on ThingDef, DoesField untagged enum, and startup validation**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-20T16:47:53Z
- **Completed:** 2026-03-20T16:50:25Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- ThingDef struct with all 10 fields (at, until, does, location/where, has, within, every, like, reference/ref, mood) and deny_unknown_fields
- DoesField untagged enum supporting both single string and list-of-strings forms with as_vec() normalizer
- 9 tests covering valid parse, unknown field rejection, keyword renames, nested has, does variants, empty thing
- Startup piece.hum validation wired into main.rs (non-fatal)

## Task Commits

Each task was committed atomically:

1. **Task 1: Define parser types and error** - `ab586b9` (feat) -- TDD: types + tests in single commit since both were needed for RED/GREEN
2. **Task 2: Wire parse_hum into main.rs** - `c7be010` (feat)

## Files Created/Modified
- `src/parser/types.rs` - Piece type alias, ThingDef struct with deny_unknown_fields, DoesField untagged enum
- `src/parser/error.rs` - HumParseError enum with InvalidSchema and Io variants
- `src/parser/mod.rs` - parse_hum() function, re-exports, 9 unit tests
- `Cargo.toml` - Added serde-saphyr 0.0.22 and indexmap 2.x dependencies
- `src/main.rs` - Added mod parser, startup piece.hum validation

## Decisions Made
- Used serde-saphyr (not serde_yaml/serde_yml) per research findings -- avoids deprecated and unsound crates
- IndexMap for Piece type to preserve declaration order from .hum files (predictable iteration, test assertions)
- Explicit #[serde(rename)] for where->location and ref->reference rather than r# raw identifier syntax
- piece.hum parse at startup is non-fatal -- logs error but continues (authoring workflow allows missing piece.hum)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- parse_hum() is ready for file watcher integration (Phase 3)
- ThingDef provides typed access to all .hum fields for timeline/state reconciliation
- SCD reader (Plan 2 of this phase) can now associate .scd files with parsed thing names

## Self-Check: PASSED

- FOUND: src/parser/mod.rs
- FOUND: src/parser/types.rs
- FOUND: src/parser/error.rs
- FOUND commit: ab586b9 (Task 1)
- FOUND commit: c7be010 (Task 2)

---
*Phase: 02-parser-scd-reader*
*Completed: 2026-03-20*

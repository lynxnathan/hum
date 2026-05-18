---
phase: 05-synth-ir
plan: 2
subsystem: ir
tags: [synth-ir, scgf, encoder, compiler, ugen-graph, binary]

# Dependency graph
requires:
  - phase: 05-synth-ir plan 1
    provides: SynthBlock, OscPrimitive, FilterPrimitive, EnvPrimitive, DistortPrimitive, FxPrimitive, PanPrimitive
provides:
  - compile_synth_block(name, &SynthBlock) -> Result<Vec<u8>>
  - encode_synthdef(name, &UgenGraph) -> Vec<u8>
  - UgenGraph, UgenSpec, InputSpec types
affects: [05-synth-ir plan 3 (main.rs wiring + hot-swap)]

# Tech tracking
tech-stack:
  added: []
  patterns: [flat-vec UGen graph with index references, constant dedup via bit-equality, fixed signal chain compilation]

key-files:
  created:
    - src/ir/encoder.rs
    - src/ir/compiler.rs
  modified:
    - src/ir/mod.rs

key-decisions:
  - "Hand-rolled SCgf v2 encoder (~100 lines) instead of sorceress dependency"
  - "Constant dedup via f32 bit-equality to minimize constants array"
  - "Perc envelope uses constant gate=1.0 (fires immediately); ADSR uses Control gate param"
  - "Bitcrush via BinaryOpUGen(round, special_index=12) instead of separate Round UGen"
  - "Delay decaytime = time * feedback * 10 heuristic for musical feedback mapping"

patterns-established:
  - "GraphBuilder helper: add_constant (with dedup), add_param, add_ugen, param_index"
  - "Fixed signal chain order enforced by sequential UGen appending"
  - "contains_pstring / count_pstring test helpers for binary introspection"

requirements-completed: [IR-10]

# Metrics
duration: 4min
completed: 2026-03-21
---

# Phase 5 Plan 2: SCgf Encoder + IR Compiler Summary

**Pure-Rust SCgf v2 binary encoder and SynthBlock-to-UGen-graph compiler covering all 9 primitive types with 35 new tests**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-21T07:19:49Z
- **Completed:** 2026-03-21T07:23:34Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- SCgf v2 binary encoder: pstring encoding, big-endian integers/floats, UGen specs, input specs (constant vs UGen output), parameter names section
- IR compiler: SynthBlock -> UgenGraph -> Vec<u8> for all primitive combinations
- Fixed signal chain: Control -> osc -> filter -> env*signal -> distort -> fx -> amp*signal -> Pan2 -> Out
- All osc (sine/saw/pulse/white-noise/pink-noise), filter (lpf/hpf/bpf), env (perc/adsr), distort (tanh/bitcrush), fx (reverb/delay), pan (center/lfo/noise) primitives compile to valid SCgf
- 125 total tests pass (35 new: 11 encoder + 24 compiler)

## Task Commits

Each task was committed atomically:

1. **Task 1: SCgf v2 binary encoder** - `f46f6f7` (feat) — 11 new tests
2. **Task 2: SynthBlock to UGen graph compiler** - `3737144` (feat) — 24 new tests, 0 regressions

## Files Created/Modified
- `src/ir/encoder.rs` - UgenGraph/UgenSpec/InputSpec types + encode_synthdef() SCgf v2 serializer
- `src/ir/compiler.rs` - compile_synth_block() with GraphBuilder, all primitive handlers, 24 tests
- `src/ir/mod.rs` - Added compiler + encoder modules, re-exports compile_synth_block

## Decisions Made
- Hand-rolled SCgf encoder (~100 lines) gives full control without external dependency
- Constant dedup via f32::to_bits() equality avoids redundant entries in constants array
- Perc envelope fires immediately with constant gate=1.0; ADSR uses named "gate" param from Control for release control
- Bitcrush implemented via BinaryOpUGen(special_index=12 = round) rather than a separate Round UGen class
- Delay feedback mapped to decaytime via heuristic (time * feedback * 10) for musically useful ranges

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- compile_synth_block() ready for Plan 3 to wire into main.rs startup and hot-swap path
- Binary output is structurally valid SCgf v2, ready for scsynth /d_recv
- All primitive combinations produce non-empty Vec<u8> starting with b"SCgf"

---
*Phase: 05-synth-ir*
*Completed: 2026-03-21*

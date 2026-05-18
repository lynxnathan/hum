---
phase: 05-synth-ir
plan: 3
subsystem: ir
tags: [synth-ir, runtime-wiring, hot-swap, sequencer, event-loop]

# Dependency graph
requires:
  - phase: 05-synth-ir plan 1
    provides: SynthBlock, OscPrimitive, all primitive enums
  - phase: 05-synth-ir plan 2
    provides: compile_synth_block(name, &SynthBlock) -> Result<Vec<u8>>
provides:
  - Startup IR compilation with .scd escape hatch precedence
  - Hot-swap IR recompilation on .hum file change
  - NoteSequencer with tempo-based freq scheduling
  - SequencerEvent channel wired into main event loop
affects: [phase 5 human-verify checkpoint (next)]

# Tech tracking
tech-stack:
  added: []
  patterns: [tokio::spawn sequencer tasks with JoinHandle abort, mpsc channel for SequencerEvent, escape hatch via scd_store.get() or path exists check]

key-files:
  created:
    - src/ir/sequencer.rs
  modified:
    - src/ir/mod.rs
    - src/main.rs

key-decisions:
  - "Startup checks scd_store.get(name) first for escape hatch, then falls back to IR compilation"
  - "Hot-swap checks out/sc/<name>.scd path existence on disk (not store) since store is loaded once at startup"
  - "NoteSequencer uses tokio::spawn + interval with MissedTickBehavior::Skip for timing"
  - "SequencerEvent channel (capacity 256) feeds into main select! loop alongside DaemonEvent"
  - "Sequencer lifecycle: spawn on Add, abort on Remove/Stop/hot-swap"

patterns-established:
  - "Two-tier escape hatch: ScdStore at startup, path.exists() on hot-swap"
  - "Sequencer as spawned tokio task returning JoinHandle for abort control"
  - "HashMap<String, JoinHandle> for tracking active sequencer handles by thing name"

requirements-completed: [IR-08, IR-10, IR-11]

# Metrics
duration: 3min
completed: 2026-03-21
---

# Phase 5 Plan 3: Runtime Wiring Summary

**Startup IR compilation with escape hatch, hot-swap on edit, and note sequencer with tempo scheduling wired into event loop**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-21T07:30:26Z
- **Completed:** 2026-03-21T07:33:49Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments
- Startup path compiles synth: blocks to SCgf binary and loads via /d_recv, with .scd escape hatch checked first
- .hum file change triggers IR recompilation + node hot-swap for running things
- NoteSequencer spawns tokio tasks that cycle through note lists at tempo intervals
- SequencerEvent::SetFreq flows through channel into event loop, calls client.set_param("freq")
- Sequencer lifecycle fully managed: spawn on Add, abort on Remove/Stop/hot-swap, re-spawn on edit
- 129 total tests pass (4 new sequencer tests, 0 regressions)

## Task Commits

Each task was committed atomically:

1. **Task 1: Startup IR compilation + escape hatch precedence** - `4a4309d` (feat) — 0 regressions
2. **Task 2: Hot-swap IR on .hum file change** - `a36b648` (feat) — 0 regressions
3. **Task 3: Note sequencer + event loop wiring** - `d617404` (feat) — 4 new tests

## Files Created/Modified
- `src/ir/sequencer.rs` - NoteSequencer struct, parse_tempo(), spawn() with tokio interval, SequencerEvent enum
- `src/ir/mod.rs` - Added sequencer module, re-exports NoteSequencer and SequencerEvent
- `src/main.rs` - Startup IR compilation, hot-swap IR recompilation, sequencer channel + handles map, sequencer spawn/abort in apply_ops, sequencer event handling in select! loop

## Decisions Made
- Startup uses scd_store.get() for escape hatch (already loaded in memory); hot-swap uses path.exists() on disk since store is not reloaded
- NoteSequencer is a standalone tokio task communicating via mpsc channel, not integrated into the reconciler
- Sequencer channel capacity of 256 provides headroom for fast tempos across multiple things
- parse_tempo handles "0.35s/note", "0.5s", and bare number formats

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - scsynth must be running for runtime verification (checkpoint task).

## Next Phase Readiness
- All three auto tasks complete; checkpoint:human-verify is next
- Full pipeline wired: synth: block -> IR parse -> SCgf binary -> /d_recv -> /s_new -> audible sound
- Hot-swap on edit triggers recompilation and node swap within event loop cycle
- Note sequencer drives freq changes at tempo intervals for things with notes+tempo

---
*Phase: 05-synth-ir*
*Completed: 2026-03-21*

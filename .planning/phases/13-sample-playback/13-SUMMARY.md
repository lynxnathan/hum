---
phase: "13"
plan: "1-2"
subsystem: ir, osc, buffer
tags: [sample-playback, playbuf, buffer-management, scsynth]
dependency_graph:
  requires: [ir-compiler, osc-bridge, file-watcher]
  provides: [sample-playback, buffer-management, playbuf-synthdef]
  affects: [reconciler, main-event-loop]
tech_stack:
  added: [PlayBuf, BufRateScale, b_allocRead, b_free]
  patterns: [buffer-id-tracking, sample-hot-reload]
key_files:
  created:
    - src/ir/buffer.rs
  modified:
    - src/ir/types.rs
    - src/ir/compiler.rs
    - src/ir/mod.rs
    - src/osc/bridge.rs
    - src/main.rs
    - src/parser/types.rs
    - src/instruments.rs
    - src/stage.rs
    - src/assistant.rs
    - src/dict.rs
    - src/pipe/executor.rs
    - src/ir/ref_resolver.rs
decisions:
  - "Buffer IDs start at 100 (0 reserved for FFT in scsynth)"
  - "PlayBuf compiled as stereo (2 channels), mixed to mono for filter/fx, re-panned via Pan2"
  - "One-shot uses doneAction: 2 (free node), loop uses doneAction: 0 (keep running)"
  - "Control rate kr for bufnum param (needed by BufRateScale.kr)"
  - "sample: field takes priority over osc: when both present"
metrics:
  duration: "15min"
  completed: "2026-03-22"
  tasks: 3
  tests_added: 20
  tests_total: 238
requirements: [SAMP-01, SAMP-02, SAMP-03, SAMP-04]
---

# Phase 13: Sample Playback Summary

PlayBuf-based sample playback with buffer management, relative path resolution, and hot-reload on file change.

## What Was Built

### Task 1: SynthBlock Fields + OSC Buffer Methods
- Added `sample: Option<String>` and `loop: Option<bool>` fields to SynthBlock
- Added `load_buffer` (/b_allocRead), `free_buffer` (/b_free), `new_synth_with_args` methods to ScsynthClient
- Updated all SynthBlock struct literals across 10 files

### Task 2: PlayBuf IR Compiler
- `compile_sample_block()` generates PlayBuf.ar UGen graph
- Signal chain: Control(kr) -> BufRateScale.kr -> PlayBuf.ar(2ch) -> mono mix -> [filter] -> [fx] -> amp -> Pan2 -> Out
- `bufnum` control parameter for runtime buffer ID injection via /s_new args
- One-shot mode: doneAction 2 (free node when sample ends)
- Loop mode: doneAction 0 (keep running forever)

### Task 3: Buffer Manager + Main Loop Wiring
- `BufferManager` struct: HashMap<String, i32> for path-to-bufferID tracking, IDs from 100
- Startup: scans piece for `sample:` fields, allocates buffer IDs, loads via /b_allocRead
- Reconciler: passes `bufnum` arg to /s_new for sample-based synths
- Watches `samples/` directory for .wav/.aif changes, reloads buffer on change
- Hot-reload on piece.hum edit: re-allocs buffer + re-swaps node with bufnum arg

## Deviations from Plan

None - plan executed exactly as written. Combined both planned PLAN files (PLAN-1 and PLAN-2) into a single execution pass since they were small and tightly coupled.

## Decisions Made

1. **Buffer IDs from 100**: scsynth reserves buffer 0 for FFT; starting at 100 gives safe headroom
2. **Stereo PlayBuf**: Compile PlayBuf with 2 output channels, mix to mono for filter/fx chain, then Pan2 for stereo output
3. **kr Control for bufnum**: BufRateScale.kr needs kr-rate bufnum input, so all sample synth controls are kr (not ir)
4. **sample: overrides osc:**: When both fields present, sample path takes priority (PlayBuf used, osc ignored)
5. **doneAction strategy**: One-shot = 2 (free node on completion), loop = 0 (run forever)

## Test Coverage

- 10 new BufferManager unit tests (alloc, free, resolve, path matching)
- 10 new PlayBuf compiler tests (SCgf header, UGen presence, param presence, filter/fx combos, sample-over-osc priority)
- All 238 tests pass

## Self-Check: PASSED

- src/ir/buffer.rs: FOUND
- 13-SUMMARY.md: FOUND
- Commit b22e1de (Task 1 - SynthBlock fields + OSC): FOUND
- Commit 1329384 (Task 2 - PlayBuf compiler): FOUND
- Commit 1d7eb1d (Task 3 - Buffer manager + wiring): FOUND
- All 238 tests pass

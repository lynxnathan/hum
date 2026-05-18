---
phase: "12"
plan: "1"
subsystem: assistant
tags: [suggest, analyze, frequency-balance, structural-hints]
dependency_graph:
  requires: [dict, parser, ir-types]
  provides: [hum-suggest, hum-analyze]
  affects: [main-cli]
tech_stack:
  added: []
  patterns: [client-side-cli, frequency-band-estimation, synth-profile-matching]
key_files:
  created:
    - src/assistant.rs
  modified:
    - src/main.rs
decisions:
  - Client-side only (no daemon needed) -- consistent with dict commands
  - Frequency bands estimated from osc type + filter cutoff + note names
  - Dict matching uses osc/fx/filter type equality (not parameter values)
  - Text inference from like: field as fallback when no synth: block present
metrics:
  duration: "3min"
  completed: "2026-03-22"
---

# Phase 12 Plan 1: Creative Assistant Summary

Client-side suggest and analyze commands using static analysis of piece.hum synth params + dict vocabulary matching + frequency band estimation from osc/filter/note data.

## What Was Built

### src/assistant.rs
- `suggest(piece, dict) -> Vec<String>` -- structural hints:
  - Shared fx detection (2+ things with same reverb/delay -> stage effect hint)
  - Timing gap detection (5s+ gaps where nothing plays)
  - Oscillator monotony (all things using same osc type)
  - Dict vocabulary matching (thing synth profile matches dict entry -> suggest style:)
- `analyze(piece) -> Vec<String>` -- frequency balance:
  - Maps osc types to frequency bands (sine=narrow, saw=wide harmonics, noise=full spectrum)
  - Applies filter cutoffs as band limiters (LPF caps top, HPF caps bottom, BPF narrows)
  - Note name -> frequency conversion for fundamental estimation
  - Text inference from like: field when no synth: block exists
  - Reports heavy bands (3+ things), empty bands, and recommendations

### CLI Wiring (src/main.rs)
- `hum suggest` -- reads piece.hum + hum.dict, prints structural hints
- `hum analyze` -- reads piece.hum, prints frequency balance assessment
- Both are client-side (no daemon socket needed)

## Tests

9 tests covering:
- suggest: shared reverb detection, osc monotony, dict matching, timing gaps
- analyze: heavy sub content detection, missing band detection
- helpers: note_to_freq accuracy, parse_time_opt formats

## Deviations from Plan

None -- plan executed exactly as written.

## Commits

| Hash | Description |
|------|-------------|
| 8f2c1c4 | feat(12): add creative assistant -- hum suggest and hum analyze |

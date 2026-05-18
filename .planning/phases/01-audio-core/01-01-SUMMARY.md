---
phase: 01-audio-core
plan: 01
status: complete
started: "2026-03-27T23:20:00.000Z"
completed: "2026-03-27T23:30:00.000Z"
---

# Plan 01-01 Summary: Cargo deps + stub modules + audio.rs

## What Was Built

- **Cargo.toml**: Added `cpal = "0.17"`, `fundsp = { version = "0.23", default-features = false, features = ["std"] }`, `crossbeam-channel = "0.5"`
- **audio.rs**: Full audio pipeline — AudioParams (fundsp::Shared pan handles), build_graph (two sine oscillators at 440/660 Hz), build_stream (allocation-free cpal callback), init_audio_async (threaded device discovery)
- **nodes.rs**: Stub with NodeState struct (x, y, freq)
- **spatial.rs**: Stub with pan_from_x() returning 0.0 (center pan for Phase 1)
- **main.rs**: Added mod declarations for audio, nodes, spatial

## Deviations

- Plan specified `default-features = false` without `features = ["std"]` — fundsp goes no_std without it. Auto-fixed by adding `std` feature.
- Plan referenced `fundsp::hacker32` — actual module in 0.23 is `fundsp::prelude32`. Auto-fixed.
- `sample_rate` function on `SupportedStreamConfig` returns `cpal::SampleRate` struct, not `u32`. Used `.0` accessor.

## Verification

- `cargo check --bin ghostinstrument --no-default-features` passes
- `cargo test --bin ghostinstrument --no-default-features` — 4/4 tests pass
- No hardcoded sample rate in audio.rs (grep verified)
- No Vec/String/Box/Arc in cpal callback closure

## Key Files

| File | Purpose |
|------|---------|
| `Cargo.toml` | 3 new deps: cpal, fundsp, crossbeam-channel |
| `src/bin/ghostinstrument/audio.rs` | Full audio pipeline |
| `src/bin/ghostinstrument/nodes.rs` | NodeState stub |
| `src/bin/ghostinstrument/spatial.rs` | pan_from_x stub |

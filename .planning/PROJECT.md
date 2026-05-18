# ghostinstrument — Instruments That Don't Exist Yet

## What This Is

ghostinstrument is a spatial audio canvas. Every sound source is a node — a synth, a neural engine, an audio file, a live mic. Nodes have positions. Proximity between nodes creates transforms: blending, modulation, convolution, filtering. Moving nodes changes the sound. The canvas IS the mixer, IS the effects chain, IS the spatial audio field.

Input devices (MIDI, gamepad, webcam, keyboard, mouse, phone sensors) bind to node parameters or node positions. No timeline (v1). The performance is live, spatial, gestural.

Born from HUM (v1-v4), which proved Makepad + Rust + real-time audio works. ghostinstrument takes the spatial direction — from text files to physical space.

## Core Value

Launch. Drag nodes. Hear sound change. Zero config, zero knowledge required. Input-to-sound latency ceiling: 20ms.

## Previous Milestones (HUM era)

- **v1.0**: OSC bridge, parser, file watcher, reconciler, transport
- **v2.0**: Synth IR, pipe language, ref resolution, instruments, stage
- **v3.0**: Dictionary sync, translation, Makepad GUI, creative assistant
- **v4.0**: Sound IDE — MilkDrop visualizer, shader editor, terminal, sample playback, project browser

## Current Milestone: v5.0 — Two Nodes Make Sound

**Goal:** Two fundsp oscillator nodes on a dark spatial canvas, draggable with mouse, stereo panning + proximity blending, running on Windows via cross-compilation from WSL2.

**Target features:**
- Dark Makepad canvas (cross-compiled to Windows via cargo-xwin)
- Two fundsp oscillator nodes (different pitches) as draggable circles
- cpal audio output streaming to Windows speakers
- Stereo panning derived from node X position
- Proximity blending (distance between nodes → wet/dry crossfade)
- Mouse drag to move nodes

**Already proven:**
- Phase 01: Makepad bare window cross-compiles and launches on Windows (2026-03-27)

## Requirements

### Validated

<!-- HUM v1-v4 — shipped, separate binary (hum-gui) -->

- [x] Makepad GUI framework works on WSL2 and cross-compiles to Windows
- [x] cargo-xwin pipeline for x86_64-pc-windows-msvc from WSL2
- [x] Separate binary target (ghostinstrument) with gpui gated behind feature flag

### Active

<!-- v5.0 scope — defined in REQUIREMENTS.md -->

### Out of Scope

- scsynth integration — fundsp only for v5.0
- RAVE / DDSP neural nodes — future milestone
- MIDI, gamepad, phone controller — mouse only for v5.0
- Recording / replay / timeline — live only
- Node trait abstraction — two hardcoded nodes is correct
- HRTF / binaural — stereo panning sufficient
- Persistence, presets, config files — not yet
- VST hosting — never (Nodes Not Plugins stance)

## Context

- **Build:** WSL2 Ubuntu, cross-compile to Windows via cargo-xwin
- **Runtime:** Windows native (first platform), Linux/macOS/WASM future
- **Rust toolchain:** 1.92.0 via asdf, edition 2024
- **UI Framework:** Makepad 1.0.0
- **Audio I/O:** cpal (WASAPI on Windows)
- **DSP:** fundsp (oscillators, filters, effects)
- **Product spec:** ghostinstrument.cog (root)
- **Behavioral orientation:** .vestibular (root)

## Constraints

- **Language:** Rust, edition 2024
- **UI:** Makepad (GPU-accelerated canvas, custom shaders)
- **Audio:** cpal for I/O, fundsp for DSP — no external audio server
- **Latency:** 20ms ceiling for any input-to-sound path
- **Distribution:** Single binary, no installer
- **Build pipeline:** WSL2 → cargo-xwin → Windows MSVC

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Separate binary from hum-gui | Different product, shared deps | ✓ Proven |
| gpui behind feature flag | Avoids ring crate cross-compile issue | ✓ Proven |
| cargo-xwin over native Windows build | Build in WSL2 where Claude Code works | ✓ Proven |
| fundsp over scsynth for v5.0 | Zero external deps, embeddable, pure Rust | — Active |
| cpal for audio output | Standard Rust audio I/O, WASAPI on Windows | — Active |
| Edition 2024 | Latest Rust edition | ✓ Set |
| app_main! at module level | Makepad macro generates fn, not a call | ✓ Learned |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd:transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-03-27 after v5.0 milestone start*

# CLAUDE.md

Project guidance for AI assistants and contributors.

## Project

**ghostinstrument — Instruments That Don't Exist Yet**

ghostinstrument is a spatial audio canvas. Every sound source is a node — a synth, a neural engine, an audio file, a live mic. Nodes have positions. Proximity between nodes creates transforms: blending, modulation, convolution, filtering. Moving nodes changes the sound. The canvas IS the mixer, IS the effects chain, IS the spatial audio field.

Input devices (MIDI, gamepad, webcam, keyboard, mouse, phone sensors) bind to node parameters or node positions. No timeline (v1). The performance is live, spatial, gestural.

Born from HUM (v1–v4), which proved Makepad + Rust + real-time audio works. ghostinstrument takes the spatial direction — from text files to physical space.

**Core Value:** Launch. Drag nodes. Hear sound change. Zero config, zero knowledge required. Input-to-sound latency ceiling: 20ms.

### Constraints

- **Language:** Rust, edition 2024
- **UI:** Makepad (GPU-accelerated canvas, custom shaders)
- **Audio:** cpal for I/O, fundsp for DSP — no external audio server
- **Latency:** 20ms ceiling for any input-to-sound path
- **Distribution:** Single binary, no installer
- **Build pipeline:** WSL2 → cargo-xwin → Windows MSVC

## Technology Stack

### Core
| Technology | Version | Purpose | Notes |
|---|---|---|---|
| cpal | 0.17.3 | Audio I/O — output stream to WASAPI on Windows | 0.17 made `Stream: Send + Sync`. Requires Rust ≥ 1.82. |
| fundsp | 0.23.0 | DSP graph — oscillators, panning, mixing | Pure Rust, zero external deps. Use `default-features = false` to drop the `rustfft` convolution engine. Composable: `sine_hz(440.0) >> pan(-0.5)`. |
| makepad-widgets | 1.0.0 | UI framework — canvas, events | `DrawQuad` + `Sdf2d` for GPU drawing, `Hit::FingerDown/Move/Up` for drag. |

### Supporting
| Library | Version | Purpose |
|---|---|---|
| atomic_float | 0.1 | `AtomicF32` — share floats between Makepad UI thread and cpal audio callback. Prefer over `Mutex` (blocks) and over channels (overkill for a few floats). |
| crossbeam-channel | 0.5 | Lock-free MPSC for UI → audio events when parameter sets grow beyond a few floats. |

### Cross-compilation
- **cargo-xwin** — WSL2 → `x86_64-pc-windows-msvc`. Proven for Makepad. cpal 0.17 uses the `windows` crate for WASAPI; cargo-xwin provides MSVC SDK headers and libs automatically.

## Real-Time Audio Rules

- **Never lock in the audio callback.** `Mutex::lock()` can block on OS scheduling and cause dropouts. Use `AtomicF32` for floats, crossbeam-channel for structured events.
- **No allocation in the audio callback.** Pre-allocate buffers and channels at graph construction.
- **Canvas positions use `draw_abs`, not `draw_walk`** — turtle flow shifts on resize; canvas nodes need fixed absolute coordinates.

## What NOT to Use

| Avoid | Why | Use Instead |
|---|---|---|
| `Mutex` in cpal callback | Blocks audio thread, causes dropouts | `AtomicF32`, crossbeam-channel |
| fundsp with default features | Pulls in `rustfft` for unused convolution | `default-features = false` |
| cpal `asio` feature | Requires proprietary ASIO SDK; not cross-compilable from WSL2 | Default WASAPI |
| rodio | Abstracts away the output callback — no DSP injection point | cpal directly |

## Conventions

To be established as patterns emerge.

## Architecture

To be mapped as the codebase stabilizes. Follow existing patterns when adding code.

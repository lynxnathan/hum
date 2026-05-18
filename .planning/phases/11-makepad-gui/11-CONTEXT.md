# Phase 11: Makepad GUI - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Replace GPUI `hum watch` with Makepad-based `hum gui`. Spectral analyzer with FFT shader visualization, Ableton-style arrangement view with per-thing lanes, waveform display, GUI transport controls, VU meters, and thing selection for solo/mute. Does NOT cover: dictionary, translation sync, or creative assistant.

</domain>

<decisions>
## Implementation Decisions

### Confirmed by Spike Test

- Makepad 1.0.0 (makepad-widgets = "1.0") compiles and runs on WSL2
- System deps: libpulse-dev + existing X11 deps
- DRI3 warnings harmless — falls back to software GL
- App pattern: lib.rs with app_main!(App) + main.rs calling app_main()
- live_design! macro for declarative UI layout
- Makepad shader DSL for custom rendering (spectral viz)

### Claude's Discretion

- Layout: spectral analyzer on top, arrangement view center, transport bar bottom?
- FFT data source: scsynth /b_get on analysis bus, or compute in Rust from audio?
- Arrangement view: how to render at:/until: blocks as colored lanes
- Communication: daemon unix socket polling (like GPUI watch) or shared memory?
- Color scheme: Catppuccin Mocha (dark, matches existing) or Ableton-dark?

</decisions>

<specifics>
## Key Architecture

scsynth audio → FFT analysis → shader uniforms → Makepad fragment shader render
                                                    ↓
Daemon status polling → arrangement view + VU meters + transport bar

Two data flows:
1. Audio FFT for spectral viz (needs scsynth analysis bus or Rust-side FFT)
2. Transport status for arrangement/VU (reuse existing unix socket Status polling)

Spectral analyzer: Makepad shader DSL can render frequency bars/waterfall from uniform arrays.
Arrangement: colored rectangles positioned by at:/until: values, scrolling with playback.
VU meters: amplitude bars per thing, same data as GPUI version.

</specifics>

<code_context>
## Existing Code

- src/watch.rs — GPUI WatchView (REPLACE with Makepad version)
- src/transport.rs — TransportCmd::Status returns amplitudes + active things
- src/osc/bridge.rs — ScsynthClient (amplitude polling)
- /tmp/makepad-spike/ — working Makepad hello world
- Cargo.toml — already has gpui dep (will add makepad-widgets, keep gpui for backward compat or remove)

</code_context>

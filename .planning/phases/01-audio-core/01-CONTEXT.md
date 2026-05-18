# Phase 1: Audio Core - Context

**Gathered:** 2026-03-27
**Status:** Ready for planning

<domain>
## Phase Boundary

Establish the audio pipeline: cpal output stream with WASAPI backend, fundsp oscillator graph producing two distinct sine tones, allocation-free callback, sample rate negotiated from device. Audio initializes without blocking Makepad's UI thread. No canvas, no drag, no spatial — just sound through speakers.

</domain>

<decisions>
## Implementation Decisions

### Audio DSP
- Use fundsp `Shared` for UI→audio parameter passing (wraps AtomicF32, integrates with `var(&shared)` graph DSL)
- Inline equal-power panning math in cpal callback (fundsp `pan()` bakes at construction, not real-time controllable)
- Pre-build fundsp graph before cpal stream creation; graph ticked per-sample in callback
- Two oscillators: 440 Hz (A4) and 660 Hz (E5) — consonant fifth, clearly distinct

### Audio Initialization
- Create cpal stream in `audio::build_stream()` called from `app_main()` before `Cx::event_loop()`
- Query device default config for sample rate — do not hardcode 44100 or 48000
- `cpal::Stream` stored in App struct (stream is `!Send` on WASAPI, must stay on main thread)
- fundsp graph boxed as `Box<dyn AudioUnit + Send>` for cpal callback

### Thread Safety
- fundsp `Shared` handles cross the thread boundary (UI writes, audio reads)
- No Mutex in audio callback — ever
- One-pole smoothing on pan parameters from Phase 3 (not needed yet — pans are static in Phase 1)

### Claude's Discretion
- Module file structure (audio.rs, nodes.rs, spatial.rs) — follow architecture research
- Error handling strategy for audio device failures
- Buffer size configuration (use device default)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/bin/ghostinstrument/app.rs` — minimal Makepad app with dark canvas, proven cross-compile
- `src/bin/ghostinstrument/main.rs` — entry point calling `app::app_main()`

### Established Patterns
- `app_main!` macro at module level (not inside a function)
- `live_design!` for Makepad widget declaration
- `LiveRegister` trait for module registration

### Integration Points
- `app_main()` in app.rs — audio init goes before `Cx::event_loop()` (inside macro expansion)
- `App` struct — will hold `cpal::Stream` to keep it alive
- `handle_event()` — future phases wire drag events here

</code_context>

<specifics>
## Specific Ideas

- Research confirms cpal 0.17.3 with `default-features = false` for fundsp 0.23.0
- Architecture diagram shows 4-module structure: app.rs, nodes.rs, spatial.rs, audio.rs
- Pitfalls research: WASAPI COM init takes 200-500ms — confirm non-blocking

</specifics>

<deferred>
## Deferred Ideas

- Proximity blending (Phase 3)
- Stereo panning from node position (Phase 3)
- Canvas visualization (Phase 2)
- Dynamic node add/remove (v2)

</deferred>

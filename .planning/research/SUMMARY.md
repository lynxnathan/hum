# Project Research Summary

**Project:** ghostinstrument v5.0 — spatial audio canvas
**Domain:** Real-time spatial audio node canvas (Rust, Makepad, cpal, fundsp, WSL2 cross-compile to Windows)
**Researched:** 2026-03-27
**Confidence:** HIGH

## Executive Summary

ghostinstrument v5.0 is a spatial audio canvas where oscillator nodes are dragged on a GPU-rendered 2D canvas and their X positions control stereo panning in real-time. The domain is well-understood: every professional spatial audio tool (Ircam Spat, Envelop for Live, Dolby Atmos) uses the same visual language — colored circles on a dark canvas, draggable, with left/right mapping to stereo panning. The project's differentiator is node-to-node proximity blending: when two oscillator nodes are near each other, their audio crossfades using an equal-power curve. No existing spatial tool implements this interaction model.

The recommended architecture is a 4-module Rust structure (app.rs, nodes.rs, spatial.rs, audio.rs) with a strict single-direction data flow: Makepad UI thread writes atomic values, cpal WASAPI audio thread reads them. fundsp's `Shared` type wraps `AtomicF32` and provides the zero-lock parameter bridge. The fundsp graph is built once at startup with `var(&shared)` nodes — parameter updates are atomic stores, never graph rebuilds. cargo-xwin cross-compilation from WSL2 to Windows MSVC is proven from prior phases and requires no new configuration beyond cpal dependency addition.

The primary risks are all audio real-time correctness issues: WASAPI sample rate negotiation (must query device config before constructing the fundsp graph), zipper noise on pan updates (must apply one-pole smoothing in the audio callback), proximity blend clicks at distance threshold (must use soft curve + same smoothing), and allocation in the audio callback (zero heap allocations permitted in the cpal closure). All are well-documented and have clear mitigation patterns. None requires research-phase deep dives — they are prevention-through-code-review items.

## Key Findings

### Recommended Stack

The stack is fully determined from prior project experience and crates.io verification. cpal 0.17.3 provides WASAPI audio output on Windows with `Stream: Send + Sync` (fixed in 0.17 from the problematic 0.15 behavior). fundsp 0.23.0 provides the DSP graph — `sine_hz(440.0) >> panner(var(&pan_shared))` is a complete stereo panned oscillator. Makepad 1.0.0 is already validated for WSL2 cross-compile. No new framework dependencies are needed. The only additions to Cargo.toml are `cpal = "0.17"`, `fundsp = { version = "0.23", default-features = false }`, and `atomic_float = "0.1"`.

**Core technologies:**
- `cpal 0.17.3`: WASAPI audio output — latest stable with Send+Sync streams, no ASIO or feature flags needed
- `fundsp 0.23.0`: DSP oscillators + panning — pure Rust, zero system deps, cross-compiles trivially
- `makepad-widgets 1.0.0`: GPU canvas + drag events — already proven, `draw_abs` + `Sdf2d::circle` for free-positioned nodes
- `atomic_float 0.1`: `AtomicF32` bridge — thread-safe UI→audio parameter passing without mutex

### Expected Features

The v5.0 MVP is tightly scoped and all table-stakes features align with the project's two-node proof-of-concept goal. The single differentiator — proximity blending — is what distinguishes this from any standard panner tool and must ship in v5.0.

**Must have (table stakes):**
- Two oscillator nodes rendered as colored circles on dark canvas — universal spatial audio UI convention
- Mouse drag to reposition nodes — primary interaction; no spatial tool omits this
- Stereo panning from X position using equal-power law — left/right expectation is immediate
- Audio that changes in real-time as you drag — the tool is meaningless without this
- Visual distinction between nodes (color by index) — two nodes must be identifiable

**Should have (differentiators, v5.0):**
- Proximity blend between two nodes using equal-power crossfade — this is the core product differentiator; not a v5.1 item

**Defer (v5.1+):**
- Amplitude pulse ring — adds liveness feedback; meaningful after core interaction is proven
- Connection line between close nodes — visual indicator of blend; implement after audio blend confirmed
- Node tooltip on hover — label clarity without persistent text clutter

**Defer (v6+):**
- HRTF binaural, movable listener, canvas zoom/pan, more than two nodes, canvas persistence

### Architecture Approach

The architecture is a strict 4-module separation with one-directional atomic data flow across the UI/audio thread boundary. `app.rs` owns the Makepad event loop and the cpal stream (held alive for program lifetime). `nodes.rs` is a pure data struct with canvas coordinates. `spatial.rs` is a pure function converting canvas positions to audio-domain pan values — unit-testable in isolation. `audio.rs` imports only cpal and fundsp, never Makepad. The fundsp graph topology is fixed at startup; runtime parameter changes are atomic stores only.

**Major components:**
1. `app.rs` — Makepad App struct, handle_event dispatch, canvas draw_walk, owns Stream and AudioParams
2. `nodes.rs` — NodeState { x, y, freq, radius }, no framework knowledge
3. `spatial.rs` — pure function: NodeState + canvas_width → (pan_a, pan_b, blend_coeff)
4. `audio.rs` — cpal stream bootstrap, fundsp graph construction, build_params() returning Arc<AudioParams>

**Build order:** nodes.rs → audio.rs (struct only) → spatial.rs (unit tests) → audio.rs (full stream) → app.rs wiring → app.rs draw_walk. Audio is testable before any UI exists.

### Critical Pitfalls

1. **WASAPI sample rate negotiation** — cpal opens the stream at the device's negotiated rate (44100 or 48000 Hz), not at your requested rate. Always call `device.default_output_config()` first, extract the sample rate, and pass it to `graph.reset(Some(rate as f64))` before starting the stream. Hardcoding 44100 produces a 9% pitch shift on 48kHz devices.

2. **Zipper noise on pan updates** — UI events arrive at 60 Hz; the audio callback runs at 48000 Hz. A direct coefficient assignment produces 60 Hz stepping artifacts audible as scratching. Apply a one-pole lowpass smoothing filter inside the audio callback (`pan_current = pan_current * 0.995 + target * 0.005`) — same pattern applies to proximity blend coefficient.

3. **Allocation in the cpal callback** — Any heap allocation (Vec, String, format!, Box, Arc clone/drop) in the WASAPI callback risks blocking on the OS allocator. Pre-allocate all buffers before stream start. fundsp's process() is allocation-free by design; keep the surrounding callback code equally clean.

4. **fundsp graph Send bound** — `Box<dyn AudioUnit>` is not `Send` by default. The cpal closure requires `Send`. Use `Box<dyn AudioUnit + Send>` explicitly, or avoid boxing by keeping the graph as a concrete type.

5. **Blocking Makepad UI thread during audio init** — WASAPI COM initialization takes 200-500ms. Calling `cpal::default_host()` in `App::new()` or a Makepad event handler freezes the window. Spawn audio init on a dedicated thread at startup; use a channel to pass the initialized stream back.

## Implications for Roadmap

Based on research, the phase structure follows the build order dictated by architecture dependencies. Audio must be testable before UI is wired. Cross-compile must be validated before DSP code is written.

### Phase 1: Cross-Compile Validation + cpal Smoke Test
**Rationale:** cargo-xwin WASAPI link errors are the highest-friction blocker. Validate the toolchain before writing any DSP logic. A minimal cpal program (device enumeration + silent stream) confirms the entire cross-compile pipeline. Cost of fixing this early: hours. Cost of discovering it after fundsp integration: days.
**Delivers:** `cargo xwin build --target x86_64-pc-windows-msvc` succeeds with cpal added; silent WASAPI stream opens on Windows
**Addresses:** Pitfall 4 (cargo-xwin WASAPI headers), Pitfall 8 (exclusive mode — confirm shared mode works)
**Avoids:** Discovering link errors after DSP and UI are tangled together

### Phase 2: Audio Core (NodeState + AudioParams + Oscillators)
**Rationale:** Build nodes.rs, audio.rs, and spatial.rs in isolation before any Makepad wiring. Two sine tones audible via cpal with pan controllable from hardcoded values proves the DSP graph is correct. Unit tests on spatial::recompute() confirm the pan math without needing a running app.
**Delivers:** Two audible sine oscillators with stereo panning, no UI; spatial math unit-tested
**Uses:** cpal 0.17, fundsp 0.23, atomic_float 0.1
**Implements:** audio.rs + nodes.rs + spatial.rs components
**Avoids:** Pitfall 1 (query sample rate before graph construction), Pitfall 2 (pre-allocate callback), Pitfall 5 (Send bound on graph)

### Phase 3: Makepad Canvas + Drag Wiring
**Rationale:** Makepad layer is added on top of a proven audio core. Dark canvas with two colored circles, drag interaction writing to AtomicF32 values, and audio panning responding in real-time. This is the first phase where both UI and audio threads are active simultaneously — thread safety discipline is critical.
**Delivers:** Full v5.0 MVP: draggable nodes on dark canvas with live stereo panning
**Uses:** makepad-widgets 1.0.0, draw_abs + Sdf2d::circle, Hit::FingerDown/Move/Up
**Implements:** app.rs, connects all components
**Avoids:** Pitfall 3 (pan smoothing must be in place before first drag-to-audio wiring), Pitfall 6 (audio init on separate thread, not in App::new())

### Phase 4: Proximity Blend
**Rationale:** After panning is proven clean, add the node-to-node proximity blend. This is the product's differentiator but depends on all Phase 3 infrastructure. Introduced separately to isolate proximity-specific issues from panning issues.
**Delivers:** Proximity blend coefficient computed from inter-node distance, equal-power crossfade applied in audio callback
**Implements:** spatial.rs proximity math, audio.rs wet/dry crossfade
**Avoids:** Pitfall 7 (soft curve + smoothing, no binary threshold click)

### Phase Ordering Rationale

- Cross-compile validation first because it is a blocking dependency with high failure cost and zero dependencies of its own.
- Audio core before UI because audio correctness issues (sample rate, allocation, Send) are invisible in UI integration and hard to isolate after the fact.
- Panning before proximity blend because panning infrastructure (atomic bridge, smoothing) is prerequisite for proximity blend. Proving panning clean confirms the smoothing approach is correct before applying it to blend.
- Proximity blend last because it is the creative feature — building it on a solid foundation avoids debugging audio artifacts under unclear causality.

### Research Flags

Phases with standard patterns (skip research-phase for these):
- **Phase 1:** cargo-xwin is proven for this project; cpal WASAPI is well-documented. No research needed.
- **Phase 2:** cpal + fundsp integration is thoroughly documented in official sources with code-level API patterns in STACK.md. No research needed.
- **Phase 3:** Makepad APIs (draw_abs, Hit events) are verified from local registry source. No research needed.
- **Phase 4:** Proximity blend math is standard equal-power crossfade — same formula as panning. No research needed.

No phases require `/gsd:research-phase`. All patterns are resolved to code level in the research files.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | cpal 0.17.3 and fundsp 0.23.0 verified from crates.io and official docs; Makepad APIs verified from local registry source; cargo-xwin proven from prior phases |
| Features | MEDIUM-HIGH | Core audio math (equal-power panning, crossfade) is HIGH. Competitive feature analysis is MEDIUM (spatial audio tools near training cutoff). MVP scope from primary project sources is HIGH. |
| Architecture | HIGH | cpal threading model from official docs; fundsp Shared pattern from official docs; Makepad main-thread constraint from Makepad source + robius integration docs 2026 |
| Pitfalls | HIGH | WASAPI behavior from official Microsoft docs + cpal issues; real-time audio constraints from canonical references; fundsp allocation contract from official docs |

**Overall confidence:** HIGH

### Gaps to Address

- **Proximity blend threshold value:** No prior art for the specific blend radius value (what canvas-pixel distance triggers blending). Start with 30% of canvas diagonal as initial heuristic; tune by ear during Phase 4.
- **Pan smoothing coefficient tuning:** 0.995 at 48kHz is the starting point (~10ms smoothing time). If dragging feels sluggish or still zippers, adjust during Phase 3 validation.
- **Audio init thread handoff pattern:** The exact OnceLock or channel pattern for handing the initialized cpal stream back to the Makepad App struct is not code-verified. May need a brief spike in Phase 3 to confirm the pattern compiles before full phase planning.

## Sources

### Primary (HIGH confidence)
- cpal 0.17.3 official docs (docs.rs/cpal/latest) — WASAPI backend, stream threading, sample rate negotiation
- fundsp 0.23.0 official docs + GitHub (docs.rs/fundsp, github.com/SamiPerttu/fundsp) — Shared params, graph notation, allocation-free contract
- makepad-draw-1.0.0 local registry source — draw_abs, Sdf2d::circle, DrawQuad APIs (verified at source level)
- makepad-widgets-1.0.0 local registry source — Hit::FingerDown/Move/Up, event.hits() patterns
- Robius roadmap blog (robius.rs/blog/robius-roadmap-2025) — Makepad main-thread-only Cx constraint
- Microsoft WASAPI docs (learn.microsoft.com/en-us/windows/win32/coreaudio/wasapi) — shared vs exclusive mode latency
- ghostinstrument.cog + PROJECT.md — primary project scope definition

### Secondary (MEDIUM confidence)
- robius-matrix-integration / Makepad skill docs (lobehub.com, 2026-02-17) — SignalToUI pattern
- cpal GitHub issues — WASAPI sample rate negotiation behavior confirmed
- cargo-xwin README — SDK download and MSVC sysroot configuration
- Reddit r/rust — cpal 0.17.0 release notes confirming Send+Sync fix
- bekk.christmas — cpal+fundsp integration pattern example (2024)

### Tertiary (LOW confidence)
- iZotope Strands (2024) feature set — near training cutoff; used only for competitive comparison, not technical decisions
- Spatial audio competitor feature sets (Ircam Spat, Envelop for Live, Dolby Atmos) — training data era; used for UX convention analysis only

---
*Research completed: 2026-03-27*
*Ready for roadmap: yes*

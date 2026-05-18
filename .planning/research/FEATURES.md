# Feature Research — Spatial Audio Canvas

**Domain:** Spatial audio canvas / 2D node-based sound positioning interface
**Researched:** 2026-03-27
**Confidence:** MEDIUM — Spatial audio canvas products (Ircam Spat, Envelop for Live, Dolby Atmos Renderer, iZotope Strands, Reaper's ReaVerb spatial, AudioThing Parallax) are training-data-era knowledge. Core audio math (panning laws, distance models) is stable and HIGH confidence. Interaction patterns inferred from analogous tools (node-based editors: Blender nodes, Max/MSP patcher, Pure Data) combined with spatial audio domain.

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features that must exist for "spatial audio canvas" to mean anything. Missing one = concept fails.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Nodes rendered as circles on dark canvas | Every spatial audio tool (Ircam Spat, Envelop for Live, Dolby Atmos Renderer) uses circles as the universal node metaphor. Users recognize it instantly. | LOW | Circle = position + identity. Size can encode amplitude or be fixed. Dark background = convention (contrast with white circle + glow). |
| Drag to move nodes | Spatial audio is meaningless without positional control. Mouse drag is the universal primary interaction. | MEDIUM | Requires: hit detection on circle radius, mouse-down capture, delta tracking, position clamp to canvas bounds. |
| Stereo panning from X position | Leftmost expectation users have when they see a 2D canvas: left/right corresponds to audio left/right. | LOW | Equal-power (constant-power) panning law: left = cos(θ), right = sin(θ) where θ = (x/width) * (π/2). Linear panning creates perceived loudness dip at center; equal-power avoids this. |
| Audio that changes as you drag | The whole point. If dragging nodes doesn't produce audible change in real-time, the canvas is a static diagram. | MEDIUM | Audio thread must read position atomically. Panning coefficient update must happen within one audio callback (no glitch). Use `AtomicU32` or similar for lock-free position passing. |
| Visual distinction between nodes | If two nodes look identical, user loses track of which is which. Every spatial tool colors nodes differently (by type, by pitch, by assignment). | LOW | Different fill colors per node index. No label needed for v5.0 — color is enough for two nodes. |
| Node stays where you drop it | Drag releases should commit position. Position must persist through audio interruptions. | LOW | Store position in node state, not in transient drag state. |

### Differentiators (Competitive Advantage)

Features that make this canvas distinct from generic spatial audio tools. Align with ghostinstrument's core value: gestural, immediate, no-config.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Proximity blending between nodes | Unique to ghostinstrument's node-interaction model. Standard spatial tools only pan relative to a listener — they don't create node-to-node interaction. When two nodes are close, their audio blends: wet/dry crossfade. | MEDIUM | Distance formula: `d = sqrt((x1-x2)² + (y1-y2)²)`. Normalize to canvas diagonal. Blend curve: use equal-power crossfade, not linear, to preserve perceived loudness during blend. |
| Amplitude pulse ring on active nodes | Visual feedback of audio state. Oscilloscope-style breathing ring that pulses with node amplitude. Users understand "this is making sound" immediately. | MEDIUM | Requires reading amplitude from audio thread (ring buffer of recent RMS). Draw expanding + fading circle around node in Makepad shader or draw call. Use `AtomicU32` for RMS value passed from audio thread. |
| Proximity influence field (subtle glow) | When two nodes are close enough to interact, a soft gradient between them signals the interaction zone. Makes the proximity blend model self-explaining. | MEDIUM | Makepad shader pass computing distance between node pair, drawing alpha-gradient ellipse in the interaction area. Only for node pairs within blend threshold. |
| Connection line between close nodes | Line drawn between two nodes when distance < blend threshold. Opacity and thickness encode blend amount. Makes invisible audio relationship visible. | LOW | Simple Makepad draw_line between node centers. Alpha = `1.0 - (distance / threshold)`. Only rendered when distance < threshold. |
| Equal-power crossfade for proximity blend | Most tools use linear crossfade (creates loudness dip at equal distance). Equal-power feels correct: moving nodes toward each other increases blend without perceived volume loss. | LOW | `wet = sin(blend_ratio * π/2)`, `dry = cos(blend_ratio * π/2)`. Trivial to implement once the ratio is computed. |

### Anti-Features (Commonly Requested, Often Problematic)

Features that would seem natural to add but violate the v5.0 scope or the product's core stance.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Node labels / names overlay | Users want to know which node is which without relying on color alone | Text rendering in Makepad requires font loading and layout; adds dependency and build complexity for v5.0 where two color-differentiated nodes are unambiguous | Defer to v5.1: tooltip on hover is simpler than persistent label |
| Amplitude-based node size | Natural mapping: louder = bigger circle feels intuitive | If size encodes amplitude, drag hit detection area changes constantly — the user chases a moving target | Fixed radius for hit detection; use pulse ring for amplitude feedback instead |
| Right-click context menu | Node-based editors (Max/MSP, Pure Data) all have context menus | Adds menu state machine, keyboard navigation, accessibility — out of scope for v5.0 | Double-click to cycle node type (future); context menu is v6+ |
| Zoom / pan of the canvas itself | Expected once canvas has more than ~6 nodes | Requires viewport transform applied to all hit detection, coordinates, and draw calls — significant complexity multiplier | Fixed canvas coordinate space for v5.0; canvas = window viewport |
| HRTF / binaural spatialization | Audiophiles expect headphone-correct 3D audio | Requires HRIR database (SOFA files, ~10MB), convolution per-channel, adds significant CPU; fundsp doesn't include HRTF natively | Stereo equal-power panning is perceptually correct for speakers; HRTF is a v7+ milestone |
| Multiple listener positions | Some spatial tools (Ircam Spat, Dolby Atmos) have a movable listener/receiver object in the space | Adds second draggable object with different interaction semantics — doubles complexity of the hit detection and audio routing | Listener is implicit center for v5.0; movable listener is v6+ |
| Undo / redo for position changes | Node editors always get "why can't I undo my drag?" | State history for positions requires a position log, undo stack, and careful interaction with the audio thread; high complexity for no audio value in v5.0 | Nodes are cheap to reposition; undo is a v5.1 quality-of-life item |
| Saving canvas state to disk | Users naturally want to save a "patch" | Requires serialization format, file picker, startup loading — a full separate feature | Out of scope for v5.0 per PROJECT.md; persistence is deliberate non-goal |

---

## Feature Dependencies

```
[Node position state (f32 x, f32 y)]
    └──required by──> [Drag interaction]
                          └──required by──> [Mouse hit detection]
    └──required by──> [Stereo panning computation]
    └──required by──> [Proximity distance computation]
                          └──required by──> [Proximity blend coefficient]
                                                └──required by──> [Wet/dry crossfade in audio mixer]
                          └──required by──> [Connection line rendering]
                          └──required by──> [Proximity glow field rendering]

[Lock-free position passing (audio thread ↔ UI thread)]
    └──required by──> [Audio changes on drag (real-time, no glitch)]
    └──required by──> [Amplitude ring rendering (audio → UI)]

[Node rendering (circle at position)]
    └──required by──> [Visual distinction between nodes]
    └──required by──> [Amplitude pulse ring]   (enhances)
    └──required by──> [Connection line]        (enhances)

[Amplitude RMS from audio thread]
    └──required by──> [Amplitude pulse ring]

[Equal-power panning law]
    └──required by──> [Stereo panning from X]

[Equal-power crossfade law]
    └──required by──> [Proximity blend coefficient]
```

### Dependency Notes

- **Lock-free position passing is load-bearing:** The UI thread writes positions; the audio thread reads them every callback at ~512-sample intervals. A mutex here causes priority inversion and audio glitches. Use `AtomicU32` (reinterpret f32 bits) or a triple-buffer. This is the riskiest coupling point.
- **Proximity blend requires both nodes' positions:** The blend coefficient is a function of inter-node distance, not node-to-listener distance. Both must be readable from the spatial mixer simultaneously.
- **Amplitude ring requires audio→UI data flow:** RMS value must travel from the real-time audio callback to the UI draw loop without blocking. An `AtomicU32` storing `f32::to_bits()` is the standard pattern.
- **Connection line enhances proximity glow:** They convey the same information (interaction between near nodes) at different visual resolutions. Connection line is simpler (draw_line). Glow field is richer (shader). Both are additive, not conflicting.
- **Equal-power panning and equal-power crossfade are the same math:** `(cos(t*π/2), sin(t*π/2))` is used for both. Same function, two call sites.

---

## MVP Definition

This maps directly to v5.0 scope as defined in PROJECT.md.

### Launch With (v5.0)

- [ ] Two fundsp oscillator nodes at fixed initial positions — concept requires at least two sound sources with different pitches to hear proximity blend
- [ ] Circles rendered on dark Makepad canvas with distinct colors — visual identity
- [ ] Mouse drag to move nodes (hit detection + delta tracking + position update) — primary interaction
- [ ] Stereo panning from node X position using equal-power law — left/right spatial mapping
- [ ] Proximity blend between two nodes using equal-power crossfade — node-to-node interaction (the core differentiator)
- [ ] Audio output via cpal to Windows speakers — proof of concept requires audible output

### Add After Validation (v5.1)

- [ ] Amplitude pulse ring — adds "liveness" to the canvas; meaningful once the core interaction is proven
- [ ] Connection line between close nodes — visual feedback of proximity blend; implement after blend audio is confirmed working
- [ ] Hover highlight on nodes — subtle outline change on hover before drag begins; improves affordance
- [ ] Node tooltip / label on hover — identify nodes without persistent text clutter

### Future Consideration (v6+)

- [ ] Proximity glow/influence field — requires Makepad shader work; impactful but non-trivial
- [ ] Movable listener position — second drag target with distinct semantics
- [ ] HRTF binaural — separate crate evaluation, SOFA file loading, significant scope
- [ ] More than two nodes — node trait abstraction, dynamic node list, hit detection over N nodes
- [ ] Zoom / pan canvas viewport — coordinate transform throughout entire render + interaction stack

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Two oscillator nodes rendering as circles | HIGH | LOW | P1 |
| Mouse drag (hit detection + position update) | HIGH | MEDIUM | P1 |
| Lock-free audio thread position read | HIGH | MEDIUM | P1 — correctness-critical |
| Equal-power stereo panning from X | HIGH | LOW | P1 |
| Proximity blend (distance → crossfade) | HIGH | LOW | P1 — once position plumbing exists |
| cpal audio output on Windows | HIGH | MEDIUM | P1 |
| Distinct node colors | MEDIUM | LOW | P1 — trivial, high clarity payoff |
| Amplitude pulse ring | MEDIUM | MEDIUM | P2 |
| Connection line between close nodes | MEDIUM | LOW | P2 |
| Hover highlight affordance | LOW | LOW | P2 |
| Node tooltip on hover | LOW | MEDIUM | P2 |
| Proximity glow field (shader) | MEDIUM | HIGH | P3 |
| Canvas zoom/pan | MEDIUM | HIGH | P3 |
| Movable listener | HIGH | MEDIUM | P3 |
| HRTF binaural | HIGH | HIGH | P3 |

**Priority key:**
- P1: v5.0 — needed to ship the milestone
- P2: v5.1 — add once core interaction is validated
- P3: v6+ — future milestone

---

## Competitor Feature Analysis

Reference tools: Ircam Spat (Max/MSP + standalone), Envelop for Live (Ableton plugin), Dolby Atmos Renderer (ProTools/Logic plugin), iZotope Strands (standalone app, 2024).

| Feature | Ircam Spat | Envelop for Live | Dolby Atmos Renderer | iZotope Strands | ghostinstrument v5.0 |
|---------|------------|-----------------|----------------------|-----------------|----------------------|
| Node visualization | Circles on dark canvas | Circles on dark canvas | Circles on dark canvas | Circles with color by type | Circles, color by index |
| Drag interaction | Click + drag | Click + drag | Click + drag | Click + drag | Click + drag |
| Panning law | Speaker-array-aware, VBAP | Equal-power stereo + HOA | Bed + object-based | Equal-power stereo | Equal-power stereo |
| Node-to-node interaction | None (listener-centric) | None (listener-centric) | None (listener-centric) | Limited | Proximity blend (differentiator) |
| Amplitude feedback | Metering overlays | Waveform thumbnail on node | None | Ring animation | Pulse ring (v5.1) |
| Connection lines | None | None | None | Yes, when grouped | Yes, when within threshold |
| HRTF binaural | Yes (SOFA) | Yes | Yes | Yes | No (v6+) |
| Listener object | Yes, movable | Yes, movable | Yes, movable | Yes, movable | Implicit center (v5.0) |
| Node types | Audio objects only | Audio objects only | Audio objects only | Audio + effect nodes | Oscillators only (v5.0) |
| Persistence | Yes | Yes (Live Set) | Yes | Yes | No (v5.0) |

**Key insight from competitive analysis:** Every spatial tool uses the same node-as-circle visual language. The universal convention is dark background + colored circles + drag. ghostinstrument's differentiation is not in the visual language — it's in the node-to-node proximity interaction model, which no existing tool implements. That's where scope focus belongs.

---

## Audio Math Reference

### Equal-Power Stereo Panning (from X position)

```rust
// x_norm: normalized X position, 0.0 (left) to 1.0 (right)
let angle = x_norm * std::f32::consts::FRAC_PI_2;
let left_gain = angle.cos();
let right_gain = angle.sin();
// Apply to mono source to get stereo output
```

Confidence: HIGH — standard audio engineering formula, mathematically stable, used by every DAW.

### Equal-Power Proximity Blend (node-to-node)

```rust
// distance: Euclidean distance between nodes, normalized 0.0..=1.0 relative to blend_radius
// at distance=0: fully blended. at distance=blend_radius: dry (no blend).
let blend_ratio = (1.0 - (distance / blend_radius).min(1.0));
let wet_gain = (blend_ratio * std::f32::consts::FRAC_PI_2).sin();
let dry_gain = (blend_ratio * std::f32::consts::FRAC_PI_2).cos();
// wet_gain applied to cross-mix, dry_gain applied to original
```

Confidence: HIGH — same equal-power principle, applied to blend ratio instead of pan angle.

### Lock-Free Position Passing Pattern

```rust
// UI thread writes:
node.x_atomic.store(f32::to_bits(new_x), Ordering::Relaxed);
// Audio thread reads:
let x = f32::from_bits(node.x_atomic.load(Ordering::Relaxed));
```

`Relaxed` ordering is sufficient because a stale position for one audio callback (~10ms) is perceptually harmless. The alternative (SeqCst) adds unnecessary memory fencing on the hot audio path.

Confidence: HIGH — standard lock-free audio pattern, well-established in Rust audio community (cpal, dasp, fundsp ecosystem).

---

## Sources

- Ircam Spat documentation and spatial audio principles — MEDIUM confidence (training data, tool well-established)
- Envelop for Live feature set — MEDIUM confidence (training data, open-source Max for Live device)
- Dolby Atmos Renderer / ADM workflow — MEDIUM confidence (training data, commercial tool)
- iZotope Strands (2024 release) — LOW confidence (near training cutoff; feature set may differ)
- Equal-power panning law derivation — HIGH confidence (audio engineering standard, mathematically verifiable)
- Equal-power crossfade math — HIGH confidence (same formula, same properties)
- Rust AtomicU32 for f32 lock-free pattern — HIGH confidence (well-established Rust audio pattern, used in cpal examples and dasp)
- ghostinstrument.cog product vision — HIGH confidence (primary source, authored by project owner)
- PROJECT.md v5.0 scope definition — HIGH confidence (primary source)

---

*Feature research for: ghostinstrument v5.0 — spatial audio canvas, node interaction and proximity mixing*
*Researched: 2026-03-27*

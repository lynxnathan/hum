# Phase 3: Spatial Wiring - Context

**Gathered:** 2026-03-27
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire node drag positions to audio parameters in real-time. Left/right position controls stereo pan (equal-power law). Distance between nodes controls proximity blend (close = mixed, far = isolated). All parameter changes use one-pole smoothing to prevent zipper noise. This completes v5.0.

</domain>

<decisions>
## Implementation Decisions

### Stereo Panning
- Equal-power pan law: `left = cos(pan * PI/2)`, `right = sin(pan * PI/2)` where pan is 0.0 (left) to 1.0 (right)
- Pan value derived directly from node X position (0.0 = hard left, 1.0 = hard right)
- UI thread writes to fundsp::Shared handles; audio callback reads them per-sample

### Proximity Blending
- Distance between nodes computed in normalized canvas coordinates (0.0 to ~1.4 diagonal)
- Blend coefficient: `1.0 - (distance / BLEND_RADIUS).clamp(0.0, 1.0)` where BLEND_RADIUS = 0.3
- At distance 0: full blend (both oscillators audible in both channels)
- At distance >= 0.3: no blend (each oscillator only in its panned position)
- Equal-power crossfade on the blend: same cos/sin formula

### One-Pole Smoothing
- Coefficient: 0.995 (~7ms at 48kHz, ~5ms at 44.1kHz)
- Applied per-sample in cpal callback: `smoothed = smoothed * coeff + target * (1.0 - coeff)`
- Applied to: pan_a, pan_b, blend coefficient
- Prevents zipper noise on slow drags and clicks on fast position changes

### Thread Communication
- fundsp::Shared handles for pan_a, pan_b (already created in Phase 1)
- Add new Shared for proximity_blend
- UI thread calls `shared.set_value()` on each FingerMove
- Audio callback reads via `var(&shared)` or direct `.value()` call

### Claude's Discretion
- Whether to add proximity_blend as a third Shared in AudioParams or compute blend from pan values
- Exact smoothing coefficient (0.995 starting point, tune by ear)
- Whether to implement smoothing in fundsp graph or inline in callback

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `audio.rs` — AudioParams{pan_a, pan_b: Shared}, build_graph(), build_stream(), init_audio_async()
- `canvas.rs` — CanvasWidget with node_positions() accessor returning &[NodeState]
- `spatial.rs` — stub with pan_from_x() returning 0.0
- `nodes.rs` — NodeState{x, y, freq}

### Integration Points
- `canvas.rs handle_event` → compute spatial params → write to AudioParams Shared handles
- `audio.rs build_stream callback` → read Shared values → apply pan + blend per-sample
- `app.rs` → pass AudioParams reference to CanvasWidget so it can write spatial values

</code_context>

<specifics>
## Specific Ideas

- Phase 1 audio.rs callback currently mixes both oscillators at center pan — needs rewrite to apply per-node pan + blend
- spatial.rs replaces stub with real equal-power pan + proximity blend math
- App struct already holds `_audio_params: Option<Arc<AudioParams>>` — need to pass this to CanvasWidget

</specifics>

<deferred>
## Deferred Ideas

- HRTF / binaural (future milestone)
- Distance-based attenuation (v5.1)
- Per-node volume control (v5.1)
- Visual feedback of blend state (v5.1)

</deferred>

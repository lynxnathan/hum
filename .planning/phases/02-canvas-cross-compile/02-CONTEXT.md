# Phase 2: Canvas + Cross-Compile - Context

**Gathered:** 2026-03-27
**Status:** Ready for planning

<domain>
## Phase Boundary

Dark Makepad canvas with two draggable circles representing oscillator nodes. Mouse click-drag moves nodes freely. Node positions persist across frames. Cross-compiled to Windows .exe via cargo-xwin. No audio wiring yet — that's Phase 3.

</domain>

<decisions>
## Implementation Decisions

### Canvas & Nodes
- Two circles: Node A is cyan (#00DDFF), Node B is magenta (#FF44AA) — high contrast on dark background
- Node radius: 25px (large enough for easy click, small enough not to dominate)
- Starting positions: Node A at (30%, 40%), Node B at (70%, 60%) — offset from center to show they're independent
- Dark canvas: #111118 (matches existing app.rs background)

### Drag Interaction
- Hit detection: check distance from click point to node center, radius-based
- Drag: FingerDown starts drag (sets active_node), FingerMove updates position, FingerUp releases
- Only one node draggable at a time (first hit wins if overlapping)
- Node positions clamped to canvas bounds (can't drag off-screen)

### Rendering
- Custom Makepad widget with draw_2d shader for circle rendering using Sdf2d
- Makepad area-based hit detection (event.hits(cx, area) pattern from slider.rs)
- Redraw on every FingerMove during drag (Makepad repaints via cx.redraw_all)

### Claude's Discretion
- Exact widget structure (single custom widget vs multiple)
- Whether to use DrawQuad + shader or pure Sdf2d approach
- Canvas coordinate system (pixels vs normalized)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/bin/ghostinstrument/app.rs` — App struct with audio fields, LiveHook, dark window
- `src/bin/ghostinstrument/nodes.rs` — NodeState struct (x, y, freq) already defined
- Makepad patterns from hum-gui (arrangement_view, spectral_view use similar custom drawing)

### Established Patterns
- `live_design!` for widget declaration
- `#[derive(Live, LiveHook)]` for Makepad widgets
- `event.hits(cx, area)` for hit detection (from makepad-widgets slider.rs)
- `DrawQuad::draw_abs(cx, Rect)` for absolute-positioned drawing

### Integration Points
- `App.handle_event()` — dispatch events to canvas widget
- `NodeState` in nodes.rs — store positions here
- Phase 3 will read node positions and write to AudioParams

</code_context>

<specifics>
## Specific Ideas

- Research confirmed: Makepad DrawQuad + Sdf2d::circle is the correct approach for canvas circles
- Hit::FingerDown/Move/Up pattern from makepad-widgets slider.rs is canonical drag implementation
- Canvas must support absolute positioning (not turtle flow layout)

</specifics>

<deferred>
## Deferred Ideas

- Audio wiring from node position (Phase 3)
- Proximity blending visualization (v5.1)
- Node color shift based on proximity (v5.1)
- Amplitude pulse rings (v5.1)

</deferred>

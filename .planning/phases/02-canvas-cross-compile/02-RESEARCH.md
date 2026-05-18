# Phase 2: Canvas + Cross-Compile - Research

**Researched:** 2026-03-27
**Domain:** Makepad custom widget drawing — Sdf2d circles, drag events, canvas absolute positioning
**Confidence:** HIGH (all APIs verified from local makepad registry source)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Two circles: Node A is cyan (#00DDFF), Node B is magenta (#FF44AA) — high contrast on dark background
- Node radius: 25px (large enough for easy click, small enough not to dominate)
- Starting positions: Node A at (30%, 40%), Node B at (70%, 60%) — offset from center to show they're independent
- Dark canvas: #111118 (matches existing app.rs background)
- Hit detection: check distance from click point to node center, radius-based
- Drag: FingerDown starts drag (sets active_node), FingerMove updates position, FingerUp releases
- Only one node draggable at a time (first hit wins if overlapping)
- Node positions clamped to canvas bounds (can't drag off-screen)
- Custom Makepad widget with draw_2d shader for circle rendering using Sdf2d
- Makepad area-based hit detection (event.hits(cx, area) pattern from slider.rs)
- Redraw on every FingerMove during drag (Makepad repaints via cx.redraw_all)

### Claude's Discretion
- Exact widget structure (single custom widget vs multiple)
- Whether to use DrawQuad + shader or pure Sdf2d approach
- Canvas coordinate system (pixels vs normalized)

### Deferred Ideas (OUT OF SCOPE)
- Audio wiring from node position (Phase 3)
- Proximity blending visualization (v5.1)
- Node color shift based on proximity (v5.1)
- Amplitude pulse rings (v5.1)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CAN-01 | Dark canvas background fills the Makepad window | DrawQuad fill on View with width/height: Fill; `#111118` color |
| CAN-02 | Two circles rendered at distinct positions with different colors | `draw_abs` on DrawQuad with Sdf2d::circle shader; `instance color: vec4` per draw call |
| CAN-03 | User can click and drag a node to move it anywhere on the canvas | `event.hits(cx, area)` + `Hit::FingerDown/Move/Up`; `fe.abs` gives absolute DVec2 |
| CAN-04 | Node positions persist across frames (state tracked per node) | Store positions in NodeState (already in nodes.rs); widget holds `[NodeState; 2]` |
| BLD-01 | Cross-compile from WSL2 to x86_64-pc-windows-msvc via cargo-xwin | cargo-xwin 0.21.4 is installed; Makepad cross-compiles with no additional flags; pattern proven in Phase 1 |
</phase_requirements>

---

## Summary

Phase 2 adds a draggable canvas UI on top of the audio core built in Phase 1. The canvas is a single custom Makepad widget that owns two `NodeState` entries and draws them as colored circles using `DrawQuad` with a `Sdf2d::circle` shader. The widget implements `handle_event` with `event.hits(cx, area)` per-node, tracking which node (if any) is being dragged via an `active_node: Option<usize>` field.

The key architectural decision — confirmed by reading source — is that two separate `DrawQuad` instances are needed, one per node. Each call to `draw_abs` records a distinct hit-test area in the draw list. Calling `event.hits(cx, self.draw_node_a.area())` then `event.hits(cx, self.draw_node_b.area())` evaluates them in order; the first match wins, satisfying the "first hit wins if overlapping" constraint.

Cross-compilation is a configuration concern only. cargo-xwin 0.21.4 is installed and was proven in Phase 1. No additional Cargo.toml changes are needed for this phase — all deps (makepad-widgets, cpal, fundsp) are already present.

**Primary recommendation:** Single `CanvasWidget` struct with `draw_node_a: DrawQuad`, `draw_node_b: DrawQuad`, `nodes: [NodeState; 2]`, `active_node: Option<usize>`. Draw loop calls `draw_abs` for each node. Event loop calls `hits` for each node in priority order.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| makepad-widgets | 1.0.0 | Widget framework — `DrawQuad`, `Sdf2d`, hit events | Already in project; sole UI dependency |
| makepad-draw | 1.0.0 | `Sdf2d::circle`, `DrawQuad::draw_abs` | Pulled by makepad-widgets; no direct dep needed |

### No New Dependencies

All required libraries are already in `Cargo.toml`. No additions for Phase 2.

**Cross-compile command (already proven):**
```bash
cargo xwin build --bin ghostinstrument --target x86_64-pc-windows-msvc
```

---

## Architecture Patterns

### Recommended Widget Structure

```
src/bin/ghostinstrument/
├── app.rs           # App struct — add CanvasWidget field, register it
├── canvas.rs        # NEW — CanvasWidget with DrawQuad x2, drag logic
├── nodes.rs         # NodeState — already exists, add initial_pos() helper
├── audio.rs         # Unchanged from Phase 1
├── spatial.rs       # Unchanged from Phase 1
└── main.rs          # Unchanged
```

### Pattern 1: Custom Widget with Two DrawQuads

**What:** One widget struct owns two `DrawQuad` instances (one per node). Each has its own draw area, enabling independent `hits()` queries.

**When to use:** Any time you need N independently hit-testable GPU primitives that share state (drag tracking, position clamping).

**Full widget declaration (live_design!):**
```rust
// Source: makepad-draw-1.0.0/src/shader/draw_quad.rs + slider.rs pattern
live_design! {
    use link::shaders::*;

    DrawNode = {{DrawNode}} {
        fn pixel(self) -> vec4 {
            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
            sdf.circle(
                self.rect_size.x * 0.5,
                self.rect_size.y * 0.5,
                self.rect_size.x * 0.5 - 1.5
            );
            sdf.fill(self.node_color);
            return sdf.result;
        }
        instance node_color: vec4
    }

    pub CanvasWidget = {{CanvasWidget}} {
        width: Fill, height: Fill
        draw_bg: { color: #111118 }
    }
}
```

**Widget struct:**
```rust
// Source: makepad-widgets-1.0.0/src/button.rs struct pattern
#[derive(Live, LiveHook, Widget)]
pub struct CanvasWidget {
    #[walk]   walk: Walk,
    #[layout] layout: Layout,
    #[live]   draw_bg: DrawQuad,       // canvas background
    #[live]   draw_node_a: DrawNode,   // Node A (cyan)
    #[live]   draw_node_b: DrawNode,   // Node B (magenta)
    #[rust]   nodes: [NodeState; 2],
    #[rust]   active_node: Option<usize>,
    #[rust]   canvas_rect: Rect,       // stored from last draw for clamping
}
```

### Pattern 2: draw_abs for Absolute Positioning

**What:** `DrawQuad::draw_abs(cx, Rect { pos, size })` bypasses turtle layout entirely. The rect's `pos` is in window-absolute pixels.

**Critical detail — verified in draw_quad.rs line 127-131:**
```rust
// Source: makepad-draw-1.0.0/src/shader/draw_quad.rs line 127
pub fn draw_abs(&mut self, cx: &mut Cx2d, rect: Rect) {
    self.rect_pos = rect.pos.into();
    self.rect_size = rect.size.into();
    self.draw(cx);  // records area for subsequent hits() query
}
```

**Usage in draw_walk:**
```rust
// Source: makepad-draw-1.0.0/src/shader/draw_quad.rs + slider.rs pattern
fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
    let rect = cx.walk_turtle(walk);        // fills width/height: Fill
    self.canvas_rect = rect;
    self.draw_bg.draw_abs(cx, rect);        // dark background

    const R: f64 = 25.0;
    let diam = dvec2(R * 2.0, R * 2.0);

    // Convert normalized NodeState coords to canvas pixels
    let pos_a = dvec2(
        rect.pos.x + self.nodes[0].x as f64 * rect.size.x - R,
        rect.pos.y + self.nodes[0].y as f64 * rect.size.y - R,
    );
    self.draw_node_a.node_color = vec4(0.0, 0.867, 1.0, 1.0); // #00DDFF
    self.draw_node_a.draw_abs(cx, Rect { pos: pos_a, size: diam });

    let pos_b = dvec2(
        rect.pos.x + self.nodes[1].x as f64 * rect.size.x - R,
        rect.pos.y + self.nodes[1].y as f64 * rect.size.y - R,
    );
    self.draw_node_b.node_color = vec4(1.0, 0.267, 0.667, 1.0); // #FF44AA
    self.draw_node_b.draw_abs(cx, Rect { pos: pos_b, size: diam });

    DrawStep::done()
}
```

**Coordinate note:** `NodeState.x` and `NodeState.y` are normalized (0.0–1.0). Converting to pixels: `pixel_x = canvas_rect.pos.x + node.x * canvas_rect.size.x`. Canvas rect origin is NOT always (0,0) — it depends on window chrome.

### Pattern 3: Area-Based Hit Detection and Drag

**Verified from makepad-widgets-1.0.0/src/slider.rs lines 2771–2837:**

```rust
// Source: makepad-widgets-1.0.0/src/slider.rs handle_event
fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
    // Node A hit detection — checked first (higher priority)
    match event.hits(cx, self.draw_node_a.area()) {
        Hit::FingerDown(fd) if fd.device.is_primary_hit() => {
            self.active_node = Some(0);
        }
        Hit::FingerMove(fm) if self.active_node == Some(0) => {
            self.update_node_pos(0, fm.abs);
            self.draw_node_a.redraw(cx);
        }
        Hit::FingerUp(_) if self.active_node == Some(0) => {
            self.active_node = None;
        }
        _ => {}
    }

    // Node B hit detection — checked second
    match event.hits(cx, self.draw_node_b.area()) {
        Hit::FingerDown(fd) if fd.device.is_primary_hit() => {
            self.active_node = Some(1);
        }
        Hit::FingerMove(fm) if self.active_node == Some(1) => {
            self.update_node_pos(1, fm.abs);
            self.draw_node_b.redraw(cx);
        }
        Hit::FingerUp(_) if self.active_node == Some(1) => {
            self.active_node = None;
        }
        _ => {}
    }
}

fn update_node_pos(&mut self, idx: usize, abs: DVec2) {
    let r = &self.canvas_rect;
    // Convert absolute pixel coords back to normalized
    let nx = ((abs.x - r.pos.x) / r.size.x).clamp(0.0, 1.0) as f32;
    let ny = ((abs.y - r.pos.y) / r.size.y).clamp(0.0, 1.0) as f32;
    self.nodes[idx].x = nx;
    self.nodes[idx].y = ny;
}
```

**Critical details on `hits()`:**
- `event.hits(cx, area)` uses the bounding rect recorded by the most recent `draw_abs` call on that DrawQuad. It is NOT a circle test — it is rect-based.
- For a 50x50 rect centered on the circle, this is a square hit zone, not circular. This is acceptable for 25px radius nodes (the locked decision says radius-based detection, but Makepad's `hits()` is rect-based — see Pitfall 2).
- `fm.abs` and `fd.abs` are `DVec2` in window-absolute pixels. `fm.abs_start` is the position where the finger first went down.
- `fe.is_primary_hit()` filters to left mouse button / first touch only.

### Pattern 4: Registering the Custom Widget in App

```rust
// Source: makepad-widgets-1.0.0/src/button.rs + slider.rs LiveRegister pattern
impl LiveRegister for App {
    fn live_register(cx: &mut Cx) {
        makepad_widgets::live_design(cx);
        crate::canvas::live_design(cx);  // register CanvasWidget
    }
}
```

The `live_design` function is auto-generated by the `live_design!` macro in canvas.rs. It must be called in `LiveRegister` before the window opens, or Makepad panics with "unknown widget type."

### Pattern 5: Adding CanvasWidget to App live_design

```rust
// In app.rs live_design!
App = {{App}} {
    ui: <Window> {
        window: { inner_size: vec2(900, 600) }
        show_bg: true
        draw_bg: { color: #111118 }

        body = <CanvasWidget> {
            width: Fill, height: Fill
        }
    }
}
```

And dispatch events from App:
```rust
impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
```

The `ui.handle_event` call walks the widget tree and dispatches to CanvasWidget automatically.

### Anti-Patterns to Avoid

- **Using `draw_walk` instead of `draw_abs` for nodes:** `draw_walk` goes through turtle layout. Node positions would shift on window resize and cannot be freely placed. Always use `draw_abs` for canvas-positioned elements.
- **Calling `cx.redraw_all()` on every event:** Only call `redraw` on the specific draw area that changed (`self.draw_node_a.redraw(cx)`). `redraw_all` redraws the entire window, which works but is wasteful.
- **Storing canvas pixel coords in NodeState:** NodeState uses normalized 0.0–1.0. Always convert at draw time. This makes Phase 3 spatial math trivial.
- **Sharing one DrawQuad for both nodes:** Each node needs its own DrawQuad instance to have its own hit area. Two draws from the same struct would overwrite the area reference.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| GPU circle rendering | Custom OpenGL calls | `DrawQuad` + `Sdf2d::circle` shader | Makepad manages GL context, shader compilation, and GPU upload |
| Hit test region tracking | Manual rect storage | `draw_abs` + `event.hits(cx, area)` | `draw_abs` registers the area in Makepad's draw list automatically |
| Redraw scheduling | Manual dirty flags | `DrawQuad::redraw(cx)` | Makepad's redraw system batches and deduplicates |
| Cross-compile toolchain | Custom MSVC setup | `cargo xwin` | Already proven; handles MSVC SDK, linker, and Windows SDK headers |

---

## Common Pitfalls

### Pitfall 1: live_design registration order
**What goes wrong:** App panics at startup with "unknown widget CanvasWidget" or similar.
**Why it happens:** `live_register` did not call `crate::canvas::live_design(cx)` before `makepad_widgets::live_design(cx)` — or called it after.
**How to avoid:** In `LiveRegister for App`, call `makepad_widgets::live_design(cx)` first, then all custom widget `live_design` calls.
**Warning signs:** Panic at window open, not at compile time.

### Pitfall 2: Makepad hit detection is rect-based, not circle-based
**What goes wrong:** Click on the corner of the node's bounding box registers as a hit even though visually outside the circle.
**Why it happens:** `event.hits(cx, area)` tests against the axis-aligned bounding rect recorded by `draw_abs`, not the SDF circle shape.
**How to avoid:** For Phase 2 this is acceptable — the node radius (25px) and bounding box (50x50px) are close enough. For pixel-perfect circle detection, check `(fd.abs - node_center).length() < RADIUS` inside the `Hit::FingerDown` branch and ignore the hit if outside.
**Warning signs:** Clicks in corners of node bounding box register unexpectedly.

### Pitfall 3: Canvas rect origin is not (0, 0)
**What goes wrong:** Nodes appear offset from expected positions, or `update_node_pos` calculates wrong normalized coords.
**Why it happens:** The canvas rect origin (`cx.walk_turtle(walk).pos`) may be (0, 8) or similar due to window title bar or padding. Subtracting only `r.pos.x` from `fm.abs.x` is correct; forgetting the subtraction places nodes in the wrong spot.
**How to avoid:** Always store the full `canvas_rect` from the last `draw_walk`, including `.pos`. Use `abs.x - canvas_rect.pos.x` for X normalization.
**Warning signs:** Nodes start at bottom-right instead of expected 30%/70% positions.

### Pitfall 4: `instance` vs `uniform` in DrawQuad shader
**What goes wrong:** Node color does not change between nodes — both nodes render the same color.
**Why it happens:** Using `uniform node_color: vec4` instead of `instance node_color: vec4`. Uniforms are per-draw-call-batch; instances are per-draw.
**How to avoid:** Declare `instance node_color: vec4` in the live_design shader. Set `self.draw_node_a.node_color = ...` before each `draw_abs` call. Verified: DrawQuad fields tagged `#[live]` and declared as `instance` in the shader are per-instance.
**Warning signs:** Both circles are the same color; only the last color set is used.

### Pitfall 5: `active_node` guard on FingerMove
**What goes wrong:** Moving mouse after releasing a node still moves it, or the wrong node moves.
**Why it happens:** The `Hit::FingerMove` guard `self.active_node == Some(idx)` was omitted.
**How to avoid:** Always gate FingerMove on `active_node`. Makepad sends FingerMove to the area that received FingerDown (capture semantics), so the area check alone is insufficient — you must also check `active_node`.
**Warning signs:** Node teleports on release; second node moves when first was just released.

---

## Code Examples

### Complete DrawNode shader in live_design!
```rust
// Source: makepad-draw-1.0.0/src/shader/std.rs Sdf2d::circle (line 302)
//         makepad-widgets-1.0.0/src/slider.rs Sdf2d::viewport pattern (line 80)
DrawNode = {{DrawNode}} {
    instance node_color: vec4

    fn pixel(self) -> vec4 {
        let sdf = Sdf2d::viewport(self.pos * self.rect_size);
        sdf.circle(
            self.rect_size.x * 0.5,
            self.rect_size.y * 0.5,
            self.rect_size.x * 0.5 - 1.5   // 1.5px inset for antialiasing
        );
        sdf.fill(self.node_color);
        return sdf.result;
    }
}
```

### Sdf2d::circle exact signature
```rust
// Source: makepad-draw-1.0.0/src/shader/std.rs line 302
fn circle(inout self, x: float, y: float, r: float) {
    let c = self.pos - vec2(x, y);
    let len = sqrt(c.x * c.x + c.y * c.y);
    self.dist = (len - r) / self.scale_factor;
    self.old_shape = self.shape;
    self.shape = min(self.shape, self.dist);
}
```
Parameters: center `x`, center `y`, radius `r` — all in pixel coordinates relative to the quad rect (not UV).

### DrawQuad::draw_abs exact signature
```rust
// Source: makepad-draw-1.0.0/src/shader/draw_quad.rs line 127
pub fn draw_abs(&mut self, cx: &mut Cx2d, rect: Rect) {
    self.rect_pos = rect.pos.into();
    self.rect_size = rect.size.into();
    self.draw(cx);
}
```
`Rect { pos: DVec2, size: DVec2 }` — pos is window-absolute origin of the quad.

### FingerMove coordinate fields
```rust
// Source: makepad-widgets-1.0.0/src/slider.rs line 2820-2834
Hit::FingerMove(fe) => {
    let rel = fe.abs - fe.abs_start;  // delta from finger-down origin
    // fe.abs    — current absolute position (DVec2, window pixels)
    // fe.abs_start — position where FingerDown occurred
    // fe.rect   — the bounding rect of the hit area
}
```

### Normalized position clamping
```rust
// Clamp node to canvas bounds (0.0–1.0 normalized)
fn update_node_pos(&mut self, idx: usize, abs: DVec2) {
    let r = &self.canvas_rect;
    let nx = ((abs.x - r.pos.x) / r.size.x).clamp(0.0, 1.0) as f32;
    let ny = ((abs.y - r.pos.y) / r.size.y).clamp(0.0, 1.0) as f32;
    self.nodes[idx].x = nx;
    self.nodes[idx].y = ny;
}
```

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| cargo-xwin | BLD-01 cross-compile | Yes | 0.21.4 | None needed |
| x86_64-pc-windows-msvc target | BLD-01 | Yes (proven Phase 1) | Rust 1.92.0 | None needed |
| makepad-widgets | CAN-01..04 | Yes (in Cargo.toml) | 1.0.0 | None |
| Windows .exe execution | BLD-01 verification | Windows host needed | — | Run under Wine (limited) |

**Missing dependencies with no fallback:**
- None that block compilation. BLD-01 only requires building the .exe from WSL2; running it requires Windows, which is available (per STATE.md: shared-target path confirmed on Windows).

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| `draw_walk` for all elements | `draw_abs` for canvas nodes | Nodes stay at absolute positions regardless of layout |
| Separate crates for 2D drawing | Makepad's built-in `Sdf2d` | Zero extra deps; GPU-accelerated SDF antialiasing |
| `cx.redraw_all()` | `draw_quad.redraw(cx)` | Only repaints the specific draw area, not full window |

---

## Open Questions

1. **DrawNode must be a separate struct from DrawQuad?**
   - What we know: `DrawQuad` is the base; custom shaders subclass it with `{{DrawNode}}` syntax in live_design and a Rust struct deriving from it via `#[deref] draw_vars: DrawVars`.
   - What's unclear: Whether `#[derive(Live, LiveHook)] struct DrawNode { #[deref] draw_vars: DrawVars, #[live] node_color: Vec4 }` is the exact pattern, or if `DrawQuad` can be used directly with an `instance` field set at runtime.
   - Recommendation: Use a custom `DrawNode` struct extending `DrawVars` for clarity. If that fails, use `DrawQuad` with `draw_vars.set_uniform(cx, id!(node_color), &[r, g, b, a])` as a fallback.

2. **Does `redraw(cx)` on one DrawQuad redraw only that quad, or the full widget?**
   - What we know: `DrawQuad::redraw(cx)` calls `cx.redraw(self.area())` which marks that area dirty.
   - What's unclear: Whether Makepad redraws the entire widget's `draw_walk` or just the dirty area.
   - Recommendation: Call `redraw` on both draw_node instances and the draw_bg to be safe. Alternatively call `cx.redraw_all()` during drag — correctness over performance for Phase 2.

---

## Sources

### Primary (HIGH confidence)
- makepad-draw-1.0.0/src/shader/draw_quad.rs (local registry) — `draw_abs`, `draw_walk`, `update_abs` exact signatures
- makepad-draw-1.0.0/src/shader/std.rs line 302 (local registry) — `Sdf2d::circle(x, y, r)` exact signature
- makepad-widgets-1.0.0/src/slider.rs lines 2771–2837 (local registry) — `event.hits`, `Hit::FingerDown/Move/Up`, `fe.abs`, `is_primary_hit()` exact pattern
- makepad-widgets-1.0.0/src/button.rs lines 578–638 (local registry) — `#[derive(Live, LiveHook, Widget)]` struct layout, `#[walk]`, `#[layout]`, `#[live]`, `#[rust]` field attributes
- makepad-widgets-1.0.0/src/view.rs line 771 (local registry) — `draw_bg.draw_abs(cx, rect)` for background fill pattern
- src/bin/ghostinstrument/audio.rs — `AudioParams` with `fundsp::Shared` already implemented
- src/bin/ghostinstrument/nodes.rs — `NodeState { x, y, freq }` struct already defined
- src/bin/ghostinstrument/app.rs — current `App` struct with `after_new_from_doc`, `LiveRegister` pattern

### Secondary (MEDIUM confidence)
- STACK.md (prior research, 2026-03-27) — cpal/fundsp patterns and cross-compile validation

---

## Metadata

**Confidence breakdown:**
- DrawQuad draw_abs API: HIGH — read directly from draw_quad.rs source
- Sdf2d::circle signature: HIGH — read directly from std.rs source
- event.hits / FingerDown/Move/Up pattern: HIGH — read directly from slider.rs source
- Widget struct attribute syntax: HIGH — read directly from button.rs source
- live_design registration order: HIGH — read directly from slider.rs + button.rs
- instance vs uniform for per-node color: HIGH — derived from understanding of Makepad's instancing, verified by slider.rs `instance` usage
- cargo-xwin cross-compile: HIGH — tool present at 0.21.4, proven in Phase 1

**Research date:** 2026-03-27
**Valid until:** 2026-04-27 (makepad-widgets 1.0.0 is stable; APIs unlikely to change)

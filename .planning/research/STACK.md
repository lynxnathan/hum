# Stack Research

**Domain:** Spatial audio canvas — real-time oscillator nodes with stereo panning, draggable in a GPU-accelerated Makepad canvas, cross-compiled to Windows via cargo-xwin
**Researched:** 2026-03-27
**Confidence:** HIGH (cpal/fundsp from crates.io metadata; Makepad APIs verified from registry source)

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| cpal | 0.17.3 | Audio I/O — output stream to WASAPI on Windows | Latest stable; 0.17 made `Stream: Send + Sync`, removing the per-thread hack required in 0.15. WASAPI is the default Windows host — no feature flags needed. Requires Rust >= 1.82 for WASAPI backend (met: project uses 1.92.0). |
| fundsp | 0.23.0 | DSP graph — oscillators, panning, mixing | Pure Rust, zero external deps, no_std compatible (use `default-features = false` to drop the `rustfft` convolution engine — not needed here). Composable graph notation: `sine_hz(440.0) >> pan(-0.5)` is a complete stereo panned oscillator. Has built-in `pan()` node with equal-power panning. Cross-compiles cleanly via cargo-xwin because it is 100% Rust. |
| makepad-widgets | 1.0.0 | UI framework — canvas, events | Already validated. Provides `DrawQuad` + `Sdf2d` for GPU shader drawing, `Hit::FingerDown/Move/Up` for drag interactions. No additional drawing crate needed. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| atomic_float | 0.1 | `AtomicF32` — shared atomic float between threads | Use for sharing pan values (1 per node) between Makepad UI thread and cpal audio callback. `Arc<AtomicF32>` cloned into cpal closure. Prefer over Mutex (which blocks) and over crossbeam-channel (overkill for 2 floats). |
| crossbeam-channel | 0.5 | Lock-free MPSC channel for UI -> audio events | Bring in later when parameter sets grow complex (node add/remove, pitch changes). For v5.0 — two pan floats — atomic_float is sufficient. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| cargo-xwin | Cross-compile WSL2 -> `x86_64-pc-windows-msvc` | Already proven for Makepad. cpal 0.17 uses the `windows` crate for WASAPI — cargo-xwin provides MSVC SDK headers and libs automatically. No extra config beyond Phase 01. |

---

## Cargo.toml Additions

```toml
[dependencies]
cpal = "0.17"
fundsp = { version = "0.23", default-features = false }
atomic_float = "0.1"
```

No feature flags on cpal for Windows WASAPI — it is the default host on `x86_64-pc-windows-msvc`. Do not enable the `asio` feature unless you need ASIO hardware.

---

## API Patterns

### 1. cpal output stream with fundsp oscillators

```rust
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use fundsp::hacker::*;
use atomic_float::AtomicF32;
use std::sync::{Arc, atomic::Ordering};

// Pan control: -1.0 (left) to 1.0 (right), derived from node X position
let pan_a = Arc::new(AtomicF32::new(0.0_f32));
let pan_b = Arc::new(AtomicF32::new(0.0_f32));

let pan_a_audio = Arc::clone(&pan_a);
let pan_b_audio = Arc::clone(&pan_b);

// fundsp oscillators — constructed once, ticked per callback
let mut osc_a = Box::new(sine_hz(440.0));
let mut osc_b = Box::new(sine_hz(528.0));
osc_a.reset();
osc_b.reset();

let host = cpal::default_host();
let device = host.default_output_device().expect("no output device");
let config = device.default_output_config().unwrap();

let stream = device.build_output_stream(
    &config.into(),
    move |data: &mut [f32], _| {
        let pa = pan_a_audio.load(Ordering::Relaxed);
        let pb = pan_b_audio.load(Ordering::Relaxed);

        // Equal-power panning: angle in 0..PI/2
        let angle_a = (pa + 1.0) * std::f32::consts::FRAC_PI_4;
        let (la, ra) = (angle_a.cos(), angle_a.sin());

        let angle_b = (pb + 1.0) * std::f32::consts::FRAC_PI_4;
        let (lb, rb) = (angle_b.cos(), angle_b.sin());

        let frames = data.len() / 2; // stereo interleaved
        for i in 0..frames {
            let sa = osc_a.get_mono() * 0.4;
            let sb = osc_b.get_mono() * 0.4;
            data[i * 2]     = sa * la + sb * lb; // left
            data[i * 2 + 1] = sa * ra + sb * rb; // right
        }
    },
    |err| eprintln!("audio error: {err}"),
    None,
).expect("failed to build output stream");

stream.play().unwrap();
// Store `stream` in App struct — dropping it stops audio silently
```

> Why inline pan instead of fundsp `pan()` node: `pan(x)` in fundsp bakes pan at graph construction time. Dynamic control requires rebuilding the graph or using fundsp's `Net` frontend (heavier). Inline equal-power math is 3 lines and RT-safe. The formula is identical to what fundsp `pan()` implements internally.

### 2. Makepad canvas drawing — circles at absolute positions

Makepad's `DrawQuad` is the base GPU primitive. Circles use GLSL-in-Rust shaders via `Sdf2d::circle()`. The key method is `draw_abs` — verified in makepad-draw-1.0.0 source:

```rust
// In live_design! macro (DSL):
// DrawNode = {{DrawNode}} {
//     fn pixel(self) -> vec4 {
//         let sdf = Sdf2d::viewport(self.pos * self.rect_size);
//         sdf.circle(
//             self.rect_size.x * 0.5,
//             self.rect_size.y * 0.5,
//             self.rect_size.x * 0.5 - 2.0
//         );
//         sdf.fill(#8af);
//         return sdf.result;
//     }
// }

// In draw_walk() implementation:
const RADIUS: f64 = 24.0;
self.draw_node.draw_abs(cx, Rect {
    pos: dvec2(node.x - RADIUS, node.y - RADIUS),
    size: dvec2(RADIUS * 2.0, RADIUS * 2.0),
});
```

`draw_abs` places a quad at canvas-absolute coordinates, bypassing the turtle layout engine entirely. This is the correct approach for freely-positioned nodes on a canvas. The area returned becomes the hit-test region for events.

`Sdf2d::circle(cx, cy, r)` signature verified in makepad-draw-1.0.0/src/shader/std.rs line 302.

### 3. Makepad mouse event handling — drag

Verified pattern from makepad-widgets-1.0.0/src/slider.rs:

```rust
// In handle_event():
match event.hits(cx, self.draw_node.area()) {
    Hit::FingerDown(fd) if fd.device.is_primary_hit() => {
        self.dragging = true;
    }
    Hit::FingerMove(fm) if self.dragging => {
        self.node_pos = fm.abs; // DVec2, canvas-absolute coords
        let pan = (fm.abs.x / self.canvas_width) * 2.0 - 1.0;
        self.pan_atomic.store(pan.clamp(-1.0, 1.0) as f32, Ordering::Relaxed);
        self.redraw(cx);
    }
    Hit::FingerUp(_) => {
        self.dragging = false;
    }
    _ => {}
}
```

`event.hits(cx, area)` performs hit detection against the last bounding rect drawn for that area. For two overlapping nodes, call `hits` for each in priority order — the first match wins. `FingerDown.abs` and `FingerMove.abs` are in window-absolute `DVec2` coordinates.

### 4. Thread-safe UI -> audio communication

```
UI Thread (Makepad event loop)        Audio Thread (cpal WASAPI callback)
──────────────────────────────        ────────────────────────────────────
FingerMove fires                       Runs every ~10ms buffer
  │                                             │
  ├─ pan_a.store(v, Relaxed) ──Arc──► pan_a.load(Ordering::Relaxed)
  └─ pan_b.store(v, Relaxed) ──Arc──► pan_b.load(Ordering::Relaxed)
                                                │
                                        apply panning in sample loop
```

`Ordering::Relaxed` is correct: pan updates have no ordering dependency on other memory. A stale value by one buffer (approximately 10ms) is inaudible and acceptable. `AtomicF32` is implemented as `AtomicU32` with float-to-bits reinterpretation — atomic on all x86 targets, including Windows.

The `Arc<AtomicF32>` pair:
1. Created in `App::new()` (or wherever the audio stream is initialized)
2. Cloned into the cpal closure (`move`, requires `Send` — `AtomicF32` is `Send`)
3. Second clone kept in `App` struct for UI thread writes
4. `Stream` stored in `App` struct to keep audio alive for program lifetime

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| cpal 0.17.3 | cpal 0.15.x | 0.15 `Stream` is not `Send` on Windows — requires a workaround thread. 0.17 fixed this cleanly. |
| `Arc<AtomicF32>` | `Arc<Mutex<f32>>` | `Mutex::lock()` in audio callback is RT-unsafe — can block on OS scheduling causing dropouts. Never lock in audio callbacks. |
| `Arc<AtomicF32>` | crossbeam-channel | Overkill for 2 floats. Channel has allocation overhead. Add when sending structured events (node add/remove, pitch changes). |
| fundsp 0.23 | dasp | Lower-level, no graph notation, more boilerplate for the same sine oscillator. |
| fundsp 0.23 | rodio | File playback library, no synthesis primitives. |
| Inline pan math in callback | fundsp `pan()` node | `pan()` bakes pan at graph construction — not real-time controllable without graph rebuild or `Net` frontend. Inline is 3 lines and equally correct. |
| `draw_abs` on DrawQuad | Widget layout system | Widgets use turtle flow — positions shift on window resize. Canvas nodes need fixed absolute coordinates. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `Mutex` in cpal callback | Blocks audio thread; causes audible dropouts under system load | `AtomicF32` for floats, crossbeam-channel for complex data |
| fundsp with default features | Pulls in `rustfft` for convolution engine not needed for oscillators; adds compile time | `fundsp = { version = "0.23", default-features = false }` |
| cpal `asio` feature | Requires proprietary ASIO SDK download; not cross-compilable from WSL2 without SDK on host | Default WASAPI — no feature flag |
| `draw_walk` for canvas nodes | Positions in turtle flow — can't freely place at arbitrary canvas coordinates | `draw_abs(cx, Rect { pos, size })` |
| rodio | Built on cpal but abstracts away the output callback — no DSP injection point | cpal directly |

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| cpal 0.17.3 | Rust >= 1.82 | WASAPI backend uses `windows` crate. Project is on 1.92.0 — satisfied. |
| fundsp 0.23.0 | Rust stable, edition 2021+ | Compiles on edition 2024. `default-features = false` removes rustfft dep. |
| makepad-widgets 1.0.0 | Rust stable | Cross-compiles to Windows MSVC via cargo-xwin — proven in Phase 01. |
| atomic_float 0.1 | Any stable Rust | Pure Rust wrapping `AtomicU32`. No system deps. |

---

## Cross-Compilation Confidence Assessment

**cpal 0.17 + cargo-xwin:** HIGH. cpal's WASAPI backend uses Microsoft's `windows` crate (pure Rust FFI bindings), not raw system WASAPI headers. cargo-xwin supplies the Windows SDK so the `windows` crate can link against `mmdevapi.lib`. This is the documented path. No additional configuration beyond Phase 01 is required.

**fundsp 0.23:** HIGH. Pure Rust, zero C/C++ dependencies, zero system libraries. Cross-compiles identically to native Linux build.

**atomic_float 0.1:** HIGH. Three files of pure Rust. Trivially cross-compiles.

---

## Sources

- crates.io/crates/cpal — version 0.17.3 verified (MEDIUM-HIGH: crates.io listing)
- docs.rs/crate/cpal/latest — WASAPI default backend, Rust >= 1.82 requirement, Send+Sync streams (HIGH: official docs)
- reddit.com/r/rust/comments/1prot31 — cpal 0.17.0 release notes, Send+Sync streams confirmed (MEDIUM)
- github.com/SamiPerttu/fundsp — version 0.23.0, `default-features = false` pattern (HIGH: official repo)
- docs.rs/fundsp/latest — shared atomics for real-time control, `pan()` node documented (HIGH: official docs)
- makepad-draw-1.0.0/src/shader/draw_quad.rs (local registry source) — `draw_abs`, `draw_walk`, `update_abs` APIs (HIGH: source verified)
- makepad-draw-1.0.0/src/shader/std.rs (local registry source) — `Sdf2d::circle(x, y, r)` signature (HIGH: source verified)
- makepad-widgets-1.0.0/src/slider.rs (local registry source) — `Hit::FingerDown`, `Hit::FingerMove`, `event.hits()` patterns (HIGH: source verified)
- bekk.christmas/post/2023/19/make-some-noise-with-rust — cpal+fundsp integration example (MEDIUM: community post, 2024)
- timur.audio/using-locks-in-real-time-audio-processing-safely — AtomicF32 vs Mutex rationale for RT audio (MEDIUM: authoritative community article)

---

*Stack research for: ghostinstrument v5.0 — spatial audio canvas*
*Researched: 2026-03-27*

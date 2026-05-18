# Strategic Research: Makepad vs GPUI for HUM Sound IDE + MilkDrop Visualizer

**Researched:** 2026-03-22
**Domain:** Rust UI frameworks for creative audio tools — GPU rendering, shader pipelines, WSL2 compatibility
**Confidence:** MEDIUM (Makepad 1.0 API not fully verifiable via Context7; claims sourced from official GitHub, crates.io, and Hacker News thread)

---

## Summary

HUM needs two distinct UI capabilities: (1) a **sound IDE** with text editing, keyboard management, and terminal-like interaction; and (2) a **MilkDrop-style real-time audio visualizer** driven by custom fragment shaders. These are almost opposite requirements — one is widget-heavy retained UI, the other is a fullscreen shader pipeline.

Makepad reached 1.0 in May 2025. Its entire rendering stack is shader-based (all primitives are GPU quads), it has its own Rust-embedded shading DSL that cross-compiles to GLSL/HLSL/MetalSL, and it explicitly lists WSL2 Linux as a supported target (OpenGL backend, Mesa deps documented). This makes it the strongest candidate for the visualizer window. However, Makepad's widget ecosystem is smaller than egui's, and there is no evidence of a built-in terminal emulator component.

GPUI (0.2.2) confirmed working on WSL2 via X11 in the existing spike. It has rich text editing precedent (it powers Zed). However, GPUI explicitly lacks a user-facing custom GPU render surface — the zed-industries GitHub discussion #45996 confirms "GPUI currently lacks a rendering context like HTML Canvas or wgpu for custom GPU render." Custom shaders inside GPUI are not on the near-term roadmap.

**Primary recommendation:** Use a **split-window architecture** — GPUI for the IDE chrome (text editing, keyboard, transport controls) + a separate wgpu or Makepad window for the MilkDrop visualizer. Do not attempt to embed a fullscreen shader pipeline inside GPUI. Alternatively, use **Makepad for everything** if the team accepts its steeper learning curve and smaller widget ecosystem.

---

## Availability and Maturity

### Makepad

| Property | Value |
|----------|-------|
| crates.io name | `makepad-widgets`, `makepad-platform` |
| Latest version | 1.0.0 (released 2025-05-13) |
| Stability | Stable release — 1.0 milestone reached |
| License | MIT |
| Rendering backend | OpenGL (Linux/WSL2), Metal (macOS), DirectX 11 (Windows), WebGL (WASM) |
| WSL2 support | Explicitly documented — `linux_deps.sh` includes Mesa EGL/GL/GLES packages |
| Shader system | Custom Rust DSL → cross-compiled to GLSL/HLSL/MetalSL at runtime |

### GPUI

| Property | Value |
|----------|-------|
| crates.io name | `gpui` |
| Latest version | 0.2.2 (pre-1.0) |
| Stability | Pre-1.0, breaking changes confirmed |
| License | Apache 2.0 |
| Rendering backend | wgpu → Vulkan (Linux), Metal (macOS), DirectX 11 (Windows) |
| WSL2 support | Working in HUM spike via X11 (`WAYLAND_DISPLAY=""`) — MEDIUM confidence |
| Custom shader support | NOT available — confirmed gap per zed-industries discussion #45996 |

---

## Feature Comparison: HUM Use Cases

| Capability | Makepad 1.0 | GPUI 0.2.2 | Winner |
|------------|-------------|-----------|--------|
| Custom fragment shaders | YES — Rust DSL compiles to GLSL/HLSL/Metal | NO — no user-facing shader surface | Makepad |
| MilkDrop-style fullscreen shader | YES — every primitive is a shader quad | NO | Makepad |
| Audio FFT → shader uniforms | Possible via uniform buffer | Not possible (no shader access) | Makepad |
| Text editor component | YES — Makepad Studio is a code editor built in Makepad | YES — Zed is built entirely on GPUI | Tie |
| Terminal emulator | NOT confirmed — no evidence in search results | NOT trivial — Termy exists but is a separate project | Neither (both require effort) |
| Keyboard shortcut management | YES — action system in live_design DSL | YES — Action trait, key bindings system | Tie |
| WSL2 Linux | YES — Mesa OpenGL, explicitly documented | YES — working spike (X11 path) | Tie |
| VU meters / timeline widgets | Basic widget system (List, Label, etc.) | Via gpui-component (60+ widgets) | GPUI edge |
| Widget ecosystem size | Smaller (first-party focus) | Larger (gpui-component library) | GPUI |
| Live-reload / hot coding | YES — core feature, shaders update without recompile | NO | Makepad |
| Stability / API churn | Stable 1.0 | Pre-1.0, breaking changes expected | Makepad |
| Learning resources | Makepad Book at makepad.rs/guide | docs.rs + Zed codebase | Tie |

---

## The Shader Question: What MilkDrop Actually Needs

MilkDrop visualizers require:

1. **Audio FFT buffer** — typically 512–2048 frequency bins, updated per frame (~60fps)
2. **Per-frame shader uniforms** — `u_time`, `u_beat`, `u_bass`, `u_mid`, `u_treb`, `u_fft[512]`
3. **Fragment shader** — runs per-pixel on a fullscreen quad, reads uniforms + previous frame texture
4. **Feedback texture** — previous frame is passed back in as a sampler for trail/blur effects

In Makepad's shader DSL you write this as Rust-like code inside `live_design!{}` macros that compiles to GLSL on Linux. The feedback texture and uniform buffers are standard GPU primitives that Makepad exposes. This is architecturally correct for MilkDrop.

In GPUI, there is no way to inject a custom fragment shader or fullscreen render pass. GPUI renders UI primitives (text, boxes, images, SVG) via its internal WGSL shaders — users cannot add new shader stages. Discussion #45996 explicitly requests this feature with no committed timeline.

**Alternative: wgpu directly.** wgpu (which GPUI uses internally) fully supports custom render pipelines, WGSL fragment shaders, storage buffers for FFT data, and feedback textures. A standalone wgpu window for the visualizer is lower-level but gives complete control and is well-documented.

---

## The projectM Option

projectM is the open-source MilkDrop-compatible visualizer library. Official Rust bindings exist on crates.io as `projectm` (LGPL-2.1), wrapping libprojectM via FFI. It reads audio and renders MilkDrop presets directly. This is the highest-leverage path for MilkDrop compatibility — it handles all the preset parsing, FFT, beat detection, and shader compilation internally.

| Property | Value |
|----------|-------|
| crates.io name | `projectm` |
| Upstream | projectM-visualizer/projectm-rs |
| Backend | OpenGL (native libprojectM) |
| MilkDrop preset support | YES — full .milk preset format |
| WSL2 | Requires OpenGL — same Mesa path as Makepad |
| License | LGPL-2.1 |

**Risk:** projectm-rs is FFI bindings, not pure Rust. Requires libprojectM shared library installed. Build complexity is higher than a pure-Rust solution.

---

## Architecture Options

### Option A: GPUI IDE + wgpu Visualizer Window (Recommended)

```
┌─────────────────────────────┐   ┌──────────────────────────────┐
│  GPUI Window (IDE chrome)   │   │  wgpu Window (Visualizer)    │
│  - Text editor (.hum files) │   │  - Custom fragment shader    │
│  - VU meters / timeline     │   │  - FFT uniform buffer        │
│  - Transport controls       │   │  - Feedback texture loop     │
│  - Keyboard shortcuts       │   │  - MilkDrop-style presets    │
└─────────────────────────────┘   └──────────────────────────────┘
         ↑                                     ↑
    Existing spike                  New window via winit + wgpu
    (WSL2 confirmed)
```

**Pros:** Leverages existing GPUI spike; wgpu is the lowest-level most-flexible shader path; two windows can share audio data via Arc<Mutex<FftBuffer>>.

**Cons:** Two frameworks in the codebase; wgpu shader window requires more boilerplate than Makepad.

### Option B: Makepad for Everything

```
┌──────────────────────────────────────────────────────────────┐
│  Makepad Application                                         │
│  ┌──────────────────────┐  ┌─────────────────────────────┐  │
│  │  IDE Panel           │  │  Visualizer Panel           │  │
│  │  (live_design DSL)   │  │  (custom shader quad)       │  │
│  └──────────────────────┘  └─────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

**Pros:** Single framework; shader pipeline is first-class; live-reload is a core feature (ideal for a creative music tool); stable 1.0 API; WSL2 explicitly supported.

**Cons:** Steeper learning curve; smaller widget ecosystem; no confirmed terminal emulator component; team has zero Makepad experience vs existing GPUI spike.

### Option C: GPUI IDE + projectM Visualizer

Same split as Option A but uses projectM FFI bindings for the visualizer instead of hand-rolled wgpu shaders. Best for MilkDrop preset compatibility (.milk files). Highest build complexity.

---

## Can Makepad and GPUI Coexist?

Technically yes — they are separate crates that each create their own OS windows. They do not share a rendering context. Both can run in the same process, each managing their own window. In practice this means two event loops, which requires one to run on a background thread. This is non-trivial but possible (Makepad uses its own event loop; GPUI uses its own). The shared data (FFT buffer, transport state) would pass via `Arc<Mutex<T>>`.

**Verdict:** Coexistence is possible but adds integration complexity. Prefer one of the clean split options above.

---

## WSL2 Compatibility Summary

| Framework | WSL2 Path | Status |
|-----------|-----------|--------|
| GPUI 0.2.2 | X11 via WSLg (`WAYLAND_DISPLAY=""`) | Confirmed working (HUM spike) |
| Makepad 1.0 | OpenGL via Mesa EGL, WSLg X11 | Documented in linux_deps.sh — HIGH confidence |
| wgpu standalone | Vulkan via D3D12 translation OR OpenGL via ANGLE | MEDIUM — Vulkan path fragile on WSL2 |
| projectM (FFI) | OpenGL via Mesa | Same as Makepad — should work |

---

## Common Pitfalls

### Pitfall 1: Embedding a Shader Pipeline Inside GPUI
GPUI's `Element` trait only exposes layout and paint operations using GPUI's own scene primitives — it does not give access to the underlying wgpu device/queue. Do not attempt to smuggle a fullscreen render pass through GPUI's paint cycle.

### Pitfall 2: Makepad Shader DSL Is Not GLSL
Makepad's shading language looks like GLSL but is a Rust-embedded DSL (`live_design!{}` macros). It cross-compiles at runtime. You cannot paste raw GLSL from ShaderToy directly — it must be translated into Makepad's syntax. Allow time for this learning curve.

### Pitfall 3: FFT Buffer Size and Update Rate Mismatch
MilkDrop expects FFT data updated at audio render rate (~44100/512 ≈ 86Hz). GPU frames run at 60fps. These rates must be decoupled: audio thread writes to a ring buffer, GPU thread reads latest snapshot per frame.

### Pitfall 4: projectM Build Complexity on WSL2
libprojectM requires C++ compilation and OpenGL development headers. The Rust FFI bindings use `bindgen`. On WSL2, ensure `libgl1-mesa-dev`, `libglew-dev`, and cmake are installed before attempting a build.

### Pitfall 5: wgpu Vulkan Path Instability on WSL2
wgpu prefers Vulkan on Linux. WSL2's Vulkan support is via D3D12 translation (`dzn`) which is marked non-conformant and has known driver issues. Force wgpu to use OpenGL ES backend on WSL2: set `WGPU_BACKEND=gl` or use `wgpu::Backends::GL`.

---

## Recommendation

| Goal | Recommendation | Confidence |
|------|----------------|-----------|
| Phase 8 TUI dashboard (VU, timeline, transport) | ratatui (as already planned) | HIGH |
| Sound IDE with text editing (future phase) | GPUI — existing spike, larger widget ecosystem | MEDIUM |
| MilkDrop visualizer (future phase) | Option A: standalone wgpu window with WGSL shaders | MEDIUM |
| Full MilkDrop preset compatibility (.milk files) | Option C: projectM FFI | MEDIUM |
| Single-framework creative coding UI | Option B: Makepad for everything | MEDIUM |

For HUM's current roadmap, the MilkDrop visualizer is not yet in scope (v2.0 is TUI-only). When it becomes active:
- If the team wants to write custom shaders from scratch: **wgpu standalone window** (most control, pure Rust, WGSL)
- If the team wants MilkDrop preset library compatibility: **projectM FFI**
- If the team wants live-reload shader coding and a unified framework: **Makepad**

Do not attempt to build the shader visualizer inside GPUI — it is architecturally unsupported.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| MilkDrop preset parsing | Custom .milk parser | projectM FFI | 500+ community presets, beat detection, all preset opcodes |
| Cross-platform shader compilation | Manual GLSL/HLSL/Metal variants | Makepad DSL OR wgpu WGSL | Makepad DSL cross-compiles; wgpu WGSL compiles to Vulkan/Metal/DX12 |
| FFT computation | Custom DFT | `rustfft` crate | Highly optimized, no unsafe, O(n log n) |
| Audio capture | Raw ALSA/CoreAudio bindings | `cpal` crate | Cross-platform, works on WSL2 via PipeWire/PulseAudio |

---

## Open Questions

1. **Does Makepad's shader DSL support feedback textures (previous frame as sampler)?**
   - What we know: Makepad renders all primitives as GPU quads with custom fragment programs; uniform buffers and textures are supported
   - What's unclear: Whether the DSL exposes a framebuffer-as-texture binding for MilkDrop-style feedback loops
   - Recommendation: Check `makepad/makepad/examples/` for texture sampler usage before committing

2. **Does Makepad have a text editor component suitable for .hum file editing?**
   - What we know: Makepad Studio is a full code editor built with Makepad; the text editing widget must exist
   - What's unclear: Whether `TextInput` or a `CodeEditor` widget is exposed in the public widget library
   - Recommendation: Check `makepad-widgets` docs.rs page for `TextEditor` or `CodeEditor` widget before planning IDE phase

3. **Is wgpu OpenGL ES backend reliable enough on WSL2 for 60fps visualizer?**
   - What we know: Mesa provides OpenGL ES on WSL2; wgpu has a GLES backend; Makepad uses this path
   - What's unclear: Performance ceiling and frame pacing on WSLg with Mesa software fallback
   - Recommendation: Spike a wgpu `WGPU_BACKEND=gl` triangle on WSL2 before committing to wgpu visualizer path

---

## Sources

### Primary (HIGH confidence)
- https://crates.io/crates/makepad-widgets/versions — Makepad 1.0.0 release date 2025-05-13 confirmed
- https://github.com/makepad/makepad — Official README: Linux OpenGL backend, WSL2 deps in `linux_deps.sh`
- https://makepad.rs/guide/start/introduction — Official Makepad Book: shader-based rendering, Rust DSL
- https://github.com/zed-industries/zed/discussions/45996 — GPUI: confirmed no custom GPU render surface
- https://crates.io/crates/projectm — projectM Rust crate confirmed on crates.io
- https://github.com/projectM-visualizer/projectm-rs — Official projectM Rust bindings (FFI + safe wrapper)

### Secondary (MEDIUM confidence)
- https://news.ycombinator.com/item?id=43971829 — Makepad 1.0 HN thread: community reception, shader architecture discussion
- https://deepwiki.com/makepad/makepad — Makepad architecture overview: shader DSL cross-compilation
- https://wgpu.rs/ — wgpu official site: WGSL cross-platform shader support confirmed
- https://gitnation.com/contents/makepad-leveraging-rust-wasm-webgl-to-build-amazing-cross-platform-applications — Rik Arends: "all rendering on the GPU, shader-based UI"

### Tertiary (LOW confidence — not independently verified)
- Makepad feedback texture support for MilkDrop: inferred from shader architecture, not confirmed in docs
- wgpu GLES backend 60fps on WSL2: no benchmark data found
- Makepad terminal emulator: no evidence found (absence of evidence, not evidence of absence)

---

## Metadata

**Confidence breakdown:**
- Makepad availability + WSL2: HIGH — crates.io + official README explicit
- Makepad shader system: MEDIUM — architecture confirmed, specific MilkDrop features unverified
- GPUI custom shader gap: HIGH — confirmed by official zed-industries discussion
- projectM Rust bindings: HIGH — crates.io confirmed, official org repo
- Architecture options: MEDIUM — based on verified framework properties, not spiked

**Research date:** 2026-03-22
**Valid until:** 2026-06-22 (Makepad 1.0 is stable; wgpu moves fast, re-check backend compatibility)

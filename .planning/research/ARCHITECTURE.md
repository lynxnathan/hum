# Architecture Research

**Domain:** Real-time spatial audio canvas (Makepad + cpal + fundsp, Rust)
**Researched:** 2026-03-27
**Confidence:** HIGH (cpal/fundsp confirmed from official docs; Makepad threading confirmed from robius integration docs 2026)

## Standard Architecture

### System Overview

```
┌──────────────────────────────────────────────────────────────────┐
│                         MAIN THREAD                              │
│                    (Makepad event loop)                          │
│  ┌─────────────┐   ┌──────────────┐   ┌─────────────────────┐  │
│  │   app.rs    │   │   nodes.rs   │   │    spatial.rs       │  │
│  │  App struct │──▶│  NodeState   │──▶│  pan/blend compute  │  │
│  │  handle_    │   │  x, y, freq  │   │  → writes Shared    │  │
│  │  event()    │   └──────────────┘   └─────────────────────┘  │
│  └─────────────┘                                                 │
│         │                                                        │
│         │ Arc<fundsp::Shared> handles (pan_a, pan_b, blend)     │
│         │ (Shared wraps AtomicF32 internally)                    │
│         ▼                                                        │
├──────────────────────────────────────────────────────────────────┤
│           SHARED STATE (lock-free, crosses thread boundary)      │
│  Shared: node_a_pan  ·  node_b_pan  ·  proximity_blend          │
├──────────────────────────────────────────────────────────────────┤
│                      AUDIO THREAD                                │
│            (cpal WASAPI callback, high-priority)                 │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                     audio.rs                            │    │
│  │  build_stream() callback — reads Shared, ticks graph    │    │
│  │                                                         │    │
│  │  fundsp graph:                                          │    │
│  │    sine(440) ──▶ var(pan_a) ──▶ panner ──▶ ─┐          │    │
│  │                                              ├──▶ [L,R] │    │
│  │    sine(660) ──▶ var(pan_b) ──▶ panner ──▶ ─┘          │    │
│  │                (+ operator sums stereo pairs)           │    │
│  └─────────────────────────────────────────────────────────┘    │
│         │                                                        │
│         ▼                                                        │
│    Windows WASAPI → speakers                                     │
└──────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Notes |
|-----------|----------------|-------|
| `app.rs` | App struct, Makepad event loop, mouse drag dispatch, canvas draw | Owns `NodeState` and `Arc<AudioParams>`; holds `cpal::Stream` alive |
| `nodes.rs` | `NodeState` — canvas position (x, y), frequency, radius | Pure data struct; no audio or UI framework knowledge |
| `spatial.rs` | Maps node positions → pan + proximity blend parameter values | Pure math; no side effects; unit-testable in isolation |
| `audio.rs` | cpal stream bootstrap, fundsp graph construction, audio callback | Imports only cpal + fundsp; never imports makepad |

## Recommended Project Structure

```
src/bin/ghostinstrument/
├── main.rs           # entry point, calls app::app_main()
├── app.rs            # App struct, handle_event, draw_walk, startup wiring
├── nodes.rs          # NodeState { x, y, freq, radius } for each node
├── spatial.rs        # recompute(node_state, canvas_size) -> (pan_a, pan_b, blend)
└── audio.rs          # build_params() + build_stream() + fundsp graph
```

### Structure Rationale

- **nodes.rs separate from app.rs:** Node state is plain data. Keeping it isolated makes it passable to spatial functions without pulling in Makepad types.
- **spatial.rs separate from nodes.rs:** The position→parameter mapping is a pure function. Can be unit-tested without a canvas, audio device, or running app.
- **audio.rs fully isolated from Makepad:** Never imports `makepad_widgets`. Takes only `Arc<AudioParams>` at construction. Can be smoke-tested without UI.
- **No mixer.rs or separate mixer thread:** For two static oscillators the fundsp graph IS the mixer. The `+` operator sums stereo output pairs inside the callback. A separate thread adds latency and complexity for zero gain.

## Architectural Patterns

### Pattern 1: fundsp Shared Variables for Cross-Thread Parameters

**What:** `fundsp::Shared` wraps an `Arc<AtomicF32>` internally. Create one per controllable parameter. Pass a clone into the audio graph via `var(&shared)`. The UI thread calls `shared.set_value(f)`, which is an atomic store. The audio graph reads it on each sample with no locking.

**When to use:** Any scalar parameter (pan, gain, frequency, blend) that the UI writes and the audio callback reads.

**Trade-offs:** Zero allocations and zero blocking in the audio path. Reads lag behind writes by at most one callback buffer (~2-5ms at 48kHz with 128-256 sample buffers). This lag is imperceptible for spatial panning controlled by mouse drags.

**Example:**
```rust
// audio.rs
use fundsp::hacker32::*;

pub struct AudioParams {
    pub pan_a: Shared,
    pub pan_b: Shared,
}

pub fn build_params() -> AudioParams {
    AudioParams {
        pan_a: Shared::new(0.0),  // center
        pan_b: Shared::new(0.0),
    }
}

pub fn build_graph(params: &AudioParams) -> Box<dyn AudioUnit> {
    // var(&shared) reads the atomic on each sample tick
    let graph_a = sine_hz(440.0) >> panner(var(&params.pan_a));
    let graph_b = sine_hz(660.0) >> panner(var(&params.pan_b));
    Box::new((graph_a + graph_b) * 0.5)
}

// spatial.rs — UI thread, called after mouse drag
pub fn update_params(node_state: &NodeState, canvas_w: f32, params: &AudioParams) {
    // x in [0, canvas_w] → pan in [-1.0, 1.0]
    let pan_a = (node_state.node_a.x / canvas_w) * 2.0 - 1.0;
    let pan_b = (node_state.node_b.x / canvas_w) * 2.0 - 1.0;
    params.pan_a.set_value(pan_a);  // AtomicF32 store, no lock
    params.pan_b.set_value(pan_b);
}
```

### Pattern 2: cpal Stream Held in App Struct, Never Moved

**What:** `cpal::build_output_stream()` returns a `Stream`. The stream is silent until `stream.play()` is called. The stream must not be dropped — dropping it stops audio. Hold it in the `App` struct with a leading underscore name to communicate "owned for lifetime only".

**When to use:** Single startup in `App`'s `LiveHook::after_new_from_doc()` or an equivalent init path.

**Trade-offs:** `cpal::Stream` is `!Send` on WASAPI. Keep it on the main thread inside `App`. The audio callback itself runs on cpal's internal high-priority thread — you never touch that thread directly.

**Example:**
```rust
// app.rs
pub struct App {
    #[live] ui: WidgetRef,
    _stream: cpal::Stream,       // held alive; never accessed after init
    audio_params: Arc<AudioParams>,
    nodes: NodeState,
}
```

### Pattern 3: No Mutex in the Audio Callback

**What:** The cpal audio callback is a `FnMut(&mut [f32], &OutputCallbackInfo)` called on a high-priority OS thread. Any call that can block (Mutex::lock, heap allocation, println!, file I/O) risks a buffer underrun (xrun = audible glitch).

**When to use:** Always. The audio callback must contain zero blocking operations.

**Trade-offs:** For v5.0, the constraint is easily satisfied — read two `Shared` values, tick the fundsp graph, write to the output buffer. The fundsp graph itself is pre-allocated at construction. If future milestones require dynamic graph changes (adding/removing nodes at runtime), use `ringbuf` crate (lock-free SPSC ring buffer) to send graph-swap commands from UI to audio thread without locking.

## Data Flow

### Primary: Mouse Drag → Spatial Parameter → Audio Output

```
MouseMove event (Makepad, main thread)
    │
    ▼
App::handle_event()
    │ mutates
    ▼
NodeState::set_position(node_id, x, y)
    │ calls
    ▼
spatial::recompute(&nodes, canvas_w) → (pan_a, pan_b)
    │ calls
    ▼
AudioParams::pan_a.set_value(v)     ← AtomicF32 store, Relaxed ordering
AudioParams::pan_b.set_value(v)
    │
    │ (shared memory, no synchronization barrier cost)
    ▼
cpal callback (audio thread)
  graph.process(buffer)
    ↑ var(&pan_a) reads atomic → panner adjusts L/R mix
    │
    ▼
WASAPI driver → Windows speakers
```

Total latency: Makepad frame time (~16ms) + cpal buffer (~3ms) ≈ 19ms, within the 20ms ceiling.

### Draw Path (independent of audio)

```
NodeState position changes
    │
App calls cx.request_draw()
    │
Makepad schedules repaint
    │
App::draw_walk() reads NodeState positions
    │
draw_quad() circles at (x, y) on dark canvas
    │
Makepad GPU pass → screen
```

The draw path never touches the audio thread or audio params.

## Thread Safety Strategy

The UI↔Audio boundary uses **one-directional atomic writes only**.

| Direction | Mechanism | Ordering | Why |
|-----------|-----------|----------|-----|
| UI → Audio | `fundsp::Shared::set_value()` | Relaxed | Audio reads slightly stale — imperceptible. No barrier needed. |
| Audio → UI | Not required for v5.0 | — | No VU metering or audio-driven UI feedback in this milestone |

**Do not use `Arc<Mutex<T>>` across this boundary.** Makepad's repaint can hold internal locks during GPU submission. If the audio callback tries to acquire the same mutex while the UI holds it during a repaint, the callback blocks and the buffer underruns.

**Future milestone (VU metering / audio → UI feedback):** Use `ringbuf` crate. Audio thread pushes peak amplitude samples into the producer end (lock-free). Makepad's `NextFrame` event polls the consumer end on each draw frame. The `ringbuf` crate provides a lock-free SPSC queue that is safe from real-time audio threads.

**Makepad wakeup from background threads:** If a future milestone needs to push data from the audio thread to trigger a UI repaint (not needed at v5.0), the correct mechanism is `SignalToUI::set_ui_signal()` followed by `Cx::post_action()`. This is the pattern used by robius-matrix-integration and confirmed in Makepad's architecture: `&mut Cx` is created only on the main thread and passed as a mutable reference to all event handlers and draw routines.

## Build Order

Components have clear dependency chains. Build in this order to always have something runnable and testable at each step.

```
Step 1 — NodeState (nodes.rs)
    Plain data struct. No deps.
    Test: construct NodeState, set positions, read them back.

Step 2 — AudioParams + audio::build_params() (audio.rs, struct only)
    Defines Shared handles. Depends on: fundsp only.
    Test: create params, set_value, read value back.

Step 3 — spatial::recompute() (spatial.rs)
    Pure function: NodeState + canvas size → (pan_a, pan_b).
    Test: unit test pan values at left/center/right edge.
    Depends on: NodeState, AudioParams.

Step 4 — audio::build_graph() + audio::build_stream() (audio.rs, full)
    Constructs fundsp graph, opens cpal device, starts stream.
    Test: run for 2s without xruns, hear two sine tones.
    Depends on: AudioParams, cpal, fundsp.

Step 5 — App wiring (app.rs)
    Connect handle_event → NodeState → spatial → AudioParams.
    Hold Stream alive. Call cx.request_draw() after position update.
    Test: drag nodes, hear pan change.
    Depends on: all above + Makepad.

Step 6 — Canvas draw (app.rs, draw_walk)
    Draw circles at node positions on dark canvas.
    No new deps — reads NodeState already in App.
    Test: visual confirmation of node positions matching audio panning.
```

Audio (Step 4) can be built and heard before Makepad visuals exist. Spatial math (Step 3) can be unit-tested before either audio or UI exist.

## Anti-Patterns

### Anti-Pattern 1: Mutex in the Audio Callback

**What people do:** `let state = self.shared.lock().unwrap();` inside the cpal callback to read node positions.

**Why it's wrong:** The Makepad repaint can hold OS-level graphics resources or internal mutexes during GPU submission. If the audio callback needs the same mutex, it blocks. A 256-sample WASAPI buffer at 48kHz gives ~5ms before an xrun. This is easily exceeded by a single frame's worth of GPU work.

**Do this instead:** Pre-compute all audio parameters on the UI thread using `spatial::recompute()`, then write them into `Shared` atomics. The audio callback reads atomics only — no lock, no block.

### Anti-Pattern 2: Rebuilding the fundsp Graph on Parameter Changes

**What people do:** Drop the graph and call `build_graph()` again every time a node moves.

**Why it's wrong:** Graph construction allocates. Allocation in the audio callback is forbidden. Even if you try to swap graphs between callbacks, the old graph drops on the audio thread (dealloc is not real-time safe).

**Do this instead:** Build the graph once at startup with `var(&shared)` nodes. Parameter changes only update the atomic value. The graph topology is fixed for the lifetime of a session at v5.0 scope.

### Anti-Pattern 3: Spawning a Separate Mixer Thread

**What people do:** Audio callback → ring buffer → mixer thread → master output thread. Three threads for two oscillators.

**Why it's wrong:** Each thread hop adds at least one callback buffer of latency. For two static oscillators with atomic pan/gain, the fundsp `+` operator inside the single cpal callback already IS the mixer with zero additional latency.

**Do this instead:** Single cpal callback owns the entire fundsp graph. All mixing and panning is internal to the graph. No additional threads.

### Anti-Pattern 4: Reading NodeState Directly in the Audio Callback

**What people do:** `Arc<Mutex<NodeState>>` shared between UI and audio; callback reads `x, y` directly.

**Why it's wrong:** Same as Anti-Pattern 1. Additionally, `NodeState` contains canvas coordinates — the audio callback should not know about canvas size or coordinate systems. That mapping belongs in `spatial.rs`, on the UI thread.

**Do this instead:** `spatial.rs` converts coordinates to audio parameters (pan range [-1,1]) on the UI thread. The audio callback sees only pre-converted audio-domain values.

## Integration Points

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| App → NodeState | Direct field mutation (same thread) | NodeState owned by App |
| NodeState → spatial::recompute | Function call, pure return value | No side effects |
| spatial::recompute → AudioParams | `Shared::set_value()` (AtomicF32 store) | Only place crossing the thread boundary |
| AudioParams → fundsp graph | `var(&shared)` reads atomic per sample | Baked in at graph construction time |
| cpal → WASAPI | cpal internal | Abstracted; ghostinstrument never calls WASAPI directly |

### External Dependencies

| Library | Version | Integration Point |
|---------|---------|-------------------|
| `cpal` | latest | `build_output_stream()` in audio.rs |
| `fundsp` | 0.23.0 | graph construction + `Shared` param vars |
| `makepad-widgets` | 1.0 | App struct, event loop, canvas draw |

## Sources

- cpal official docs (docs.rs/cpal/latest) — "dedicated, high-priority thread" for audio callback. HIGH confidence.
- fundsp GitHub README (github.com/SamiPerttu/fundsp) — "Shared variables can be cloned and sent into another thread. Use var and var_fn opcodes." HIGH confidence.
- fundsp hacker module docs (docs.rs/fundsp/latest/fundsp/hacker) — confirms `panner` opcode: "Fixed equal power mono-to-stereo panner with pan value in -1…1". HIGH confidence.
- robius-matrix-integration / Makepad skill docs (lobehub.com, 2026-02-17) — confirms `SignalToUI::set_ui_signal()` + `Cx::post_action` pattern. MEDIUM confidence.
- Robius roadmap blog (robius.rs/blog/robius-roadmap-2025) — confirms `&mut Cx` is main-thread-only token for Makepad. HIGH confidence.
- ringbuf crate (docs.rs/ringbuf) — lock-free SPSC FIFO, appropriate for future audio→UI metering path. HIGH confidence.

---
*Architecture research for: ghostinstrument v5.0 — spatial audio canvas (Makepad + cpal + fundsp)*
*Researched: 2026-03-27*

# Pitfalls Research

**Domain:** Rust real-time audio — cpal + fundsp + Makepad + Windows cross-compilation from WSL2
**Researched:** 2026-03-27
**Confidence:** HIGH for cpal/WASAPI pitfalls (well-documented in cpal issues and community); HIGH for fundsp real-time constraints (from fundsp docs and Rust audio community); MEDIUM for cargo-xwin + cpal cross-compile (less documented, inference from known C-dep patterns); HIGH for Makepad threading model (from Makepad source and prior hum project experience)

---

## Critical Pitfalls

### Pitfall 1: WASAPI Default Mode Negotiates Sample Rate — Your fundsp Graph Must Match

**What goes wrong:**
cpal on Windows in shared mode (the default) does NOT use your requested sample rate. WASAPI negotiates the sample rate with the audio engine, which is set by the Windows sound control panel (typically 44100 Hz or 48000 Hz per device). If your fundsp graph is constructed at 44100 Hz but the WASAPI stream runs at 48000 Hz, you get a 9% pitch shift on all oscillators and incorrect filter cutoffs. No error is raised — cpal silently opens the stream at the negotiated rate.

**Why it happens:**
Developers hardcode `AudioUnit::new()` or build the fundsp graph with a constant before calling `cpal::default_output_device()`. The actual sample rate is only known after `device.default_output_config()` or after the stream is opened with a specific config. fundsp's `AudioUnit` encodes sample rate at construction time into its internal state.

**How to avoid:**
1. Call `device.default_output_config()` first to get the negotiated sample rate.
2. Pass that rate to fundsp graph construction: `let graph = sine() >> pan(0.0); graph.reset(Some(sample_rate as f64));`
3. Alternatively, use `AudioUnit::reset()` after opening the stream if graph was built with a default rate.
4. Never hardcode `44100.0f32` anywhere in the DSP path.

**Warning signs:**
- Oscillators play back at slightly wrong pitch (sharp or flat by a semitone or more)
- Filter resonant frequencies are off
- Works correctly on one machine but wrong on another (different device default rates)

**Phase to address:**
cpal stream initialization phase — before any fundsp graph is constructed.

---

### Pitfall 2: Allocating in the Audio Callback Causes Glitches and Eventual Deadlock

**What goes wrong:**
The cpal audio callback runs on a high-priority real-time thread. Any heap allocation inside the callback (Vec, Box, String, Arc clone that triggers a dealloc, format!, collect(), push) can cause the OS memory allocator to block while another thread holds the allocator lock. On Windows with WASAPI, the system audio thread has a strict deadline; missing it causes buffer underruns (clicks, dropouts). If the allocator deadlocks, the audio thread starves and the system audio engine may terminate the stream entirely.

**Why it happens:**
Rust's ergonomic ownership model makes allocation invisible. `let v: Vec<f32> = (0..n).map(f).collect()` looks like a stack operation but allocates. `format!("{}", x)` allocates. Even `Arc::clone` is allocation-free but `Arc::drop` (when refcount hits zero) calls the allocator. fundsp's `AudioUnit::process()` is allocation-free by design but you can accidentally allocate in the surrounding callback code.

**How to avoid:**
- Pre-allocate all buffers before the stream starts. Pass them into the closure via `move` capture.
- Use fixed-size arrays or pre-allocated `Vec` with known capacity.
- Never use `format!`, `String::new()`, `Vec::push`, or `collect()` inside the callback.
- For sending node positions from UI to audio: use `crossbeam_channel::bounded` (lock-free) or `std::sync::atomic` values (AtomicF32 via bit-casting). Never use `Mutex` in the callback.
- Validate with `cargo-flamegraph` or `heaptrack` — allocation in callback shows as short spikes in the audio thread.

**Warning signs:**
- Periodic clicks or dropouts under load (not just startup)
- `cargo clippy` passes but audio glitches appear when the UI is active
- Latency spikes visible in Windows Task Manager audio graphs

**Phase to address:**
cpal stream + fundsp integration phase — the callback structure must be correct from the first working oscillator.

---

### Pitfall 3: Panning Parameter Updates Cause Zipper Noise (Staircase Artifacts)

**What goes wrong:**
When a node is dragged, the X position updates at the UI frame rate (~60 Hz). If the pan value is applied directly to fundsp's `pan()` node by setting a parameter once per frame, the panning coefficient jumps in discrete steps 60 times per second. At 48000 Hz sample rate, each step lasts 800 samples — audible as a scratching/zipper noise on the stereo image. This is especially pronounced for nodes near center (where panning sensitivity is highest).

**Why it happens:**
UI events and audio callbacks run at completely different rates. A 60 Hz UI update setting a single coefficient means the audio signal sees a step function, not a smooth ramp. The human ear can detect discontinuities above ~20 Hz as tonal artifacts.

**How to avoid:**
Use **parameter smoothing** (a one-pole lowpass on the coefficient) inside the audio callback:
```rust
// In callback state, per-node:
struct NodeState {
    pan_target: AtomicF32,   // written by UI thread
    pan_current: f32,        // read/written only in audio thread
    pan_smooth: f32,         // smoothing coefficient, e.g. 0.995
}

// In callback, per sample:
let target = node.pan_target.load(Relaxed);
node.pan_current = node.pan_current * node.pan_smooth + target * (1.0 - node.pan_smooth);
```
A smoothing coefficient of 0.995 at 48kHz gives ~10ms smoothing — inaudible latency, no zipper noise.

Alternatively, use fundsp's `shared()` + `var()` mechanism which does not smooth, so manual smoothing is still needed.

**Warning signs:**
- Dragging a node produces a scratchy/crunchy sound on the stereo image
- Static position sounds clean, but motion sounds rough
- Artifact frequency matches frame rate (60 Hz or its harmonics)

**Phase to address:**
Spatial panning implementation — before the first mouse drag is wired to audio.

---

### Pitfall 4: cpal Does Not Cross-Compile Without WASAPI Headers — cargo-xwin Must Be Configured Correctly

**What goes wrong:**
cpal depends on `windows-sys` (or `winapi`) to call WASAPI. When cross-compiling from WSL2 to `x86_64-pc-windows-msvc`, the linker needs Windows SDK headers and import libraries. If cargo-xwin is not correctly configured to provide the Windows SDK sysroot, compilation fails with errors like `error: linking with 'rust-lld'` or missing `mmdevapi.lib` / `ole32.lib`. Even if it compiles, if the wrong Windows SDK version is used, WASAPI API surface mismatches cause link errors.

**Why it happens:**
cpal's `windows` feature pulls in `windows-sys` crate which generates bindings to WASAPI COM interfaces. These require the full Windows SDK, not just the MSVC compiler. cargo-xwin downloads the SDK automatically, but the download can fail silently or use a cached stale version.

**How to avoid:**
1. Verify `cargo-xwin` is installed and `xwin` cache is populated: `cargo xwin build --target x86_64-pc-windows-msvc` should trigger SDK download on first run.
2. Add cpal to `Cargo.toml` with explicit feature flags: `cpal = { version = "0.15", features = ["wasapi"] }` — do not rely on auto-detection.
3. Confirm `XWIN_ARCH=x86_64` and `XWIN_VERSION` environment variables are set if non-default SDK version is needed.
4. Test compilation with a minimal cpal program (device enumeration only) before integrating fundsp — isolates the linker issue from DSP logic.

**Warning signs:**
- Link errors mentioning `mmdevapi`, `ole32`, `propsys`, `combase`
- Build succeeds on a native Windows machine but fails in WSL2 cross-compile
- cargo-xwin hangs on first run (SDK download timeout — retry with `XWIN_CACHE_DIR` set to a reliable path)

**Phase to address:**
Cross-compilation setup phase — must be validated before any audio code is written.

---

### Pitfall 5: fundsp Graph Is Not `Send` By Default — cpal Requires `Send` on the Callback

**What goes wrong:**
cpal's `Stream::build_output_stream` requires the callback closure to be `Send` (it will be moved to a different thread). fundsp's `AudioUnit` trait object (`Box<dyn AudioUnit>`) is `Send` only if all internal nodes are `Send`. Most fundsp primitive nodes are `Send`, but any node that holds a `Shared<T>` variable (used for parameter sharing) wraps `Arc<Mutex<T>>` which is `Send`. However, a bare `Box<dyn AudioUnit>` without explicit `+ Send` bound will fail to compile.

**Why it happens:**
The fundsp graph building DSL returns concrete types like `An<Sine<f32>>` which implement `AudioUnit`. When you box them to `Box<dyn AudioUnit>`, the `+ Send` marker is lost. The compiler error is accurate but cryptic: `the trait Send is not implemented for dyn AudioUnit`.

**How to avoid:**
Box as `Box<dyn AudioUnit + Send>` explicitly:
```rust
let graph: Box<dyn AudioUnit + Send> = Box::new(sine() * constant(440.0) >> pan(0.0));
```
Or keep graphs as concrete types in the callback closure (avoids boxing entirely — preferred for simple graphs with only two oscillators).

**Warning signs:**
- Compiler error: `the trait Send is not implemented for dyn AudioUnit`
- Error appears when passing the audio closure to `build_output_stream`, not when building the graph

**Phase to address:**
cpal + fundsp integration — first oscillator phase.

---

### Pitfall 6: Blocking the Makepad UI Thread With Audio Initialization

**What goes wrong:**
Makepad's event loop runs on the main thread. `cpal::default_output_device()`, `device.default_output_config()`, and `device.build_output_stream()` can each block for hundreds of milliseconds on Windows (WASAPI initialization involves COM object creation and device enumeration). If these are called in `App::new()` or inside a Makepad `handle_event()` call, the UI thread blocks, causing the window to be unresponsive or the OS to show a "not responding" state during startup.

**Why it happens:**
The natural place to initialize audio is during application startup. WASAPI on Windows is slower to initialize than ALSA on Linux — first-time COM initialization can take 200-500ms. Makepad does not protect against blocking calls in the event loop.

**How to avoid:**
Initialize cpal on a **dedicated thread** spawned at startup, before or alongside Makepad's event loop:
```rust
let (tx, rx) = crossbeam_channel::bounded(1);
std::thread::spawn(move || {
    let host = cpal::default_host();
    let device = host.default_output_device().unwrap();
    let config = device.default_output_config().unwrap();
    tx.send((device, config)).unwrap();
});
// Makepad event loop starts immediately, audio arrives async
```
Use a `crossbeam_channel` or `std::sync::OnceLock` to pass the initialized device/stream back to the main thread context after Makepad is running.

**Warning signs:**
- Window appears but is frozen for 0.5-1s on startup (Windows "not responding" briefly)
- Works fine on fast machines but freezes on slower Windows systems
- The freeze duration matches `cpal::default_host()` call time (measurable with `Instant::now()`)

**Phase to address:**
Audio + UI integration phase — the threading model must be right from the first wired oscillator.

---

### Pitfall 7: Proximity Blending Clicks on Node Collision (Discontinuous Wet/Dry Jump)

**What goes wrong:**
If proximity blending is computed as a step function at a distance threshold (e.g., "if distance < 100px, blend = 1.0, else blend = 0.0"), nodes crossing the threshold boundary cause an instantaneous coefficient change from 0.0 to 1.0. This is heard as a click or pop — a single-sample discontinuity at audio rate. The effect is most audible when nodes are near the threshold and the user is moving them slowly.

**Why it happens:**
Threshold logic is natural from a UI design perspective ("nodes in proximity = connected"). Translating this to audio requires the same parameter-smoothing discipline as panning (Pitfall 3), but the proximity threshold makes it worse: it's a binary flip rather than a gradient.

**How to avoid:**
Use a **soft curve** for proximity blending, not a threshold:
```rust
// Map distance [0, max_dist] to blend [1.0, 0.0] with a cosine or exponential curve
let blend = (1.0 - (distance / max_distance).min(1.0)).powi(2);
```
Apply the same one-pole smoothing to `blend_target → blend_current` as used for panning (Pitfall 3). The smoothing ensures the wet/dry crossfade is audio-rate-gradual even when the position update is at 60 Hz.

**Warning signs:**
- A click or pop audible when two nodes get close or move apart
- The click is deterministic and reproducible at a specific distance
- Slowing down node movement makes the click quieter but not gone (confirms it's a step, not a sample error)

**Phase to address:**
Proximity blending implementation — before any crossfade logic is wired to distance.

---

### Pitfall 8: Exclusive Mode WASAPI Fails on Most Windows Consumer Setups

**What goes wrong:**
If cpal is configured for exclusive mode (lower latency, direct hardware access), it will fail on most Windows systems where other applications already have the audio device open, or where the device driver does not support exclusive mode. The error is a runtime panic or a `StreamError` — there is no graceful fallback unless explicitly coded.

**Why it happens:**
Exclusive mode requires the audio device to be free (no other app using it). In practice, Windows has system sounds, browser audio, Discord, etc. always holding the device in shared mode. Exclusive mode is also not supported by all drivers (Bluetooth, USB audio class devices often reject it).

**How to avoid:**
Use **shared mode** (cpal default) for v5.0. The latency difference is 5-20ms — well within the 20ms ceiling requirement stated in PROJECT.md. Only pursue exclusive mode if latency measurements show shared mode is consistently over 20ms (unlikely on modern hardware). Document this decision explicitly.

**Warning signs:**
- `StreamError::DeviceNotAvailable` at stream creation
- Works when all other audio is closed, fails when browser is open
- Error message mentions "AUDCLNT_E_DEVICE_IN_USE" in the HRESULT

**Phase to address:**
cpal stream initialization — default to shared mode explicitly, do not leave it implicit.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Hardcode sample rate as 44100 | Simpler graph construction | Wrong pitch on 48kHz devices, inaudible on some systems | Never |
| Set pan value directly without smoothing | Simpler UI→audio bridge | Zipper noise on every drag | Never |
| Initialize cpal on main thread | Simpler startup code | UI freezes on Windows startup (COM overhead) | Never |
| Use `Box<dyn AudioUnit>` without `+ Send` | Typechecks locally | Compile error when moving to cpal thread | Never |
| Proximity threshold as binary step | Simpler to reason about | Audible click at distance boundary | Never |
| Skip cargo-xwin sysroot validation | Faster iteration start | Hours debugging link errors later | Never |
| Use `Mutex<f32>` for UI→audio param passing | Familiar, works in tests | Potential priority inversion / glitch in real-time callback | Never (use AtomicF32) |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| cpal + fundsp | Build fundsp graph before knowing sample rate | Query `device.default_output_config()` first, pass rate to graph |
| cpal + fundsp | Box graph as `Box<dyn AudioUnit>` | Box as `Box<dyn AudioUnit + Send>` |
| cpal + WASAPI | Attempt exclusive mode | Default to shared mode; 20ms latency ceiling is achievable in shared |
| cpal + cargo-xwin | Missing `wasapi` feature flag | Explicitly add `features = ["wasapi"]` in Cargo.toml |
| Makepad + cpal | Call `cpal::default_host()` in event handler | Spawn audio init on separate thread before Makepad loop |
| UI → audio | `Mutex<f32>` for parameter passing | `AtomicU32` with `f32::to_bits()` / `f32::from_bits()` |
| spatial panning | Direct coefficient from UI position | One-pole smoothing filter on all audio-rate parameters |
| proximity blend | Binary threshold at distance | Soft curve (quadratic or cosine) + same smoothing as panning |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Allocation in cpal callback | Periodic clicks, dropout under load | Pre-allocate all buffers, no Vec/String/format! in callback | Under any non-trivial load |
| fundsp graph rebuild per frame | CPU spike every 60 Hz, audio dropout | Build graph once, update parameters via `Shared<T>` or atomics | Immediately |
| Polling AtomicF32 inside sample loop | Extra memory barrier per sample | Read once per callback buffer, not once per sample | At high buffer counts (>512 samples) |
| Frame-rate parameter updates without smoothing | Zipper noise at 60 Hz | Smooth all parameters inside audio callback | Always audible |
| UI thread blocking on audio device enumeration | UI freeze on startup | Async audio init on separate thread | Every startup on Windows |

---

## "Looks Done But Isn't" Checklist

- [ ] **Sample rate:** Confirmed fundsp graph uses the actual negotiated WASAPI sample rate — not a hardcoded constant. Verify by checking `config.sample_rate().0` at runtime and asserting it matches the rate passed to graph construction.
- [ ] **Pan smoothing:** Drag a node rapidly from left to right — no zipper or scratching noise audible. Silence while static, smooth while moving.
- [ ] **Proximity blend:** Move two nodes through the proximity threshold slowly — no click or pop. Crossfade is gradual.
- [ ] **Cross-compile:** `cargo xwin build --target x86_64-pc-windows-msvc --release` succeeds from WSL2 clean environment (fresh xwin cache). Run the binary on Windows, not just a build success.
- [ ] **UI non-blocking:** Window renders and responds to mouse input before first audio is audible. Audio glitch during startup does not affect UI.
- [ ] **Thread safety:** `cargo test` and `cargo check` pass with `--features wasapi` on the cross-compile target.
- [ ] **No Mutex in callback:** Confirm with code review — zero `Mutex::lock()` calls inside any cpal callback closure.
- [ ] **Shared mode confirmed:** Attempt to open audio while a browser tab plays audio — stream opens successfully.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Hardcoded sample rate | LOW | Query config, pass to graph, rebuild |
| No pan smoothing | LOW | Add one-pole filter state to callback struct, 10 lines of code |
| Proximity click | LOW | Replace threshold with soft curve + same smoothing |
| cargo-xwin link errors | MEDIUM | Clear xwin cache (`~/.xwin-cache`), re-run with verbose logging, identify missing SDK component |
| fundsp `Send` bound missing | LOW | Change `Box<dyn AudioUnit>` to `Box<dyn AudioUnit + Send>` throughout |
| UI blocking on audio init | MEDIUM | Refactor startup: spawn audio thread, use channel to signal readiness to UI |
| Exclusive mode failures | LOW | Remove explicit exclusive mode config; shared mode is the default |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Sample rate mismatch | cpal stream init (first audio phase) | Print negotiated sample rate at startup, confirm matches device setting |
| Allocation in callback | cpal + fundsp integration | Play audio while dragging — no dropouts after 60 seconds |
| Zipper noise on pan | Spatial panning phase | Drag node full left→right at 60fps, no audible artifact |
| cargo-xwin WASAPI link errors | Cross-compile validation (before DSP) | `cargo xwin build` succeeds from clean WSL2 environment |
| fundsp `Send` missing | First oscillator phase | `cargo check --target x86_64-pc-windows-msvc` passes |
| Makepad UI blocking | Audio + UI threading phase | Startup time < 200ms to first rendered frame |
| Proximity click | Proximity blending phase | Nodes through threshold at 1px/frame — no audible discontinuity |
| Exclusive mode failure | cpal init phase | Open stream while browser plays audio — succeeds |

---

## Sources

- cpal WASAPI backend: https://docs.rs/cpal/latest/cpal/ — HIGH confidence (official docs, WASAPI shared/exclusive mode documented)
- fundsp AudioUnit trait: https://docs.rs/fundsp/latest/fundsp/audiounit/trait.AudioUnit.html — HIGH confidence (official docs, allocation-free contract stated)
- Real-time audio programming rules (Ross Bencina): http://www.rossbencina.com/code/real-time-audio-programming-101-time-waits-for-nothing — HIGH confidence (canonical reference, widely cited in Rust audio community)
- cpal GitHub issues — WASAPI sample rate negotiation: https://github.com/RustAudio/cpal/issues — MEDIUM confidence (multiple issues confirming negotiated rate behavior)
- cargo-xwin usage with Windows deps: https://github.com/rust-cross/cargo-xwin — MEDIUM confidence (README covers SDK download; WASAPI-specific tested from prior hum project experience)
- fundsp Shared parameter system: https://docs.rs/fundsp/latest/fundsp/shared/index.html — HIGH confidence (official docs)
- Makepad threading model: inferred from Makepad source + hum v3 experience (phase 11 summary) — MEDIUM confidence
- WASAPI latency in shared vs exclusive mode: https://learn.microsoft.com/en-us/windows/win32/coreaudio/wasapi — HIGH confidence (official Microsoft docs)

---
*Pitfalls research for: ghostinstrument v5.0 — cpal + fundsp + Makepad + Windows cross-compilation*
*Researched: 2026-03-27*

---
phase: 14-milkdrop-viz
plan: 3
type: execute
wave: 2
depends_on: [14-PLAN-1]
files_modified:
  - src/bin/gui/beat_detector.rs
  - src/bin/gui/visualizer.rs
  - src/bin/gui/transport_client.rs
autonomous: true
requirements: [VIZ-04, VIZ-05]

must_haves:
  truths:
    - "A loud transient (beat) causes a distinct visual spike — visually different from steady FFT response"
    - "Beat energy decays smoothly after the transient (not a hard on/off)"
    - "Each active thing's amplitude drives a visually distinct element in the visualizer"
    - "A thing playing at high amplitude produces more prominent visual output than a quiet thing"
    - "Silence from all things = minimal/still frame"
  artifacts:
    - path: "src/bin/gui/beat_detector.rs"
      provides: "BeatDetector: onset detection from FFT bins, outputs beat_energy 0..1"
      exports: [BeatDetector, BeatState]
    - path: "src/bin/gui/transport_client.rs"
      provides: "FftState extended with beat_energy: f32 and per_thing_amp: HashMap<String,f32>"
      contains: "beat_energy"
    - path: "src/bin/gui/visualizer.rs"
      provides: "Reads beat_energy + per_thing amplitudes, passes to shader as uniforms"
      contains: "beat_energy"
  key_links:
    - from: "src/bin/gui/beat_detector.rs BeatDetector::update()"
      to: "src/bin/gui/transport_client.rs FftState.beat_energy"
      via: "Called inside start_fft_polling loop after each /b_setn decode"
      pattern: "beat_detector.update"
    - from: "src/bin/gui/transport_client.rs FftState.beat_energy"
      to: "src/bin/gui/visualizer.rs uniform beat"
      via: "FFT_STATE global read in draw_walk"
      pattern: "beat_energy"
    - from: "src/bin/gui/transport_client.rs GuiState.amplitudes"
      to: "src/bin/gui/visualizer.rs per-thing orbit elements"
      via: "Separate THING_AMP_STATE OnceLock or same FFT_STATE extended"
      pattern: "per_thing"
---

<objective>
Add beat detection to the FFT pipeline and route per-thing amplitudes from GuiState
into the visualizer shader. Beat transients produce a distinct visual hit; each active
thing drives a visually unique element.

Purpose: VIZ-04 (beat reactivity) and VIZ-05 (per-thing amplitude ownership).
Output: beat_detector.rs + extended FftState + visualizer shader updates.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/phases/14-milkdrop-viz/14-1-SUMMARY.md

<interfaces>
<!-- From transport_client.rs — extend FftState -->
```rust
pub struct FftState {
    pub bins: [f32; 64],
    // ADD:
    pub beat_energy: f32,      // 0.0..1.0, decays ~200ms after transient
    pub per_thing: Vec<(String, f32)>,  // up to 8 things, amp 0..1
}

// start_fft_polling() will be modified to:
// 1. Call BeatDetector::update(bins) after each /b_setn decode
// 2. Write beat_energy into FftState
// 3. Read per-thing amplitudes from a shared GuiState ref (passed in)
```

<!-- From beat_detector.rs — new module -->
```rust
pub struct BeatDetector {
    // spectral flux onset detection:
    prev_bins: [f32; 64],
    energy_history: [f32; 16],  // circular buffer of frame energies
    history_idx: usize,
    pub beat_energy: f32,       // current beat energy, decays each frame
}

impl BeatDetector {
    pub fn new() -> Self { ... }
    // Call once per FFT frame (~30fps):
    pub fn update(&mut self, bins: &[f32; 64]) -> f32 {
        // 1. Compute spectral flux: sum of positive differences from prev frame
        // 2. Compare flux against rolling mean of energy_history
        // 3. If flux > mean * 1.5: onset detected, set beat_energy = 1.0
        // 4. Else: beat_energy *= 0.85 (decay ~200ms at 30fps)
        // 5. Update history, store prev_bins
        // Return beat_energy
    }
}
```

<!-- From visualizer.rs (Plan 1) — add these uniforms -->
// uniform beat: float    (0..1 — spikes on transient, decays)
// uniform thing0..7: float  (per-thing amplitudes, 0 if inactive)
// In pixel() function, use beat to scale effects:
//   - All presets: multiply animation speed or brightness by (1.0 + beat * 3.0)
//   - Waveform: beat causes the wave to "jump" (vertical scale * (1 + beat*2))
//   - Plasma: beat causes a flash (add beat*0.3 to luminance)
//   - Tunnel: beat causes a ring burst (discrete ring at beat peak)
// Per-thing: render up to 8 small orbiting circles in the visualizer corners/edges,
//   brightness proportional to thing amplitude. Overlay on all presets.
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: BeatDetector + extend FftState + wire into polling</name>
  <files>src/bin/gui/beat_detector.rs, src/bin/gui/transport_client.rs</files>
  <action>
**Part A — Create src/bin/gui/beat_detector.rs:**

Implement spectral flux onset detection:

```rust
pub struct BeatDetector {
    prev_bins: [f32; 64],
    energy_history: [f32; 43],  // ~1.4s at 30fps circular buffer
    history_idx: usize,
    pub beat_energy: f32,
}

impl BeatDetector {
    pub fn new() -> Self {
        BeatDetector {
            prev_bins: [0.0; 64],
            energy_history: [0.0001; 43],
            history_idx: 0,
            beat_energy: 0.0,
        }
    }

    pub fn update(&mut self, bins: &[f32; 64]) -> f32 {
        // Spectral flux: sum of positive bin differences
        let flux: f32 = bins.iter().zip(self.prev_bins.iter())
            .map(|(cur, prev)| (cur - prev).max(0.0))
            .sum();

        // Rolling mean of recent fluxes
        let mean = self.energy_history.iter().sum::<f32>() / self.energy_history.len() as f32;

        // Onset if flux exceeds threshold
        if flux > mean * 1.5 && flux > 0.01 {
            self.beat_energy = 1.0_f32.max(self.beat_energy);
        } else {
            self.beat_energy *= 0.82;  // decay: ~200ms at 30fps
        }

        // Update circular buffer and prev_bins
        self.energy_history[self.history_idx] = flux.max(0.0001);
        self.history_idx = (self.history_idx + 1) % self.energy_history.len();
        self.prev_bins.copy_from_slice(bins);

        self.beat_energy
    }
}
```

**Part B — Extend FftState in transport_client.rs:**

```rust
#[derive(Clone)]
pub struct FftState {
    pub bins: [f32; 64],
    pub beat_energy: f32,                    // ADD
    pub per_thing: [(f32, [u8; 32]); 8],     // ADD: (amplitude, name_bytes) for up to 8 things
}
```

Use a fixed-size array to avoid heap allocation in the hot path. `name_bytes` is a 32-byte
zero-padded UTF-8 name (truncated if longer). Amplitude is 0.0 if slot unused.

Extend `start_fft_polling` signature to also accept `Arc<Mutex<GuiState>>`:
```rust
pub fn start_fft_polling(fft_state: Arc<Mutex<FftState>>, gui_state: Arc<Mutex<GuiState>>)
```

Inside the polling loop, after writing `bins`:
1. Call `beat_detector.update(&bins)` (BeatDetector lives in the polling thread as a local)
2. Write `state.beat_energy = beat_detector.beat_energy`
3. Read `gui_state.lock().ok().map(|g| &g.amplitudes)` and populate `state.per_thing`
   (take first 8 entries by iteration order, write amplitude + name bytes)

Update `app.rs` call site: pass `gui_state.clone()` as second arg to `start_fft_polling`.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo check --bin hum-gui 2>&1 | tail -20</automated>
  </verify>
  <done>
`cargo check` passes. beat_detector.rs exists. FftState has beat_energy and per_thing fields. start_fft_polling takes gui_state parameter.
  </done>
</task>

<task type="auto">
  <name>Task 2: Beat + per-thing uniforms in VisualizerView shader</name>
  <files>src/bin/gui/visualizer.rs</files>
  <action>
Extend VisualizerView in `src/bin/gui/visualizer.rs` to consume beat_energy and
per_thing from the extended FftState:

1. **Read new fields in draw_walk:**
   ```rust
   let (bins, beat_energy, per_thing) = FFT_STATE
       .get()
       .and_then(|fft| fft.lock().ok().map(|s| (s.bins, s.beat_energy, s.per_thing)))
       .unwrap_or(([0.0f32; 64], 0.0, [(0.0, [0u8; 32]); 8]));
   ```

2. **Pass as uniforms** (use Makepad's uniform setter API):
   - `uniform beat: float` — set to `beat_energy`
   - `uniform thing0: float` through `thing7: float` — set to per_thing[i].0

3. **Update shader pixel() branches to react to `beat`:**
   - **Waveform**: vertical scale multiplied by `(1.0 + self.beat * 1.5)`, wave amplitude
     boost on transient. Color shifts toward #cba6f7 (mauve) at beat peak.
   - **Spectrum**: bar brightness multiplied by `(1.0 + self.beat * 2.0)`. All bars pulse
     white briefly on beat.
   - **Plasma**: add `self.beat * 0.4` to luminance output for a flash on transient.
     Animation time runs faster: `effective_time = self.time + self.beat * 0.5`.
   - **Tunnel**: ring zoom speed boosted by beat. At high beat, draw a bright ring burst
     at the center (circle at radius < 0.1 * beat).

4. **Per-thing visual elements (overlay on all presets):**
   Draw up to 8 small indicator dots at evenly-spaced positions along the bottom edge
   of the visualizer. For thing slot i:
   - Position: `x = (i as f32 + 0.5) / 8.0`, `y = 0.04`
   - Radius: `0.015 + thing_amp * 0.025` (grows with amplitude)
   - Color: cycle through Catppuccin accent colors by index (blue, mauve, green, peach,
     red, teal, yellow, flamingo)
   - Alpha: `thing_amp` (invisible when silent, opaque when loud)
   - Use a circle SDF in pixel(): `length(pos - dot_center) < radius`
   - Overlay with `mix(base_color, dot_color, dot_alpha * dot_mask)`

   In the pixel() fn, after computing the base preset color, iterate the 8 thing
   uniform values and composite the dot overlay.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build --bin hum-gui 2>&1 | tail -30</automated>
  </verify>
  <done>
`cargo build --bin hum-gui` succeeds. Visualizer shader reads beat_energy and per_thing uniforms. Beat transients cause distinct visual hits across all 4 presets. Per-thing amplitude dots appear at the bottom edge, scaling with amplitude.
  </done>
</task>

</tasks>

<verification>
- `cargo build --bin hum-gui` passes
- `hum gui` runs — all 4 presets react to beat transients with a visible pulse/flash
- Per-thing amplitude dots visible at bottom of visualizer when things are active
- Silence = still frame; loud beat = visible hit distinct from steady-state FFT
</verification>

<success_criteria>
1. Build passes
2. Beat onset detection fires on transients (spectral flux threshold)
3. Beat energy decay is smooth (~200ms), not a hard toggle
4. Per-thing amplitude dots scale with GuiState.amplitudes
5. All 4 presets show beat reactivity
</success_criteria>

<output>
After completion, create `.planning/phases/14-milkdrop-viz/14-3-SUMMARY.md`
</output>

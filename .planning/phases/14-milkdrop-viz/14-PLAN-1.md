---
phase: 14-milkdrop-viz
plan: 1
type: execute
wave: 1
depends_on: []
files_modified:
  - src/bin/gui/visualizer.rs
  - src/bin/gui/app.rs
autonomous: true
requirements: [VIZ-01, VIZ-02]

must_haves:
  truths:
    - "Silence produces a still/flat frame in the visualizer pane"
    - "A loud transient causes a perceptible visual hit within one frame"
    - "Waveform preset renders FFT as a continuous wave line"
    - "Spectrum preset renders 64 magnitude bars (replacing SpectralView)"
    - "Plasma preset renders animated colour fields driven by bass/mids/highs uniforms"
    - "Tunnel preset renders a receding-ring zoom effect driven by beat energy"
    - "Preset can be switched at runtime from the transport bar without restart"
  artifacts:
    - path: "src/bin/gui/visualizer.rs"
      provides: "VisualizerView widget with 4 presets, Makepad shader DSL"
      exports: [VisualizerView, VisualizerPreset, live_design block]
    - path: "src/bin/gui/app.rs"
      provides: "Preset selector buttons wired to VisualizerView"
      contains: "preset_btn"
  key_links:
    - from: "src/bin/gui/transport_client.rs (FFT_STATE / FftState)"
      to: "src/bin/gui/visualizer.rs"
      via: "Same OnceLock<Arc<Mutex<FftState>>> pattern as spectral_view.rs"
      pattern: "FFT_STATE.get"
    - from: "src/bin/gui/app.rs preset buttons"
      to: "src/bin/gui/visualizer.rs set_preset()"
      via: "handle_event ButtonAction"
      pattern: "set_preset"
---

<objective>
Replace the existing SpectralView with a full VisualizerView widget that renders
four switchable presets (waveform, spectrum, plasma, tunnel) using the Makepad shader
DSL. FFT data from the existing OnceLock pipeline feeds uniforms each frame.

Purpose: Core reactive visualizer driven by live audio FFT.
Output: visualizer.rs widget + preset selector in app.rs transport bar.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md

<interfaces>
<!-- From src/bin/gui/transport_client.rs — use these directly -->
```rust
pub struct FftState {
    pub bins: [f32; 64],  // magnitudes, 0.0..1.0, updated ~30fps
}
// Global: static FFT_STATE: OnceLock<Arc<Mutex<FftState>>>
// Init: pub fn init_fft_state(state: Arc<Mutex<FftState>>)
// (already wired in app.rs, SpectralView consumes it — copy this pattern)

pub struct GuiState {
    pub amplitudes: HashMap<String, f32>, // per-thing amplitudes 0..1
    // ...
}
```

<!-- From src/bin/gui/spectral_view.rs — the pattern to follow -->
```rust
// Use OnceLock global for FFT — DO NOT pass via widget props
// Draw with draw_abs(cx, Rect { pos, size }) per element
// draw_bg: DrawColor is the background; add draw_shader: DrawShader for the viz quad
// Makepad shader DSL lives inside live_design!{} block
// Rust-like syntax, cross-compiles to GLSL on Linux
```

<!-- From src/bin/gui/app.rs layout -->
```
body = <View> { flow: Down
  spectral = <SpectralView> { height: 120 }   // REPLACE with VisualizerView
  mid_zone = <View> { flow: Right ... }
  transport_bar = <View> { ... }
}
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: VisualizerView widget with 4 shader presets</name>
  <files>src/bin/gui/visualizer.rs</files>
  <action>
Create `src/bin/gui/visualizer.rs` as a new Makepad widget. Key design:

1. **Preset enum** (derives Clone, Copy, PartialEq):
   `pub enum VisualizerPreset { Waveform, Spectrum, Plasma, Tunnel }`

2. **VisualizerView struct** (derives Live, LiveHook, Widget):
   - `draw_bg: DrawColor` — background fill
   - `draw_viz: DrawShader` — fullscreen quad with custom fragment program
   - `walk: Walk`, `layout: Layout`
   - `#[rust] preset: VisualizerPreset`
   - `#[rust] time: f64` — accumulated frame time for animation

3. **live_design! block** — define `DrawViz = {{DrawViz}} {}` with a shader that:
   - Accepts uniforms: `uniform time: float`, `uniform preset: float` (0=waveform, 1=spectrum, 2=plasma, 3=tunnel)
   - Accepts `uniform bass: float`, `uniform mids: float`, `uniform highs: float`, `uniform volume: float`
   - Accepts a texture: `texture spectrum_tex: texture2D` (64-bin FFT as 1D texture row)
   - Fragment fn `pixel()`:
     - Branch on `preset` value (use `if` chains — Makepad DSL has no switch)
     - **Waveform** (preset=0): Sample spectrum_tex at y=0.5 for the bin at pos.x*64, draw a filled wave shape. Color with Catppuccin blue (#89b4fa) at amplitude, dark surface below.
     - **Spectrum** (preset=1): Recreate bar chart look but rendered as a single shader quad. For each pixel column, sample spectrum_tex at that bin, fill if pos.y < magnitude. Same gradient: zero=#313244, mid=#89b4fa, high=#cba6f7.
     - **Plasma** (preset=2): Classic plasma: `sin(pos.x*10+time) + sin(pos.y*10+time*0.7) + sin((pos.x+pos.y)*6+time*1.3)`. Mix Catppuccin green/mauve/blue based on plasma value. Scale animation speed by `bass*3.0+0.5`.
     - **Tunnel** (preset=3): Compute polar coords from center. Rings at `fract(length(uv)*8.0 - time*volume*4.0)`. Tint ring brightness by `mids`. Color with blue→mauve radial gradient.

4. **impl Widget draw_walk**:
   - Get rect, draw background
   - Read FFT bins from `FFT_STATE` global (same pattern as spectral_view.rs)
   - Compute `bass` = mean of bins[0..8], `mids` = mean of bins[8..32], `highs` = mean of bins[32..64], `volume` = max of all bins
   - Update `time` by adding `cx.get_frame_time()` (or track via `std::time::Instant`)
   - Upload texture: create a 64x1 RGBA texture from bins array and set it on draw_viz
   - Set uniforms via `draw_viz.set_uniform(cx, id!(time), &[self.time as f32])` etc.
   - Call `draw_viz.draw_abs(cx, rect)`

5. **pub fn set_preset(&mut self, cx: &mut Cx, preset: VisualizerPreset)**:
   - Set `self.preset` and call `self.redraw(cx)`

6. **pub fn init_fft_state** re-export: add `pub use crate::spectral_view::init_fft_state;` at top so app.rs has one import point.

NOTE: Makepad DSL is NOT raw GLSL. Variables use `self.` prefix for instance fields. `fn pixel() -> vec4` is the entry point. Use `#[live] draw_viz: DrawShader` not `DrawColor` for the shader quad. If `DrawShader` API is uncertain from docs, use `DrawColor` with a custom `pixel()` fn in the live_design block — Makepad's `DrawColor` already has a `pixel()` hook.

Use Catppuccin Mocha palette throughout (matching existing spectral_view.rs constants).
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo check --bin hum-gui 2>&1 | tail -20</automated>
  </verify>
  <done>
`cargo check` passes. visualizer.rs exists with VisualizerView, VisualizerPreset enum, 4 shader branches, and set_preset() method.
  </done>
</task>

<task type="auto">
  <name>Task 2: Wire VisualizerView into app.rs with preset selector</name>
  <files>src/bin/gui/app.rs</files>
  <action>
Modify `src/bin/gui/app.rs` to replace `SpectralView` with `VisualizerView` and add
preset selector buttons in the transport bar.

1. **Module declaration**: Replace `mod spectral_view;` with `mod visualizer;`.
   Add `use crate::visualizer::{VisualizerView, VisualizerPreset};` and keep
   `use crate::visualizer::init_fft_state;`.

2. **live_design! layout changes**:
   - Remove `use crate::spectral_view::SpectralView;`
   - Add `use crate::visualizer::VisualizerView;`
   - Replace `spectral = <SpectralView> { height: 120 }` with:
     ```
     visualizer = <VisualizerView> {
         width: Fill
         height: 220
     }
     ```
   - In `transport_bar`, after the stop button, add 4 preset buttons:
     ```
     preset_wave = <Button> { text: "WAVE" draw_text: { text_style: { font_size: 10.0 } color: (SUBTLE) } }
     preset_spec = <Button> { text: "SPEC" ... }
     preset_plasma = <Button> { text: "PLASMA" ... }
     preset_tunnel = <Button> { text: "TUNNEL" ... }
     ```

3. **App struct**: Replace `spectral: SpectralView` field with `visualizer: VisualizerView`
   (or use `ui: WidgetRef` pattern — check existing app.rs field style).

4. **handle_startup**: Replace `init_fft_state(...)` call — keep it, just ensure it uses the
   re-exported fn from visualizer.rs.

5. **handle_event / handle_actions**: Add match arms for each preset button:
   ```rust
   if self.ui.button(id!(preset_wave)).clicked(actions) {
       self.ui.visualizer(id!(visualizer)).set_preset(cx, VisualizerPreset::Waveform);
   }
   // ... repeat for Spectrum, Plasma, Tunnel
   ```

6. Remove any leftover `spectral_view` references. Keep `vu_meters.rs` and `arrangement_view.rs`
   untouched.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build --bin hum-gui 2>&1 | tail -30</automated>
  </verify>
  <done>
`cargo build --bin hum-gui` succeeds. The GUI shows VisualizerView where SpectralView was. Four preset buttons appear in the transport bar. Switching presets changes the active shader branch.
  </done>
</task>

</tasks>

<verification>
- `cargo build --bin hum-gui` passes with no errors
- `hum gui` opens window with a 220px visualizer pane showing animated content when audio plays
- Four preset buttons in transport bar (WAVE, SPEC, PLASMA, TUNNEL) switch the visualization mode
- Silence = still/minimal frame; audio = visible animation
</verification>

<success_criteria>
1. cargo build passes
2. Visualizer reacts to FFT data within one frame latency
3. All 4 presets render distinct visual patterns
4. Preset switching works at runtime without restart
</success_criteria>

<output>
After completion, create `.planning/phases/14-milkdrop-viz/14-1-SUMMARY.md`
</output>

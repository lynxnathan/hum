use makepad_widgets::*;
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicU8, Ordering};
use crate::transport_client::FftState;

/// Number of frequency bins from FFT analysis.
const NUM_BINS: usize = 64;

/// Catppuccin Mocha palette constants
const COLOR_SURFACE0: [f32; 3] = [0.192, 0.196, 0.267]; // #313244
const COLOR_BLUE: [f32; 3] = [0.537, 0.706, 0.980];     // #89b4fa
const COLOR_MAUVE: [f32; 3] = [0.796, 0.651, 0.969];     // #cba6f7
const COLOR_GREEN: [f32; 3] = [0.651, 0.890, 0.631];     // #a6e3a1
const COLOR_PEACH: [f32; 3] = [0.980, 0.702, 0.529];     // #fab387
const COLOR_RED: [f32; 3] = [0.953, 0.545, 0.659];       // #f38ba8
const COLOR_TEAL: [f32; 3] = [0.580, 0.886, 0.835];      // #94e2d5
const COLOR_YELLOW: [f32; 3] = [0.976, 0.886, 0.686];    // #f9e2af
const COLOR_FLAMINGO: [f32; 3] = [0.949, 0.604, 0.643];  // #f2cdcd

/// Per-thing accent color palette (Catppuccin accents, cycled by index).
const THING_COLORS: [[f32; 3]; 8] = [
    COLOR_BLUE, COLOR_MAUVE, COLOR_GREEN, COLOR_PEACH,
    COLOR_RED, COLOR_TEAL, COLOR_YELLOW, COLOR_FLAMINGO,
];

/// Bar gap for spectrum preset (pixels).
const BAR_GAP: f64 = 1.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum VisualizerPreset {
    Waveform,
    Spectrum,
    Plasma,
    Tunnel,
    Custom,
}

impl Default for VisualizerPreset {
    fn default() -> Self {
        Self::Spectrum
    }
}

/// Global FFT state reference (same OnceLock as spectral_view).
fn fft_state() -> Option<&'static Arc<Mutex<FftState>>> {
    // Re-use the spectral_view's FFT_STATE — we access it through the same
    // init_fft_state function. But we need our own reference too.
    FFT_STATE_VIZ.get()
}

static FFT_STATE_VIZ: OnceLock<Arc<Mutex<FftState>>> = OnceLock::new();

/// Global preset selection — written by app.rs button handlers, read by VisualizerView each frame.
/// 0=Waveform, 1=Spectrum, 2=Plasma, 3=Tunnel, 4=Custom
static ACTIVE_PRESET: AtomicU8 = AtomicU8::new(1); // default: Spectrum

/// Initialize the visualizer's FFT state reference. Called from app.rs startup.
pub fn init_viz_fft_state(state: Arc<Mutex<FftState>>) {
    FFT_STATE_VIZ.set(state).ok();
}

/// Set the active preset from outside the widget (called by app.rs button handlers).
pub fn set_active_preset(preset: VisualizerPreset) {
    let val = match preset {
        VisualizerPreset::Waveform => 0,
        VisualizerPreset::Spectrum => 1,
        VisualizerPreset::Plasma => 2,
        VisualizerPreset::Tunnel => 3,
        VisualizerPreset::Custom => 4,
    };
    ACTIVE_PRESET.store(val, Ordering::Relaxed);
}

fn read_active_preset() -> VisualizerPreset {
    match ACTIVE_PRESET.load(Ordering::Relaxed) {
        0 => VisualizerPreset::Waveform,
        1 => VisualizerPreset::Spectrum,
        2 => VisualizerPreset::Plasma,
        3 => VisualizerPreset::Tunnel,
        4 => VisualizerPreset::Custom,
        _ => VisualizerPreset::Spectrum,
    }
}

/// Custom shader parameters parsed from user-edited source text.
/// This drives a CPU-rendered custom preset using parameter extraction.
#[derive(Clone, Debug)]
pub struct CustomShaderParams {
    /// Color A RGB (0..1)
    pub color_a: [f32; 3],
    /// Color B RGB (0..1)
    pub color_b: [f32; 3],
    /// Animation speed multiplier
    pub speed: f32,
    /// Pattern type: 0=plasma, 1=rings, 2=waves, 3=grid
    pub pattern: u8,
    /// Frequency scale
    pub freq_scale: f32,
}

impl Default for CustomShaderParams {
    fn default() -> Self {
        Self {
            color_a: COLOR_MAUVE,
            color_b: COLOR_BLUE,
            speed: 1.0,
            pattern: 0,
            freq_scale: 10.0,
        }
    }
}

/// Global custom shader state — written by ShaderEditor Apply, read by VisualizerView each frame.
static CUSTOM_SHADER: OnceLock<Mutex<CustomShaderParams>> = OnceLock::new();

fn custom_shader_state() -> &'static Mutex<CustomShaderParams> {
    CUSTOM_SHADER.get_or_init(|| Mutex::new(CustomShaderParams::default()))
}

/// Parse a shader source string into CustomShaderParams.
/// The "shader language" is a simple key=value format:
///   color_a: 0.8 0.6 0.9
///   color_b: 0.5 0.7 1.0
///   speed: 2.0
///   pattern: rings
///   freq: 12.0
/// Returns Ok(params) or Err(message) on parse failure.
pub fn parse_custom_shader(src: &str) -> Result<CustomShaderParams, String> {
    let mut params = CustomShaderParams::default();
    for line in src.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid line (expected key: value): '{}'", line));
        }
        let key = parts[0].trim().to_lowercase();
        let val = parts[1].trim();
        match key.as_str() {
            "color_a" | "colora" => {
                params.color_a = parse_color3(val)
                    .map_err(|e| format!("color_a: {}", e))?;
            }
            "color_b" | "colorb" => {
                params.color_b = parse_color3(val)
                    .map_err(|e| format!("color_b: {}", e))?;
            }
            "speed" => {
                params.speed = val.parse::<f32>()
                    .map_err(|_| format!("speed: expected number, got '{}'", val))?
                    .clamp(0.1, 20.0);
            }
            "pattern" => {
                params.pattern = match val.to_lowercase().as_str() {
                    "plasma" | "0" => 0,
                    "rings" | "1" => 1,
                    "waves" | "2" => 2,
                    "grid" | "3" => 3,
                    _ => return Err(format!("pattern: unknown '{}' (plasma/rings/waves/grid)", val)),
                };
            }
            "freq" | "frequency" => {
                params.freq_scale = val.parse::<f32>()
                    .map_err(|_| format!("freq: expected number, got '{}'", val))?
                    .clamp(1.0, 100.0);
            }
            _ => {
                return Err(format!("Unknown parameter: '{}'", key));
            }
        }
    }
    Ok(params)
}

fn parse_color3(s: &str) -> Result<[f32; 3], String> {
    // Accept "0.8 0.6 0.9" or hex "#cba6f7"
    let s = s.trim();
    if s.starts_with('#') && s.len() == 7 {
        let r = u8::from_str_radix(&s[1..3], 16).map_err(|_| "invalid hex")?;
        let g = u8::from_str_radix(&s[3..5], 16).map_err(|_| "invalid hex")?;
        let b = u8::from_str_radix(&s[5..7], 16).map_err(|_| "invalid hex")?;
        return Ok([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]);
    }
    let nums: Vec<f32> = s.split_whitespace()
        .map(|n| n.parse::<f32>().map_err(|_| format!("not a number: '{}'", n)))
        .collect::<Result<Vec<_>, _>>()?;
    if nums.len() != 3 {
        return Err(format!("expected 3 values, got {}", nums.len()));
    }
    Ok([nums[0].clamp(0.0, 1.0), nums[1].clamp(0.0, 1.0), nums[2].clamp(0.0, 1.0)])
}

/// Attempt to reload the custom shader from source text.
/// On success, updates global custom shader params and switches to Custom preset.
/// On failure, returns error message; previous shader keeps running.
pub fn reload_shader(src: &str) -> Result<(), String> {
    let params = parse_custom_shader(src)?;
    // Update global custom shader state
    if let Ok(mut state) = custom_shader_state().lock() {
        *state = params;
    }
    // Switch to custom preset
    set_active_preset(VisualizerPreset::Custom);
    Ok(())
}

/// Returns the default custom shader source text.
pub fn default_custom_source() -> String {
    "// HUM Custom Visualizer\n\
     // Edit parameters and press Apply\n\
     //\n\
     // Colors: RGB floats (0.0-1.0) or hex (#cba6f7)\n\
     // Pattern: plasma / rings / waves / grid\n\
     \n\
     color_a: #cba6f7\n\
     color_b: #89b4fa\n\
     speed: 1.0\n\
     pattern: plasma\n\
     freq: 10.0\n"
    .to_string()
}

live_design! {
    use link::theme::*;
    use link::widgets::*;

    VIZ_BG = #181825

    pub VisualizerView = {{VisualizerView}} {
        width: Fill
        height: 220
        show_bg: true
        draw_bg: { color: (VIZ_BG) }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct VisualizerView {
    #[redraw]
    #[live]
    draw_bg: DrawColor,

    #[walk]
    walk: Walk,

    #[layout]
    layout: Layout,

    /// Current active preset.
    #[rust]
    preset: VisualizerPreset,

    /// Accumulated time for animation (seconds).
    #[rust]
    time: f64,

    /// Instant of last frame for delta-time calculation.
    #[rust]
    last_instant: Option<std::time::Instant>,

    /// Reusable DrawColor instances for bar-based rendering (spectrum/waveform).
    #[rust]
    draw_elements: Vec<DrawColor>,
}

impl Widget for VisualizerView {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {
        // No interactive events for the visualizer
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        if rect.size.x < 1.0 || rect.size.y < 1.0 {
            return DrawStep::done();
        }

        // Draw background
        self.draw_bg.draw_abs(cx, rect);

        // Read current FFT bins + beat energy + per-thing amplitudes
        let (bins, beat_energy, per_thing) = fft_state()
            .and_then(|fft| fft.lock().ok().map(|s| (s.bins, s.beat_energy, s.per_thing)))
            .unwrap_or(([0.0f32; NUM_BINS], 0.0, [(0.0, [0u8; 32]); 8]));

        // Compute band energies
        let bass = mean_range(&bins, 0, 8);
        let mids = mean_range(&bins, 8, 32);
        let highs = mean_range(&bins, 32, 64);
        let volume = bins.iter().cloned().fold(0.0f32, f32::max);

        // Update time
        let now = std::time::Instant::now();
        if let Some(last) = self.last_instant {
            let dt = now.duration_since(last).as_secs_f64();
            self.time += dt;
        }
        self.last_instant = Some(now);

        // Read active preset from global (set by app.rs button handlers)
        self.preset = read_active_preset();

        // Dispatch to preset renderer (beat_energy passed to all presets)
        match self.preset {
            VisualizerPreset::Waveform => self.draw_waveform(cx, rect, &bins, beat_energy),
            VisualizerPreset::Spectrum => self.draw_spectrum(cx, rect, &bins, beat_energy),
            VisualizerPreset::Plasma => self.draw_plasma(cx, rect, bass, mids, highs, beat_energy),
            VisualizerPreset::Tunnel => self.draw_tunnel(cx, rect, volume, mids, beat_energy),
            VisualizerPreset::Custom => self.draw_custom(cx, rect, bass, mids, highs, volume),
        }

        // Overlay per-thing amplitude dots at the bottom edge (all presets)
        self.draw_thing_dots(cx, rect, &per_thing);

        DrawStep::done()
    }
}

impl VisualizerView {
    /// Ensure we have at least `n` DrawColor instances available.
    fn ensure_elements(&mut self, cx: &mut Cx2d, n: usize) {
        while self.draw_elements.len() < n {
            self.draw_elements.push(DrawColor::new_local(cx));
        }
    }

    // -----------------------------------------------------------------------
    // Preset: Waveform — continuous wave line from FFT bins
    // Beat: vertical scale boost + color shift toward mauve
    // -----------------------------------------------------------------------
    fn draw_waveform(&mut self, cx: &mut Cx2d, rect: Rect, bins: &[f32; NUM_BINS], beat: f32) {
        let num_cols = NUM_BINS * 2;
        self.ensure_elements(cx, num_cols);

        let col_width = rect.size.x / num_cols as f64;
        let center_y = rect.pos.y + rect.size.y * 0.5;
        let max_amp = rect.size.y * 0.45;
        // Beat boosts vertical scale
        let beat_scale = 1.0 + beat as f64 * 1.5;

        for i in 0..num_cols {
            let bin_f = (i as f32 / num_cols as f32) * (NUM_BINS - 1) as f32;
            let bin_lo = bin_f as usize;
            let bin_hi = (bin_lo + 1).min(NUM_BINS - 1);
            let t = bin_f - bin_lo as f32;
            let magnitude = bins[bin_lo] * (1.0 - t) + bins[bin_hi] * t;
            let magnitude = magnitude.clamp(0.0, 1.0);

            let bar_h = (magnitude as f64 * max_amp * beat_scale).max(0.5);
            let x = rect.pos.x + i as f64 * col_width;
            let y = center_y - bar_h;

            // Color shifts toward mauve at beat peak
            let base_color = lerp3(COLOR_BLUE, COLOR_MAUVE, magnitude);
            let color = lerp3(base_color, COLOR_MAUVE, beat * 0.6);
            self.draw_elements[i].color = vec4(color[0], color[1], color[2], 1.0);
            self.draw_elements[i].draw_abs(cx, Rect {
                pos: dvec2(x, y),
                size: dvec2(col_width.max(1.0), bar_h * 2.0),
            });
        }
    }

    // -----------------------------------------------------------------------
    // Preset: Spectrum — 64 magnitude bars
    // Beat: brightness boost + white flash on transient
    // -----------------------------------------------------------------------
    fn draw_spectrum(&mut self, cx: &mut Cx2d, rect: Rect, bins: &[f32; NUM_BINS], beat: f32) {
        self.ensure_elements(cx, NUM_BINS);

        let total_width = rect.size.x;
        let max_height = rect.size.y - 4.0;
        let bar_width = (total_width / NUM_BINS as f64) - BAR_GAP;
        let bar_width = bar_width.max(1.0);
        // Beat boosts bar brightness
        let brightness_mult = 1.0 + beat * 2.0;

        for i in 0..NUM_BINS {
            let magnitude = bins[i].clamp(0.0, 1.0);

            let mut color = bar_color(magnitude);
            // Apply brightness boost
            color[0] = (color[0] * brightness_mult).min(1.0);
            color[1] = (color[1] * brightness_mult).min(1.0);
            color[2] = (color[2] * brightness_mult).min(1.0);
            // White flash on beat: blend toward white
            let white = [1.0f32; 3];
            color = lerp3(color, white, beat * 0.4);

            let bar_height = (magnitude as f64 * max_height).max(1.0);
            let x = rect.pos.x + (i as f64 * (bar_width + BAR_GAP));
            let y = rect.pos.y + rect.size.y - bar_height - 2.0;

            self.draw_elements[i].color = vec4(color[0], color[1], color[2], 1.0);
            self.draw_elements[i].draw_abs(cx, Rect {
                pos: dvec2(x, y),
                size: dvec2(bar_width, bar_height),
            });
        }
    }

    // -----------------------------------------------------------------------
    // Preset: Plasma — animated colour fields driven by bass/mids/highs
    // Beat: luminance flash + time acceleration on transient
    // -----------------------------------------------------------------------
    fn draw_plasma(&mut self, cx: &mut Cx2d, rect: Rect, bass: f32, mids: f32, highs: f32, beat: f32) {
        let cols = 64usize;
        let rows = 32usize;
        let total = cols * rows;
        self.ensure_elements(cx, total);

        let cell_w = rect.size.x / cols as f64;
        let cell_h = rect.size.y / rows as f64;
        // Beat accelerates animation time
        let time = self.time as f32 + beat * 0.5;
        let speed = bass * 3.0 + 0.5;

        let mut idx = 0;
        for row in 0..rows {
            for col in 0..cols {
                let nx = col as f32 / cols as f32;
                let ny = row as f32 / rows as f32;

                let v = (nx * 10.0 + time * speed).sin()
                    + (ny * 10.0 + time * speed * 0.7).sin()
                    + ((nx + ny) * 6.0 + time * speed * 1.3).sin();
                let v = (v + 3.0) / 6.0;

                let mut r = COLOR_GREEN[0] * v + COLOR_MAUVE[0] * (1.0 - v) * (mids + 0.3);
                let mut g = COLOR_GREEN[1] * v * (bass + 0.5) + COLOR_BLUE[1] * (1.0 - v);
                let mut b = COLOR_BLUE[2] * v + COLOR_MAUVE[2] * (1.0 - v) * (highs + 0.3);

                // Beat flash: add luminance on transient
                r += beat * 0.4;
                g += beat * 0.4;
                b += beat * 0.4;

                let x = rect.pos.x + col as f64 * cell_w;
                let y = rect.pos.y + row as f64 * cell_h;

                self.draw_elements[idx].color = vec4(
                    r.clamp(0.0, 1.0),
                    g.clamp(0.0, 1.0),
                    b.clamp(0.0, 1.0),
                    1.0,
                );
                self.draw_elements[idx].draw_abs(cx, Rect {
                    pos: dvec2(x, y),
                    size: dvec2(cell_w.ceil(), cell_h.ceil()),
                });
                idx += 1;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Preset: Tunnel — receding-ring zoom driven by beat energy
    // Beat: ring speed boost + bright ring burst at center on transient
    // -----------------------------------------------------------------------
    fn draw_tunnel(&mut self, cx: &mut Cx2d, rect: Rect, volume: f32, mids: f32, beat: f32) {
        let cols = 64usize;
        let rows = 32usize;
        let total = cols * rows;
        self.ensure_elements(cx, total);

        let cell_w = rect.size.x / cols as f64;
        let cell_h = rect.size.y / rows as f64;
        let time = self.time as f32;
        let cx_f = 0.5f32;
        let cy_f = 0.5f32;
        // Beat boosts ring zoom speed
        let speed_mult = 1.0 + beat * 3.0;

        let mut idx = 0;
        for row in 0..rows {
            for col in 0..cols {
                let nx = col as f32 / cols as f32;
                let ny = row as f32 / rows as f32;

                let dx = nx - cx_f;
                let dy = (ny - cy_f) * (rect.size.y as f32 / rect.size.x as f32);
                let dist = (dx * dx + dy * dy).sqrt();

                let ring = (dist * 8.0 - time * volume * 4.0 * speed_mult).fract().abs();
                let brightness = ring * (mids * 2.0 + 0.3);

                let t = (dist * 2.0).clamp(0.0, 1.0);
                let mut r = COLOR_BLUE[0] * (1.0 - t) + COLOR_MAUVE[0] * t;
                let mut g = COLOR_BLUE[1] * (1.0 - t) + COLOR_MAUVE[1] * t;
                let mut b_c = COLOR_BLUE[2] * (1.0 - t) + COLOR_MAUVE[2] * t;

                r *= brightness;
                g *= brightness;
                b_c *= brightness;

                // Beat ring burst: bright ring at center when beat is high
                if dist < 0.1 * beat && beat > 0.1 {
                    let burst = (1.0 - dist / (0.1 * beat)).clamp(0.0, 1.0);
                    r = r + burst * 0.8;
                    g = g + burst * 0.7;
                    b_c = b_c + burst * 1.0;
                }

                let x = rect.pos.x + col as f64 * cell_w;
                let y = rect.pos.y + row as f64 * cell_h;

                self.draw_elements[idx].color = vec4(
                    r.clamp(0.0, 1.0),
                    g.clamp(0.0, 1.0),
                    b_c.clamp(0.0, 1.0),
                    1.0,
                );
                self.draw_elements[idx].draw_abs(cx, Rect {
                    pos: dvec2(x, y),
                    size: dvec2(cell_w.ceil(), cell_h.ceil()),
                });
                idx += 1;
            }
        }
    }
    // -----------------------------------------------------------------------
    // Preset: Custom — user-editable parameters drive a CPU-rendered effect
    // -----------------------------------------------------------------------
    fn draw_custom(
        &mut self,
        cx: &mut Cx2d,
        rect: Rect,
        bass: f32,
        mids: f32,
        highs: f32,
        volume: f32,
    ) {
        let params = custom_shader_state()
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default();

        let cols = 64usize;
        let rows = 32usize;
        let total = cols * rows;
        self.ensure_elements(cx, total);

        let cell_w = rect.size.x / cols as f64;
        let cell_h = rect.size.y / rows as f64;
        let time = self.time as f32;
        let speed = params.speed * (bass * 2.0 + 0.5);
        let freq = params.freq_scale;

        let mut idx = 0;
        for row in 0..rows {
            for col in 0..cols {
                let nx = col as f32 / cols as f32;
                let ny = row as f32 / rows as f32;

                // Compute pattern value (0..1)
                let v = match params.pattern {
                    0 => {
                        // Plasma
                        let v = (nx * freq + time * speed).sin()
                            + (ny * freq + time * speed * 0.7).sin()
                            + ((nx + ny) * freq * 0.6 + time * speed * 1.3).sin();
                        (v + 3.0) / 6.0
                    }
                    1 => {
                        // Rings
                        let dx = nx - 0.5;
                        let dy = (ny - 0.5) * (rect.size.y as f32 / rect.size.x as f32);
                        let dist = (dx * dx + dy * dy).sqrt();
                        let ring = (dist * freq - time * speed).fract().abs();
                        ring * (mids * 2.0 + 0.3)
                    }
                    2 => {
                        // Waves
                        let wave = (nx * freq + time * speed + ny * 3.0).sin() * 0.5 + 0.5;
                        wave * (volume + 0.4)
                    }
                    _ => {
                        // Grid
                        let gx = ((nx * freq + time * speed * 0.3).sin() * 0.5 + 0.5).powf(2.0);
                        let gy = ((ny * freq + time * speed * 0.5).sin() * 0.5 + 0.5).powf(2.0);
                        (gx + gy) * 0.5 * (highs + 0.3)
                    }
                };
                let v = v.clamp(0.0, 1.0);

                // Mix color_a and color_b based on pattern value
                let r = params.color_a[0] * v + params.color_b[0] * (1.0 - v);
                let g = params.color_a[1] * v + params.color_b[1] * (1.0 - v);
                let b = params.color_a[2] * v + params.color_b[2] * (1.0 - v);

                let x = rect.pos.x + col as f64 * cell_w;
                let y = rect.pos.y + row as f64 * cell_h;

                self.draw_elements[idx].color = vec4(
                    r.clamp(0.0, 1.0),
                    g.clamp(0.0, 1.0),
                    b.clamp(0.0, 1.0),
                    1.0,
                );
                self.draw_elements[idx].draw_abs(cx, Rect {
                    pos: dvec2(x, y),
                    size: dvec2(cell_w.ceil(), cell_h.ceil()),
                });
                idx += 1;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Per-thing amplitude dots — overlaid on all presets at the bottom edge
    // -----------------------------------------------------------------------
    fn draw_thing_dots(
        &mut self,
        cx: &mut Cx2d,
        rect: Rect,
        per_thing: &[(f32, [u8; 32]); 8],
    ) {
        // Each thing gets a small colored dot at evenly-spaced positions
        // along the bottom edge. Dot radius and alpha scale with amplitude.
        let dot_base_idx = self.draw_elements.len();
        self.ensure_elements(cx, dot_base_idx + 8);

        let dot_y = rect.pos.y + rect.size.y - rect.size.y * 0.04; // 4% from bottom

        for i in 0..8 {
            let amp = per_thing[i].0;
            if amp < 0.001 {
                continue; // Skip silent/unused slots
            }

            let dot_x = rect.pos.x + ((i as f64 + 0.5) / 8.0) * rect.size.x;
            let radius = (0.015 + amp * 0.025) as f64 * rect.size.y;
            let color = THING_COLORS[i];

            self.draw_elements[dot_base_idx + i].color = vec4(
                color[0],
                color[1],
                color[2],
                amp.clamp(0.0, 1.0), // alpha proportional to amplitude
            );
            self.draw_elements[dot_base_idx + i].draw_abs(cx, Rect {
                pos: dvec2(dot_x - radius, dot_y - radius),
                size: dvec2(radius * 2.0, radius * 2.0),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn mean_range(bins: &[f32; NUM_BINS], start: usize, end: usize) -> f32 {
    let count = (end - start) as f32;
    if count <= 0.0 {
        return 0.0;
    }
    bins[start..end].iter().sum::<f32>() / count
}

/// Lerp bar color based on magnitude:
/// 0.0       -> COLOR_SURFACE0 (dark surface)
/// 0.0..0.5  -> lerp SURFACE0 -> BLUE
/// 0.5..1.0  -> lerp BLUE -> MAUVE
fn bar_color(magnitude: f32) -> [f32; 3] {
    if magnitude <= 0.0 {
        COLOR_SURFACE0
    } else if magnitude <= 0.5 {
        let t = magnitude / 0.5;
        lerp3(COLOR_SURFACE0, COLOR_BLUE, t)
    } else {
        let t = (magnitude - 0.5) / 0.5;
        lerp3(COLOR_BLUE, COLOR_MAUVE, t)
    }
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

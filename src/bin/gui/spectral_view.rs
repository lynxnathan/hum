use makepad_widgets::*;
use std::sync::{Arc, Mutex, OnceLock};
use crate::transport_client::FftState;

live_design! {
    use link::theme::*;
    use link::widgets::*;

    SPECTRAL_BG = #181825

    pub SpectralView = {{SpectralView}} {
        width: Fill
        height: 200
        show_bg: true
        draw_bg: { color: (SPECTRAL_BG) }
    }
}

/// Global shared FFT state. Set once at startup by App, read every frame by SpectralView.
/// Using OnceLock avoids needing to cast WidgetRef to access custom widget fields.
static FFT_STATE: OnceLock<Arc<Mutex<FftState>>> = OnceLock::new();

/// Initialize the global FFT state. Called once from App::handle_startup.
pub fn init_fft_state(state: Arc<Mutex<FftState>>) {
    FFT_STATE.set(state).ok();
}

/// Color constants for bar gradient (Catppuccin Mocha):
/// zero  -> #313244 (surface0, dark grey)
/// mid   -> #89b4fa (blue)
/// high  -> #cba6f7 (mauve)
const COLOR_ZERO: [f32; 3] = [0.192, 0.196, 0.267]; // #313244
const COLOR_MID: [f32; 3] = [0.537, 0.706, 0.980];  // #89b4fa
const COLOR_HIGH: [f32; 3] = [0.796, 0.651, 0.969];  // #cba6f7

/// Number of frequency bins from FFT analysis.
const NUM_BINS: usize = 64;

/// Gap between bars in pixels.
const BAR_GAP: f64 = 1.0;

#[derive(Live, LiveHook, Widget)]
pub struct SpectralView {
    #[redraw]
    #[live]
    draw_bg: DrawColor,

    #[walk]
    walk: Walk,

    #[layout]
    layout: Layout,

    #[rust]
    draw_bars: Vec<DrawColor>,
}

impl Widget for SpectralView {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {
        // No interactive events for the spectral view
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        // Begin drawing: get our allocated rect
        let rect = cx.walk_turtle(walk);
        if rect.size.x < 1.0 || rect.size.y < 1.0 {
            return DrawStep::done();
        }

        // Draw background
        self.draw_bg.draw_abs(cx, rect);

        // Read current FFT bins from the global shared state
        let bins = FFT_STATE
            .get()
            .and_then(|fft| fft.lock().ok().map(|s| s.bins))
            .unwrap_or([0.0f32; NUM_BINS]);

        // Ensure we have enough DrawColor instances for bars
        while self.draw_bars.len() < NUM_BINS {
            self.draw_bars.push(DrawColor::new_local(cx));
        }

        // Calculate bar dimensions
        let total_width = rect.size.x;
        let max_height = rect.size.y - 4.0; // 2px padding top+bottom
        let bar_width = (total_width / NUM_BINS as f64) - BAR_GAP;
        let bar_width = bar_width.max(1.0);

        // Draw each frequency bar
        for i in 0..NUM_BINS {
            let magnitude = bins[i].clamp(0.0, 1.0);

            // Compute bar color: lerp zero->mid->high based on magnitude
            let color = bar_color(magnitude);

            // Bar position: grows upward from bottom
            let bar_height = (magnitude as f64 * max_height).max(1.0);
            let x = rect.pos.x + (i as f64 * (bar_width + BAR_GAP));
            let y = rect.pos.y + rect.size.y - bar_height - 2.0; // 2px bottom pad

            self.draw_bars[i].color = vec4(color[0], color[1], color[2], 1.0);
            self.draw_bars[i].draw_abs(cx, Rect {
                pos: dvec2(x, y),
                size: dvec2(bar_width, bar_height),
            });
        }

        DrawStep::done()
    }
}

/// Lerp bar color based on magnitude:
/// 0.0       -> COLOR_ZERO (dark surface)
/// 0.0..0.5  -> lerp ZERO -> MID (blue)
/// 0.5..1.0  -> lerp MID -> HIGH (mauve)
fn bar_color(magnitude: f32) -> [f32; 3] {
    if magnitude <= 0.0 {
        COLOR_ZERO
    } else if magnitude <= 0.5 {
        let t = magnitude / 0.5;
        lerp3(COLOR_ZERO, COLOR_MID, t)
    } else {
        let t = (magnitude - 0.5) / 0.5;
        lerp3(COLOR_MID, COLOR_HIGH, t)
    }
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

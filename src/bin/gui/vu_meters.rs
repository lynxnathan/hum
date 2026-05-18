use makepad_widgets::*;
use std::sync::{Arc, Mutex, OnceLock};
use crate::transport_client::GuiState;

const LANE_HEIGHT: f64 = 32.0;
const VU_BAR_MAX_WIDTH: f64 = 72.0; // max bar width inside 80px panel
const VU_BAR_HEIGHT: f64 = 28.0;    // bar height centered in lane

const COLOR_GREEN: [f32; 4] = [0.651, 0.890, 0.631, 1.0]; // #a6e3a1
const COLOR_RED: [f32; 4] = [0.953, 0.545, 0.659, 1.0];   // #f38ba8
const COLOR_BG: [f32; 4] = [0.118, 0.118, 0.157, 1.0];    // #1e1e28

/// Global shared GUI state for VU meters. Set once at startup.
static VU_GUI_STATE: OnceLock<Arc<Mutex<GuiState>>> = OnceLock::new();

/// Global thing name order (matches arrangement lanes).
static VU_THING_NAMES: OnceLock<Vec<String>> = OnceLock::new();

/// Initialize global VU state. Called once from App::handle_startup.
pub fn init_vu_state(gui_state: Arc<Mutex<GuiState>>, thing_names: Vec<String>) {
    VU_GUI_STATE.set(gui_state).ok();
    VU_THING_NAMES.set(thing_names).ok();
}

live_design! {
    use link::theme::*;
    use link::widgets::*;

    VU_BG = #181825

    pub VuMeters = {{VuMeters}} {
        width: 80
        height: Fill
        show_bg: true
        draw_bg: { color: (VU_BG) }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct VuMeters {
    #[redraw]
    #[live]
    draw_bg: DrawColor,

    #[walk]
    walk: Walk,

    #[layout]
    layout: Layout,

    /// Reusable DrawColor instances for VU bars.
    #[rust]
    draw_bars: Vec<DrawColor>,
}

impl Widget for VuMeters {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {
        // No interactive events for VU meters
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        if rect.size.x < 1.0 || rect.size.y < 1.0 {
            return DrawStep::done();
        }

        // Draw background
        self.draw_bg.draw_abs(cx, rect);

        let thing_names = match VU_THING_NAMES.get() {
            Some(names) => names,
            None => return DrawStep::done(),
        };

        let amplitudes = VU_GUI_STATE
            .get()
            .and_then(|gs| gs.lock().ok().map(|s| s.amplitudes.clone()))
            .unwrap_or_default();

        // Ensure enough DrawColor instances
        while self.draw_bars.len() < thing_names.len() * 2 {
            self.draw_bars.push(DrawColor::new_local(cx));
        }

        let mut ri = 0;
        for (i, name) in thing_names.iter().enumerate() {
            let y = rect.pos.y + i as f64 * LANE_HEIGHT;
            let amp = amplitudes.get(name).copied().unwrap_or(0.0).clamp(0.0, 1.0);

            // Lane background slot
            self.draw_bars[ri].color = vec4(COLOR_BG[0], COLOR_BG[1], COLOR_BG[2], COLOR_BG[3]);
            self.draw_bars[ri].draw_abs(cx, Rect {
                pos: dvec2(rect.pos.x + 4.0, y + (LANE_HEIGHT - VU_BAR_HEIGHT) / 2.0),
                size: dvec2(VU_BAR_MAX_WIDTH, VU_BAR_HEIGHT),
            });
            ri += 1;

            // Amplitude bar
            let bar_w = amp as f64 * VU_BAR_MAX_WIDTH;
            if bar_w > 0.5 {
                let color = if amp < 0.7 { COLOR_GREEN } else { COLOR_RED };
                self.draw_bars[ri].color = vec4(color[0], color[1], color[2], color[3]);
                self.draw_bars[ri].draw_abs(cx, Rect {
                    pos: dvec2(rect.pos.x + 4.0, y + (LANE_HEIGHT - VU_BAR_HEIGHT) / 2.0),
                    size: dvec2(bar_w, VU_BAR_HEIGHT),
                });
            }
            ri += 1;
        }

        DrawStep::done()
    }
}

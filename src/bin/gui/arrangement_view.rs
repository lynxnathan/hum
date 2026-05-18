use makepad_widgets::*;
use std::sync::{Arc, Mutex, OnceLock};
use crate::transport_client::{self, GuiState};

// Catppuccin Mocha palette for thing lanes (cycling)
const PALETTE: &[[f32; 4]] = &[
    [0.537, 0.706, 0.980, 1.0], // #89b4fa blue
    [0.651, 0.890, 0.631, 1.0], // #a6e3a1 green
    [0.980, 0.702, 0.529, 1.0], // #fab387 peach
    [0.796, 0.651, 0.969, 1.0], // #cba6f7 mauve
    [0.953, 0.545, 0.659, 1.0], // #f38ba8 red
    [0.537, 0.863, 0.922, 1.0], // #89dceb sky
];

const LANE_HEIGHT: f64 = 32.0;
const LABEL_WIDTH: f64 = 120.0;
const PLAYHEAD_COLOR: [f32; 4] = [0.953, 0.545, 0.659, 1.0]; // #f38ba8
const SOLO_BORDER_COLOR: [f32; 4] = [0.976, 0.886, 0.686, 1.0]; // #f9e2af
const MUTED_COLOR: [f32; 4] = [0.345, 0.357, 0.439, 0.6]; // #585b70 greyed

/// Global shared GUI state. Set once at startup, read every frame by ArrangementView.
static ARR_GUI_STATE: OnceLock<Arc<Mutex<GuiState>>> = OnceLock::new();

/// Global shared thing lanes parsed from piece.hum. Set once at startup.
static ARR_LANES: OnceLock<(Vec<ThingLane>, f64)> = OnceLock::new();

/// Initialize global arrangement state. Called once from App::handle_startup.
pub fn init_arrangement_state(gui_state: Arc<Mutex<GuiState>>) {
    ARR_GUI_STATE.set(gui_state).ok();
    let lanes = parse_piece_lanes();
    let total_duration = lanes.iter().map(|l| l.until).fold(60.0_f64, f64::max);
    ARR_LANES.set((lanes, total_duration)).ok();
}

/// A thing lane with its timing and assigned color.
#[derive(Clone, Debug)]
pub struct ThingLane {
    pub name: String,
    pub at: f64,     // start in seconds
    pub until: f64,  // end in seconds
    pub color: [f32; 4],
}

live_design! {
    use link::theme::*;
    use link::widgets::*;

    ARR_BG = #11111b

    pub ArrangementView = {{ArrangementView}} {
        width: Fill
        height: Fill
        show_bg: true
        draw_bg: { color: (ARR_BG) }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct ArrangementView {
    #[redraw]
    #[live]
    draw_bg: DrawColor,

    #[walk]
    walk: Walk,

    #[layout]
    layout: Layout,

    /// Reusable DrawColor instances for lane backgrounds, blocks, etc.
    /// Grown as needed, never shrunk.
    #[rust]
    draw_rects: Vec<DrawColor>,
}

impl ArrangementView {
    /// Ensure we have at least `n` DrawColor instances available.
    fn ensure_rects(&mut self, cx: &mut Cx2d, n: usize) {
        while self.draw_rects.len() < n {
            self.draw_rects.push(DrawColor::new_local(cx));
        }
    }
}

impl Widget for ArrangementView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        match event.hits(cx, self.draw_bg.area()) {
            Hit::FingerDown(fe) => {
                if let Some((lanes, _)) = ARR_LANES.get() {
                    // Compute relative Y from absolute position and area rect
                    let area_rect = self.draw_bg.area().rect(cx);
                    let rel_y = fe.abs.y - area_rect.pos.y;
                    let lane_index = (rel_y / LANE_HEIGHT) as usize;
                    if lane_index < lanes.len() {
                        let thing_name = &lanes[lane_index].name;
                        let cmd = format!(r#"{{"cmd":"solo","thing":"{}"}}"#, thing_name);
                        transport_client::send_cmd(&cmd).ok();
                    }
                }
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        if rect.size.x < 1.0 || rect.size.y < 1.0 {
            return DrawStep::done();
        }

        // Draw background
        self.draw_bg.draw_abs(cx, rect);

        let (lanes, total_duration) = match ARR_LANES.get() {
            Some((l, d)) => (l, *d),
            None => return DrawStep::done(),
        };

        let state = ARR_GUI_STATE
            .get()
            .and_then(|gs| gs.lock().ok().map(|s| s.clone()));

        if lanes.is_empty() {
            return DrawStep::done();
        }

        let timeline_x = rect.pos.x + LABEL_WIDTH;
        let timeline_w = (rect.size.x - LABEL_WIDTH).max(1.0);

        // We need up to: lanes * 3 (bg + label_bg + block) + lanes (solo borders * 2 uses same rect) + 1 (playhead)
        // Conservative: 4 per lane + 1
        self.ensure_rects(cx, lanes.len() * 4 + 1);
        let mut ri = 0; // rect index

        for (i, lane) in lanes.iter().enumerate() {
            let y = rect.pos.y + i as f64 * LANE_HEIGHT;

            // Lane background (alternating subtle shading)
            let bg_val = if i % 2 == 0 { 0.067_f32 } else { 0.078_f32 };
            self.draw_rects[ri].color = vec4(bg_val, bg_val, bg_val + 0.01, 1.0);
            self.draw_rects[ri].draw_abs(cx, Rect {
                pos: dvec2(rect.pos.x, y),
                size: dvec2(rect.size.x, LANE_HEIGHT),
            });
            ri += 1;

            // Label background
            self.draw_rects[ri].color = vec4(0.118, 0.118, 0.157, 1.0);
            self.draw_rects[ri].draw_abs(cx, Rect {
                pos: dvec2(rect.pos.x, y),
                size: dvec2(LABEL_WIDTH, LANE_HEIGHT),
            });
            ri += 1;

            // Colored block for at:/until:
            let x_start = timeline_x + (lane.at / total_duration) * timeline_w;
            let x_end = timeline_x + (lane.until / total_duration) * timeline_w;
            let block_w = (x_end - x_start).max(4.0);

            let (is_active, is_solo, is_muted) = if let Some(ref st) = state {
                (
                    st.active.contains(&lane.name),
                    st.solo.contains(&lane.name),
                    st.mute.contains(&lane.name),
                )
            } else {
                (false, false, false)
            };

            let block_color = if is_muted {
                MUTED_COLOR
            } else {
                let mut c = lane.color;
                if !is_active {
                    c[3] = 0.4;
                }
                c
            };

            self.draw_rects[ri].color = vec4(block_color[0], block_color[1], block_color[2], block_color[3]);
            self.draw_rects[ri].draw_abs(cx, Rect {
                pos: dvec2(x_start, y + 2.0),
                size: dvec2(block_w, LANE_HEIGHT - 4.0),
            });
            ri += 1;

            // Solo border highlight (top + bottom lines)
            if is_solo {
                self.draw_rects[ri].color = vec4(
                    SOLO_BORDER_COLOR[0], SOLO_BORDER_COLOR[1],
                    SOLO_BORDER_COLOR[2], SOLO_BORDER_COLOR[3],
                );
                // Top border
                self.draw_rects[ri].draw_abs(cx, Rect {
                    pos: dvec2(x_start, y + 1.0),
                    size: dvec2(block_w, 2.0),
                });
                // Bottom border
                self.draw_rects[ri].draw_abs(cx, Rect {
                    pos: dvec2(x_start, y + LANE_HEIGHT - 3.0),
                    size: dvec2(block_w, 2.0),
                });
                ri += 1;
            }
        }

        // Playhead
        if let Some(ref st) = state {
            if st.playing || st.pos > 0.0 {
                let playhead_x = timeline_x + (st.pos / total_duration) * timeline_w;
                if playhead_x >= timeline_x && playhead_x <= timeline_x + timeline_w {
                    let total_h = (lanes.len() as f64 * LANE_HEIGHT).max(rect.size.y);
                    self.ensure_rects(cx, ri + 1);
                    self.draw_rects[ri].color = vec4(
                        PLAYHEAD_COLOR[0], PLAYHEAD_COLOR[1],
                        PLAYHEAD_COLOR[2], PLAYHEAD_COLOR[3],
                    );
                    self.draw_rects[ri].draw_abs(cx, Rect {
                        pos: dvec2(playhead_x - 1.0, rect.pos.y),
                        size: dvec2(2.0, total_h),
                    });
                }
            }
        }

        DrawStep::done()
    }
}

/// Parse piece.hum to extract thing lanes with at:/until: values.
/// Reads HUM_PIECE env var (default: "piece.hum" in cwd).
pub fn parse_piece_lanes() -> Vec<ThingLane> {
    let piece_path = std::env::var("HUM_PIECE").unwrap_or_else(|_| "piece.hum".to_string());
    let content = match std::fs::read_to_string(&piece_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut lanes = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_at: Option<f64> = None;
    let mut current_until: Option<f64> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        // Top-level thing definition (not indented, ends with :)
        if !line.starts_with(' ') && !line.starts_with('\t') && trimmed.ends_with(':') && !trimmed.is_empty() {
            // Save previous thing if any
            if let Some(name) = current_name.take() {
                let at = current_at.unwrap_or(0.0);
                let until = current_until.unwrap_or(at + 60.0);
                let color_idx = lanes.len() % PALETTE.len();
                lanes.push(ThingLane {
                    name,
                    at,
                    until,
                    color: PALETTE[color_idx],
                });
            }
            current_name = Some(trimmed.trim_end_matches(':').to_string());
            current_at = None;
            current_until = None;
        } else if let Some(val) = trimmed.strip_prefix("at:") {
            current_at = parse_seconds(val.trim());
        } else if let Some(val) = trimmed.strip_prefix("until:") {
            current_until = parse_seconds(val.trim());
        }
    }

    // Don't forget the last thing
    if let Some(name) = current_name.take() {
        let at = current_at.unwrap_or(0.0);
        let until = current_until.unwrap_or(at + 60.0);
        let color_idx = lanes.len() % PALETTE.len();
        lanes.push(ThingLane {
            name,
            at,
            until,
            color: PALETTE[color_idx],
        });
    }

    lanes
}

/// Parse a time value like "15s", "120s", "3.5s", or bare "15" into seconds.
fn parse_seconds(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches('s');
    s.parse::<f64>().ok()
}

use makepad_widgets::*;
use crate::app::AUDIO_PARAMS;
use crate::nodes::NodeState;
use crate::spatial;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    DrawNodeCircle = {{DrawNodeCircle}} {
        fn pixel(self) -> vec4 {
            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
            sdf.circle(
                self.rect_size.x * 0.5,
                self.rect_size.y * 0.5,
                self.rect_size.x * 0.5 - 1.5
            );
            sdf.fill(self.color);
            return sdf.result;
        }
    }

    pub CanvasWidget = {{CanvasWidget}} {
        width: Fill, height: Fill
        draw_bg: { color: #111118 }
        node_a: { color: #00DDFF }
        node_b: { color: #FF44AA }
    }
}

#[derive(Live, LiveRegister, LiveHook)]
#[repr(C)]
pub struct DrawNodeCircle {
    #[deref] pub draw_super: DrawQuad,
    #[live] pub color: Vec4,
}

#[derive(Live, Widget)]
pub struct CanvasWidget {
    #[redraw] #[live] draw_bg: DrawQuad,
    #[walk] walk: Walk,
    #[layout] layout: Layout,
    #[live] node_a: DrawNodeCircle,
    #[live] node_b: DrawNodeCircle,
    #[rust] nodes: Vec<NodeState>,
    #[rust] active_node: Option<usize>,
    #[rust] canvas_rect: Rect,
}

impl LiveHook for CanvasWidget {
    fn after_new_from_doc(&mut self, _cx: &mut Cx) {
        self.nodes = vec![
            NodeState::initial_pos(0.30, 0.40, 440.0),
            NodeState::initial_pos(0.70, 0.60, 660.0),
        ];
    }
}

impl CanvasWidget {
    fn update_node_pos(&mut self, idx: usize, abs: DVec2) {
        let r = &self.canvas_rect;
        if r.size.x > 0.0 && r.size.y > 0.0 {
            let nx = ((abs.x - r.pos.x) / r.size.x).clamp(0.0, 1.0) as f32;
            let ny = ((abs.y - r.pos.y) / r.size.y).clamp(0.0, 1.0) as f32;
            self.nodes[idx].x = nx;
            self.nodes[idx].y = ny;
        }
    }

    fn write_spatial_params(&self) {
        if let Some(params) = AUDIO_PARAMS.get() {
            if self.nodes.len() >= 2 {
                let a = &self.nodes[0];
                let b = &self.nodes[1];
                params.pan_a.set_value(spatial::pan_from_x(a.x));
                params.pan_b.set_value(spatial::pan_from_x(b.x));
                params.blend.set_value(spatial::proximity_blend(a.x, a.y, b.x, b.y));
            }
        }
    }

    pub fn node_positions(&self) -> &[NodeState] {
        &self.nodes
    }
}

impl Widget for CanvasWidget {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        // Node A — checked first (higher priority when overlapping)
        match event.hits(cx, self.node_a.draw_vars.area()) {
            Hit::FingerDown(fd) if fd.device.is_primary_hit() => {
                self.active_node = Some(0);
            }
            Hit::FingerMove(fm) if self.active_node == Some(0) => {
                self.update_node_pos(0, fm.abs);
                self.write_spatial_params();
                cx.redraw_all();
            }
            Hit::FingerUp(_) if self.active_node == Some(0) => {
                self.active_node = None;
            }
            _ => {}
        }
        // Node B — checked second
        match event.hits(cx, self.node_b.draw_vars.area()) {
            Hit::FingerDown(fd) if fd.device.is_primary_hit() => {
                if self.active_node.is_none() {
                    self.active_node = Some(1);
                }
            }
            Hit::FingerMove(fm) if self.active_node == Some(1) => {
                self.update_node_pos(1, fm.abs);
                self.write_spatial_params();
                cx.redraw_all();
            }
            Hit::FingerUp(_) if self.active_node == Some(1) => {
                self.active_node = None;
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.canvas_rect = rect;

        // Draw background
        self.draw_bg.draw_abs(cx, rect);

        const R: f64 = 25.0;
        let diam = dvec2(R * 2.0, R * 2.0);

        if self.nodes.len() >= 2 {
            // Node A — cyan
            let pos_a = dvec2(
                rect.pos.x + self.nodes[0].x as f64 * rect.size.x - R,
                rect.pos.y + self.nodes[0].y as f64 * rect.size.y - R,
            );
            self.node_a.draw_abs(cx, Rect { pos: pos_a, size: diam });

            // Node B — magenta
            let pos_b = dvec2(
                rect.pos.x + self.nodes[1].x as f64 * rect.size.x - R,
                rect.pos.y + self.nodes[1].y as f64 * rect.size.y - R,
            );
            self.node_b.draw_abs(cx, Rect { pos: pos_b, size: diam });
        }

        DrawStep::done()
    }
}

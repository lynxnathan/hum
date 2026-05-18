//! Project browser sidebar for hum-gui.
//!
//! Lists .hum pieces, instruments/*.hum, and hum.dict in a scrollable
//! sidebar rendered via DrawColor rects + DrawText labels.
//! Clicking an entry opens it in $EDITOR.  (IDE-01)

use makepad_widgets::*;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Global project root path. Set once at startup from App::handle_startup.
static PROJECT_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Initialize the project root. Called once from app.rs handle_startup.
pub fn init_project_root(root: PathBuf) {
    PROJECT_ROOT.set(root).ok();
}

live_design! {
    use link::theme::*;
    use link::widgets::*;

    BROWSER_BG = #181825

    pub ProjectBrowser = {{ProjectBrowser}} {
        width: Fill
        height: Fill
        show_bg: true
        draw_bg: { color: (BROWSER_BG) }
    }
}

const ROW_HEIGHT: f64 = 26.0;
const HEADER_HEIGHT: f64 = 32.0;
const LEFT_PAD: f64 = 12.0;

const BG_COLOR: [f32; 4] = [0.094, 0.094, 0.145, 1.0];       // #181825
const SELECTED_COLOR: [f32; 4] = [0.192, 0.196, 0.267, 1.0];  // #313244
const HOVER_COLOR: [f32; 4] = [0.153, 0.157, 0.220, 1.0];     // slightly lighter
const TEXT_COLOR: [f32; 4] = [0.804, 0.839, 0.957, 1.0];       // #cdd6f4
const SUBTLE_COLOR: [f32; 4] = [0.651, 0.678, 0.784, 1.0];    // #a6adc8
const ACCENT_COLOR: [f32; 4] = [0.537, 0.706, 0.980, 1.0];    // #89b4fa

/// What kind of project entry this is.
#[derive(Debug, Clone, PartialEq)]
pub enum EntryKind {
    Piece,
    Instrument,
    Dict,
}

/// A single entry in the project browser.
#[derive(Debug, Clone)]
pub struct BrowserEntry {
    pub label: String,
    pub path: PathBuf,
    pub kind: EntryKind,
}

#[derive(Live, LiveHook, Widget)]
pub struct ProjectBrowser {
    #[redraw]
    #[live]
    draw_bg: DrawColor,

    #[walk]
    walk: Walk,

    #[layout]
    layout: Layout,

    #[rust]
    entries: Vec<BrowserEntry>,
    #[rust]
    selected: Option<usize>,
    #[rust]
    draw_rects: Vec<DrawColor>,
    #[rust]
    draw_texts: Vec<DrawText>,
    #[rust]
    area_rect: Rect,
    #[rust]
    initialized: bool,
}

impl ProjectBrowser {
    fn ensure_rects(&mut self, cx: &mut Cx2d, n: usize) {
        while self.draw_rects.len() < n {
            self.draw_rects.push(DrawColor::new_local(cx));
        }
    }

    fn ensure_texts(&mut self, cx: &mut Cx2d, n: usize) {
        while self.draw_texts.len() < n {
            self.draw_texts.push(DrawText::new_local(cx));
        }
    }
}

impl Widget for ProjectBrowser {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        match event.hits(cx, self.draw_bg.area()) {
            Hit::FingerDown(fe) => {
                let rel_y = fe.abs.y - self.area_rect.pos.y - HEADER_HEIGHT;
                if rel_y >= 0.0 {
                    let idx = (rel_y / ROW_HEIGHT) as usize;
                    if idx < self.entries.len() {
                        self.selected = Some(idx);
                        Self::open_file(&self.entries[idx].path);
                        cx.redraw_all();
                    }
                }
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        // Auto-refresh on first draw
        if !self.initialized {
            if let Some(root) = PROJECT_ROOT.get() {
                self.refresh(root);
            }
            self.initialized = true;
        }

        let rect = cx.walk_turtle(walk);
        if rect.size.x < 1.0 || rect.size.y < 1.0 {
            return DrawStep::done();
        }
        self.area_rect = rect;

        // Background
        self.draw_bg.draw_abs(cx, rect);

        // Need: 1 rect per entry (row bg) + 1 header text + 1 text per entry + 1 icon per entry
        let n = self.entries.len();
        self.ensure_rects(cx, n + 1);
        self.ensure_texts(cx, n * 2 + 1); // header + name + icon per entry

        let mut ti = 0; // text index
        let mut ri = 0; // rect index

        // Header: "PROJECT"
        self.draw_texts[ti].color = vec4(SUBTLE_COLOR[0], SUBTLE_COLOR[1], SUBTLE_COLOR[2], 1.0);
        self.draw_texts[ti].text_style.font_size = 9.0;
        self.draw_texts[ti].draw_abs(cx, dvec2(rect.pos.x + LEFT_PAD, rect.pos.y + 12.0), "PROJECT");
        ti += 1;

        // Entry rows
        for (i, entry) in self.entries.iter().enumerate() {
            let y = rect.pos.y + HEADER_HEIGHT + i as f64 * ROW_HEIGHT;

            // Row background (highlight selected)
            let row_color = if self.selected == Some(i) {
                SELECTED_COLOR
            } else if i % 2 == 0 {
                BG_COLOR
            } else {
                [BG_COLOR[0] + 0.01, BG_COLOR[1] + 0.01, BG_COLOR[2] + 0.01, 1.0]
            };
            self.draw_rects[ri].color = vec4(row_color[0], row_color[1], row_color[2], row_color[3]);
            self.draw_rects[ri].draw_abs(cx, Rect {
                pos: dvec2(rect.pos.x, y),
                size: dvec2(rect.size.x, ROW_HEIGHT),
            });
            ri += 1;

            // Icon character
            let (icon, icon_color) = match entry.kind {
                EntryKind::Piece => ("~", ACCENT_COLOR),
                EntryKind::Instrument => ("i", SUBTLE_COLOR),
                EntryKind::Dict => ("d", SUBTLE_COLOR),
            };
            self.draw_texts[ti].color = vec4(icon_color[0], icon_color[1], icon_color[2], 1.0);
            self.draw_texts[ti].text_style.font_size = 10.0;
            self.draw_texts[ti].draw_abs(cx, dvec2(rect.pos.x + LEFT_PAD, y + 7.0), icon);
            ti += 1;

            // Entry name
            let text_color = if self.selected == Some(i) { ACCENT_COLOR } else { TEXT_COLOR };
            self.draw_texts[ti].color = vec4(text_color[0], text_color[1], text_color[2], 1.0);
            self.draw_texts[ti].text_style.font_size = 10.0;
            self.draw_texts[ti].draw_abs(cx, dvec2(rect.pos.x + LEFT_PAD + 18.0, y + 7.0), &entry.label);
            ti += 1;
        }

        DrawStep::done()
    }
}

impl ProjectBrowser {
    /// Scan the project root and populate the entry list.
    pub fn refresh(&mut self, project_root: &Path) {
        self.entries.clear();

        // Scan *.hum in root -> Piece entries
        if let Ok(rd) = std::fs::read_dir(project_root) {
            let mut pieces: Vec<BrowserEntry> = rd
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path().extension().map_or(false, |ext| ext == "hum")
                        && e.file_type().map_or(false, |ft| ft.is_file())
                })
                .map(|e| BrowserEntry {
                    label: e.file_name().to_string_lossy().to_string(),
                    path: e.path(),
                    kind: EntryKind::Piece,
                })
                .collect();
            pieces.sort_by(|a, b| a.label.cmp(&b.label));
            self.entries.extend(pieces);
        }

        // Scan instruments/*.hum -> Instrument entries
        let inst_dir = project_root.join("instruments");
        if inst_dir.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&inst_dir) {
                let mut insts: Vec<BrowserEntry> = rd
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path().extension().map_or(false, |ext| ext == "hum")
                            && e.file_type().map_or(false, |ft| ft.is_file())
                    })
                    .map(|e| BrowserEntry {
                        label: format!("instruments/{}", e.file_name().to_string_lossy()),
                        path: e.path(),
                        kind: EntryKind::Instrument,
                    })
                    .collect();
                insts.sort_by(|a, b| a.label.cmp(&b.label));
                self.entries.extend(insts);
            }
        }

        // Check hum.dict
        let dict_path = project_root.join("hum.dict");
        if dict_path.is_file() {
            self.entries.push(BrowserEntry {
                label: "hum.dict".to_string(),
                path: dict_path,
                kind: EntryKind::Dict,
            });
        }
    }

    /// Open a file in $EDITOR (fire-and-forget).
    fn open_file(path: &Path) {
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".into());
        let _ = std::process::Command::new(&editor)
            .arg(path)
            .spawn();
    }
}

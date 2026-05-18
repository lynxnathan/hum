use makepad_widgets::*;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// Default terminal grid dimensions.
const TERM_COLS: usize = 80;
const TERM_ROWS: usize = 24;
/// Maximum scrollback lines retained.
const MAX_SCROLLBACK: usize = 1000;
/// Default foreground color (Catppuccin Mocha text #cdd6f4).
const FG_DEFAULT: [f32; 4] = [0.804, 0.839, 0.957, 1.0];
/// Cursor color (Catppuccin Mocha accent #89b4fa).
const CURSOR_COLOR: [f32; 4] = [0.537, 0.706, 0.980, 0.85];

/// Regex pattern to strip ANSI escape sequences.
/// Matches ESC[ followed by parameters and a final letter.
fn strip_ansi(input: &str) -> String {
    // Simple state-machine strip: remove ESC[...letter sequences
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Check for CSI sequence: ESC [
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // Consume until a letter A-Za-z or end of input
                loop {
                    match chars.peek() {
                        Some(&c) if c.is_ascii_alphabetic() => {
                            chars.next(); // consume final letter
                            break;
                        }
                        Some(_) => {
                            chars.next();
                        }
                        None => break,
                    }
                }
            }
            // Also handle ESC ] (OSC sequences) — consume until BEL or ST
            else if chars.peek() == Some(&']') {
                chars.next(); // consume ']'
                loop {
                    match chars.next() {
                        Some('\x07') => break, // BEL
                        Some('\x1b') => {
                            // Check for ST (ESC \)
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                                break;
                            }
                        }
                        None => break,
                        _ => {}
                    }
                }
            }
            // Skip other ESC sequences (single char after ESC)
            else {
                chars.next();
            }
        } else {
            out.push(ch);
        }
    }
    out
}

live_design! {
    use link::theme::*;
    use link::widgets::*;

    TERM_BG = #1e1e2e

    pub TerminalPane = {{TerminalPane}} {
        width: Fill
        height: 200
        show_bg: true
        draw_bg: { color: (TERM_BG) }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct TerminalPane {
    #[redraw]
    #[live]
    draw_bg: DrawColor,

    #[walk]
    walk: Walk,

    #[layout]
    layout: Layout,

    /// Writer handle to send input to the PTY.
    #[rust]
    pty_writer: Option<Box<dyn Write + Send>>,

    /// Character grid: rows of (char, rgba) cells. Shared with reader thread.
    #[rust]
    char_grid: Arc<Mutex<Vec<Vec<(char, [f32; 4])>>>>,

    /// Pending keystroke buffer (for display echo tracking).
    #[rust]
    input_buf: String,

    /// Number of lines scrolled from bottom.
    #[rust]
    scroll_offset: usize,

    /// Whether this pane currently has keyboard focus.
    #[rust]
    has_focus: bool,

    /// Selection range for copy: (start_row, start_col, end_row, end_col).
    #[rust]
    selection: Option<(usize, usize, usize, usize)>,

    /// Selection drag active flag.
    #[rust]
    selecting: bool,

    /// Reusable DrawColor instances for character cells.
    #[rust]
    draw_cells: Vec<DrawColor>,

    /// Whether PTY has been started.
    #[rust]
    pty_started: bool,
}

impl TerminalPane {
    /// Spawn the PTY process and reader thread.
    fn start_pty(&mut self) {
        if self.pty_started {
            return;
        }
        self.pty_started = true;

        let pty_system = portable_pty::native_pty_system();
        let pty_pair = match pty_system.openpty(portable_pty::PtySize {
            rows: TERM_ROWS as u16,
            cols: TERM_COLS as u16,
            pixel_width: 0,
            pixel_height: 0,
        }) {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("[hum-gui] PTY open failed: {e}");
                return;
            }
        };

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
        let mut cmd = portable_pty::CommandBuilder::new(&shell);
        cmd.env("TERM", "dumb");
        // Set a simpler prompt to reduce ANSI noise
        cmd.env("PS1", "$ ");

        if let Err(e) = pty_pair.slave.spawn_command(cmd) {
            eprintln!("[hum-gui] PTY spawn failed: {e}");
            return;
        }

        // Take writer for stdin
        match pty_pair.master.take_writer() {
            Ok(writer) => {
                self.pty_writer = Some(writer);
            }
            Err(e) => {
                eprintln!("[hum-gui] PTY writer failed: {e}");
                return;
            }
        }

        // Spawn reader thread
        let grid = Arc::clone(&self.char_grid);
        let reader = match pty_pair.master.try_clone_reader() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[hum-gui] PTY reader failed: {e}");
                return;
            }
        };

        std::thread::spawn(move || {
            use std::io::Read;
            let mut reader = std::io::BufReader::new(reader);
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let text = String::from_utf8_lossy(&buf[..n]);
                        let clean = strip_ansi(&text);
                        if let Ok(mut grid) = grid.lock() {
                            for ch in clean.chars() {
                                match ch {
                                    '\n' => {
                                        grid.push(Vec::new());
                                    }
                                    '\r' => {
                                        // Carriage return — move to start of current line
                                        // (handled by not advancing, next chars overwrite)
                                    }
                                    '\t' => {
                                        // Expand tab to spaces
                                        let last = grid.last_mut();
                                        if let Some(row) = last {
                                            let spaces = 8 - (row.len() % 8);
                                            for _ in 0..spaces {
                                                row.push((' ', FG_DEFAULT));
                                            }
                                        }
                                    }
                                    '\x08' => {
                                        // Backspace
                                        if let Some(row) = grid.last_mut() {
                                            row.pop();
                                        }
                                    }
                                    c if c >= ' ' => {
                                        // Ensure there is at least one row
                                        if grid.is_empty() {
                                            grid.push(Vec::new());
                                        }
                                        if let Some(row) = grid.last_mut() {
                                            row.push((c, FG_DEFAULT));
                                        }
                                    }
                                    _ => {
                                        // Ignore other control characters
                                    }
                                }
                            }
                            // Cap scrollback
                            while grid.len() > MAX_SCROLLBACK {
                                grid.remove(0);
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }
}

impl Widget for TerminalPane {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        // Start PTY on first event if not already started
        if !self.pty_started {
            self.start_pty();
        }

        match event {
            Event::MouseDown(e) => {
                // Click to focus
                self.has_focus = true;
                // Start selection
                self.selecting = true;
                let (row, col) = self.pos_to_grid(e.abs);
                self.selection = Some((row, col, row, col));
                cx.redraw_all();
            }
            Event::MouseMove(e) => {
                if self.selecting {
                    let (row, col) = self.pos_to_grid(e.abs);
                    if let Some(ref mut sel) = self.selection {
                        sel.2 = row;
                        sel.3 = col;
                    }
                    cx.redraw_all();
                }
            }
            Event::MouseUp(_e) => {
                self.selecting = false;
            }
            Event::KeyDown(e) if self.has_focus => {
                // Check for Ctrl+C with selection (copy)
                if e.key_code == KeyCode::KeyC && e.modifiers.control {
                    if self.selection.is_some() {
                        let text = self.get_selected_text();
                        if !text.is_empty() {
                            cx.copy_to_clipboard(&text);
                            self.selection = None;
                            cx.redraw_all();
                            return;
                        }
                    }
                    // No selection: send Ctrl+C interrupt
                    self.write_to_pty(&[0x03]);
                    return;
                }

                // Ctrl+D
                if e.key_code == KeyCode::KeyD && e.modifiers.control {
                    self.write_to_pty(&[0x04]);
                    return;
                }

                // Ctrl+V paste
                if e.key_code == KeyCode::KeyV && e.modifiers.control {
                    // Paste not yet implemented (would need clipboard read)
                    return;
                }

                match e.key_code {
                    KeyCode::ReturnKey => {
                        self.write_to_pty(b"\r");
                        self.input_buf.clear();
                    }
                    KeyCode::Backspace => {
                        self.write_to_pty(&[0x7f]);
                        self.input_buf.pop();
                    }
                    KeyCode::Tab => {
                        self.write_to_pty(b"\t");
                    }
                    KeyCode::Escape => {
                        // Could toggle focus in plan 2
                    }
                    KeyCode::ArrowUp => {
                        self.write_to_pty(b"\x1b[A");
                    }
                    KeyCode::ArrowDown => {
                        self.write_to_pty(b"\x1b[B");
                    }
                    KeyCode::ArrowRight => {
                        self.write_to_pty(b"\x1b[C");
                    }
                    KeyCode::ArrowLeft => {
                        self.write_to_pty(b"\x1b[D");
                    }
                    _ => {
                        // Printable character
                        if let Some(ch) = Self::key_to_char(e) {
                            let mut buf = [0u8; 4];
                            let s = ch.encode_utf8(&mut buf);
                            self.write_to_pty(s.as_bytes());
                            self.input_buf.push(ch);
                        }
                    }
                }
                cx.redraw_all();
            }
            Event::Scroll(e) => {
                // Scroll the terminal view
                let delta = if e.scroll.y > 0.0 { 3 } else if e.scroll.y < 0.0 { -3i32 } else { 0 };
                if delta > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_add(delta as usize);
                } else {
                    self.scroll_offset = self.scroll_offset.saturating_sub((-delta) as usize);
                }
                // Clamp scroll offset
                if let Ok(grid) = self.char_grid.lock() {
                    let max_scroll = grid.len().saturating_sub(TERM_ROWS);
                    self.scroll_offset = self.scroll_offset.min(max_scroll);
                }
                cx.redraw_all();
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

        // Calculate cell dimensions
        let cell_w = rect.size.x / TERM_COLS as f64;
        let cell_h = rect.size.y / TERM_ROWS as f64;

        // Lock grid and render visible rows
        let grid = self.char_grid.lock().unwrap_or_else(|e| e.into_inner());
        let total_rows = grid.len();

        // Auto-scroll to bottom when not manually scrolled
        // (scroll_offset 0 means we're at the bottom)
        let visible_start = if total_rows > TERM_ROWS {
            total_rows - TERM_ROWS - self.scroll_offset.min(total_rows.saturating_sub(TERM_ROWS))
        } else {
            0
        };

        let visible_end = (visible_start + TERM_ROWS).min(total_rows);

        // Count cells needed
        let cells_needed = TERM_ROWS * TERM_COLS + 1; // +1 for cursor
        while self.draw_cells.len() < cells_needed {
            self.draw_cells.push(DrawColor::new_local(cx));
        }

        let mut cell_idx = 0;

        for (screen_row, grid_row_idx) in (visible_start..visible_end).enumerate() {
            let row = &grid[grid_row_idx];
            for col in 0..TERM_COLS {
                let (ch, color) = if col < row.len() {
                    row[col]
                } else {
                    (' ', FG_DEFAULT)
                };

                if ch != ' ' {
                    let x = rect.pos.x + col as f64 * cell_w;
                    let y = rect.pos.y + screen_row as f64 * cell_h;

                    // Draw character as a small colored rect (glyph approximation)
                    // In Makepad, true text rendering requires DrawText which needs
                    // font loading. For v1, we render colored blocks for non-space chars.
                    self.draw_cells[cell_idx].color = vec4(color[0], color[1], color[2], color[3]);
                    self.draw_cells[cell_idx].draw_abs(cx, Rect {
                        pos: dvec2(x + cell_w * 0.1, y + cell_h * 0.1),
                        size: dvec2(cell_w * 0.8, cell_h * 0.8),
                    });
                    cell_idx += 1;
                    if cell_idx >= cells_needed - 1 {
                        break;
                    }
                }
            }
        }

        // Draw cursor
        if self.has_focus {
            let cursor_row = if total_rows > 0 {
                (visible_end.saturating_sub(visible_start)).saturating_sub(1)
            } else {
                0
            };
            let cursor_col = if total_rows > 0 && visible_end > 0 {
                let last_visible = visible_end - 1;
                if last_visible < grid.len() {
                    grid[last_visible].len().min(TERM_COLS - 1)
                } else {
                    0
                }
            } else {
                0
            };

            let cx_pos = rect.pos.x + cursor_col as f64 * cell_w;
            let cy_pos = rect.pos.y + cursor_row as f64 * cell_h;

            if cell_idx < cells_needed {
                self.draw_cells[cell_idx].color = vec4(
                    CURSOR_COLOR[0],
                    CURSOR_COLOR[1],
                    CURSOR_COLOR[2],
                    CURSOR_COLOR[3],
                );
                self.draw_cells[cell_idx].draw_abs(cx, Rect {
                    pos: dvec2(cx_pos, cy_pos),
                    size: dvec2(cell_w * 0.15, cell_h),
                });
            }
        }

        DrawStep::done()
    }
}

impl TerminalPane {
    /// Write bytes to the PTY stdin.
    fn write_to_pty(&mut self, data: &[u8]) {
        if let Some(ref mut writer) = self.pty_writer {
            let _ = writer.write_all(data);
            let _ = writer.flush();
        }
    }

    /// Convert a KeyDown event to a printable char.
    fn key_to_char(e: &KeyEvent) -> Option<char> {
        // If the key event has a text representation, use the first char
        // Makepad KeyEvent doesn't have a direct `text` field in all versions,
        // so we map common keys manually
        let base = match e.key_code {
            KeyCode::KeyA => 'a', KeyCode::KeyB => 'b', KeyCode::KeyC => 'c',
            KeyCode::KeyD => 'd', KeyCode::KeyE => 'e', KeyCode::KeyF => 'f',
            KeyCode::KeyG => 'g', KeyCode::KeyH => 'h', KeyCode::KeyI => 'i',
            KeyCode::KeyJ => 'j', KeyCode::KeyK => 'k', KeyCode::KeyL => 'l',
            KeyCode::KeyM => 'm', KeyCode::KeyN => 'n', KeyCode::KeyO => 'o',
            KeyCode::KeyP => 'p', KeyCode::KeyQ => 'q', KeyCode::KeyR => 'r',
            KeyCode::KeyS => 's', KeyCode::KeyT => 't', KeyCode::KeyU => 'u',
            KeyCode::KeyV => 'v', KeyCode::KeyW => 'w', KeyCode::KeyX => 'x',
            KeyCode::KeyY => 'y', KeyCode::KeyZ => 'z',
            KeyCode::Key0 => '0', KeyCode::Key1 => '1', KeyCode::Key2 => '2',
            KeyCode::Key3 => '3', KeyCode::Key4 => '4', KeyCode::Key5 => '5',
            KeyCode::Key6 => '6', KeyCode::Key7 => '7', KeyCode::Key8 => '8',
            KeyCode::Key9 => '9',
            KeyCode::Space => ' ',
            KeyCode::Minus => '-',
            KeyCode::Equals => '=',
            KeyCode::LBracket => '[',
            KeyCode::RBracket => ']',
            KeyCode::Backslash => '\\',
            KeyCode::Semicolon => ';',
            KeyCode::Quote => '\'',
            KeyCode::Comma => ',',
            KeyCode::Period => '.',
            KeyCode::Slash => '/',
            KeyCode::Backtick => '`',
            _ => return None,
        };

        if e.modifiers.shift {
            Some(match base {
                'a'..='z' => (base as u8 - b'a' + b'A') as char,
                '1' => '!', '2' => '@', '3' => '#', '4' => '$', '5' => '%',
                '6' => '^', '7' => '&', '8' => '*', '9' => '(', '0' => ')',
                '-' => '_', '=' => '+', '[' => '{', ']' => '}', '\\' => '|',
                ';' => ':', '\'' => '"', ',' => '<', '.' => '>', '/' => '?',
                '`' => '~',
                c => c,
            })
        } else {
            Some(base)
        }
    }

    /// Convert an absolute screen position to a grid (row, col) coordinate.
    fn pos_to_grid(&self, _pos: DVec2) -> (usize, usize) {
        // Simplified: would need rect offset, but for now return (0,0)
        // Full implementation needs the last known rect stored
        (0, 0)
    }

    /// Extract selected text from the grid.
    fn get_selected_text(&self) -> String {
        let sel = match self.selection {
            Some(s) => s,
            None => return String::new(),
        };

        let (sr, sc, er, ec) = sel;
        let (start_row, start_col, end_row, end_col) = if sr < er || (sr == er && sc <= ec) {
            (sr, sc, er, ec)
        } else {
            (er, ec, sr, sc)
        };

        let grid = match self.char_grid.lock() {
            Ok(g) => g,
            Err(_) => return String::new(),
        };

        let mut result = String::new();
        for row_idx in start_row..=end_row.min(grid.len().saturating_sub(1)) {
            if row_idx >= grid.len() {
                break;
            }
            let row = &grid[row_idx];
            let col_start = if row_idx == start_row { start_col } else { 0 };
            let col_end = if row_idx == end_row {
                end_col.min(row.len())
            } else {
                row.len()
            };
            for col in col_start..col_end {
                if col < row.len() {
                    result.push(row[col].0);
                }
            }
            if row_idx < end_row {
                result.push('\n');
            }
        }
        result
    }
}

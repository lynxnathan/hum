use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gpui::*;

use crate::transport::{TransportCmd, TransportReply, SOCKET_PATH};

// ---------------------------------------------------------------------------
// Catppuccin Mocha palette
// ---------------------------------------------------------------------------
const BG: u32 = 0x1e1e2e;
const SURFACE: u32 = 0x24273a;
const OVERLAY: u32 = 0x313244;
const TEXT: u32 = 0xcdd6f4;
const GREEN: u32 = 0xa6e3a1;
const BLUE: u32 = 0x89b4fa;
const RED: u32 = 0xf38ba8;
const GRAY: u32 = 0x6c7086;

// ---------------------------------------------------------------------------
// Shared data between poll thread and GPUI view
// ---------------------------------------------------------------------------
#[derive(Clone, Default)]
struct WatchData {
    playing: bool,
    pos: f64,
    active: Vec<String>,
    amplitudes: HashMap<String, f32>,
    all_things: Vec<String>,
    error: Option<String>,
}

// ---------------------------------------------------------------------------
// WatchView
// ---------------------------------------------------------------------------
struct WatchView {
    data: Arc<Mutex<WatchData>>,
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------
impl Render for WatchView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.data.lock().unwrap().clone();

        div()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .size_full()
            .text_color(rgb(TEXT))
            .child(render_transport_bar(&data))
            .child(render_thing_list(&data))
            .child(render_status_line(&data))
    }
}

// ---------------------------------------------------------------------------
// Layout functions (free functions to avoid borrow issues)
// ---------------------------------------------------------------------------

fn render_transport_bar(data: &WatchData) -> Div {
    let pos_label = format!("{:.1}s", data.pos);

    let (state_text, state_color) = if data.playing {
        ("PLAYING", GREEN)
    } else {
        ("STOPPED", GRAY)
    };

    div()
        .flex()
        .flex_row()
        .w_full()
        .h(px(48.0))
        .bg(rgb(SURFACE))
        .items_center()
        .px(px(16.0))
        .child(
            // Left: position
            div()
                .flex()
                .flex_1()
                .text_xl()
                .text_color(rgb(TEXT))
                .child(pos_label),
        )
        .child(
            // Center: state badge
            div()
                .flex()
                .flex_1()
                .justify_center()
                .child(
                    div()
                        .px(px(12.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .bg(rgb(OVERLAY))
                        .text_color(rgb(state_color))
                        .text_sm()
                        .child(state_text),
                ),
        )
        .child(
            // Right: label
            div()
                .flex()
                .flex_1()
                .justify_end()
                .text_sm()
                .text_color(rgb(GRAY))
                .child("hum watch"),
        )
}

fn render_thing_list(data: &WatchData) -> Div {
    let things = if data.all_things.is_empty() {
        &data.active
    } else {
        &data.all_things
    };

    let rows: Vec<Div> = things
        .iter()
        .map(|name| render_thing_row(data, name))
        .collect();

    div()
        .flex()
        .flex_col()
        .flex_1()
        .px(px(12.0))
        .py(px(8.0))
        .gap(px(2.0))
        .children(rows)
}

fn render_thing_row(data: &WatchData, name: &str) -> Div {
    let is_active = data.active.contains(&name.to_string());
    let amp = data.amplitudes.get(name).copied().unwrap_or(0.0);
    let bar_w = (amp * 200.0).clamp(0.0, 200.0);

    let row_bg = if is_active { SURFACE } else { BG };
    let dot_color = if is_active { GREEN } else { OVERLAY };

    // VU color: blue < 0.5, green 0.5-0.85, red > 0.85
    let vu_color = if amp > 0.85 {
        RED
    } else if amp > 0.5 {
        GREEN
    } else {
        BLUE
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .h(px(36.0))
        .w_full()
        .px(px(8.0))
        .rounded(px(4.0))
        .bg(rgb(row_bg))
        .gap(px(12.0))
        // Thing name
        .child(
            div()
                .w(px(160.0))
                .text_sm()
                .text_color(rgb(TEXT))
                .child(name.to_string()),
        )
        // Active dot
        .child(
            div()
                .w(px(10.0))
                .h(px(10.0))
                .rounded(px(5.0))
                .bg(rgb(dot_color)),
        )
        // VU meter track + bar
        .child(
            div()
                .w(px(200.0))
                .h(px(8.0))
                .rounded(px(4.0))
                .bg(rgb(OVERLAY))
                .child(
                    div()
                        .w(px(bar_w))
                        .h(px(8.0))
                        .rounded(px(4.0))
                        .bg(rgb(vu_color)),
                ),
        )
}

fn render_status_line(data: &WatchData) -> Div {
    let (text, color) = if let Some(err) = &data.error {
        (err.clone(), RED)
    } else {
        ("polling at 20fps".to_string(), GRAY)
    };

    div()
        .flex()
        .h(px(24.0))
        .w_full()
        .items_center()
        .px(px(16.0))
        .text_xs()
        .text_color(rgb(color))
        .child(text)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------
pub fn run_watch() {
    // Force X11 on WSL2 (WSLg Wayland too old for gpui 0.2)
    std::env::remove_var("WAYLAND_DISPLAY");

    let shared = Arc::new(Mutex::new(WatchData::default()));

    // Spawn background poll thread with its own tokio runtime
    let poll_data = shared.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime for watch poll thread");

        rt.block_on(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;

                match send_cmd_safe().await {
                    Ok(TransportReply::Status {
                        playing,
                        pos,
                        active,
                        amplitudes,
                        ..
                    }) => {
                        let mut d = poll_data.lock().unwrap();
                        // Merge newly seen things into all_things
                        for name in &active {
                            if !d.all_things.contains(name) {
                                d.all_things.push(name.clone());
                            }
                        }
                        d.playing = playing;
                        d.pos = pos;
                        d.active = active;
                        d.amplitudes = amplitudes;
                        d.error = None;
                    }
                    Ok(_) => {
                        let mut d = poll_data.lock().unwrap();
                        d.error = Some("unexpected reply".to_string());
                    }
                    Err(_) => {
                        let mut d = poll_data.lock().unwrap();
                        d.error = Some("daemon not running".to_string());
                    }
                }
            }
        });
    });

    Application::new().run(move |cx: &mut App| {
        let view_data = shared.clone();
        let window_handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.0), px(0.0)),
                        size: size(px(900.0), px(500.0)),
                    })),
                    ..Default::default()
                },
                move |_window, cx| {
                    cx.new(move |_cx| WatchView { data: view_data })
                },
            )
            .unwrap();

        // Spawn a 20fps re-render loop on GPUI's executor.
        // App::spawn takes AsyncFnOnce(&mut AsyncApp) -> R
        cx.spawn(async move |cx: &mut AsyncApp| {
            loop {
                Timer::after(std::time::Duration::from_millis(50)).await;
                let result = window_handle.update(cx, |_view, window, _cx| {
                    window.refresh();
                });
                if result.is_err() {
                    break; // window closed
                }
            }
        })
        .detach();
    });
}

/// Send Status command to daemon without process::exit on failure.
async fn send_cmd_safe() -> anyhow::Result<TransportReply> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let stream = UnixStream::connect(SOCKET_PATH).await?;
    let (reader, mut writer) = stream.into_split();

    let cmd_json = serde_json::to_string(&TransportCmd::Status)?;
    writer.write_all(cmd_json.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;

    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();
    buf_reader.read_line(&mut line).await?;

    let reply: TransportReply = serde_json::from_str(line.trim())?;
    Ok(reply)
}

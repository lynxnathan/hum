---
phase: 08-gpui-dashboard
plan: 2
type: execute
wave: 2
depends_on: [08-PLAN-1.md]
files_modified:
  - Cargo.toml
  - src/main.rs
  - src/watch.rs
autonomous: false
requirements: [TUI-01, TUI-02, TUI-03]

must_haves:
  truths:
    - "`hum watch` opens a GPUI window (800x500, dark background)"
    - "Active things are highlighted in the timeline area"
    - "Per-thing VU meter bars update in real time from amplitude data"
    - "Transport bar shows current position and playing/stopped state"
    - "Window polls daemon at ~20fps and re-renders automatically"
  artifacts:
    - path: "src/watch.rs"
      provides: "WatchView (Render impl), daemon poll loop, GPUI Application entry"
      min_lines: 120
    - path: "Cargo.toml"
      provides: "gpui = '0.2' dependency"
      contains: "gpui"
    - path: "src/main.rs"
      provides: "'watch' arm in run_cli spawning GPUI application"
      contains: "watch"
  key_links:
    - from: "src/watch.rs poll loop"
      to: "src/transport.rs send_cmd(Status)"
      via: "tokio::time::interval at 50ms, updates WatchView fields via cx.update"
    - from: "src/watch.rs WatchView::render"
      to: "WatchView.status fields"
      via: "GPUI div/flex layout reading playing, pos, active, amplitudes"
---

<objective>
Build the `hum watch` GPUI window. A separate module (`src/watch.rs`) contains the GPUI application: opens a window, polls the daemon's Status command at 20fps, and renders a live timeline with active-thing highlighting, per-thing VU meter bars, and a transport status bar.

Purpose: Delivers TUI-01, TUI-02, TUI-03 — the complete dashboard experience.
Output: `hum watch` subcommand opens a GPU-accelerated live dashboard window.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/08-gpui-dashboard/08-1-SUMMARY.md

@src/transport.rs
@src/main.rs

<interfaces>
<!-- GPUI 0.2 API (confirmed by spike at /tmp/gpui-spike/src/main.rs) -->

```rust
// Entry point
Application::new().run(|cx: &mut App| {
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(800.0), px(500.0)),
            })),
            ..Default::default()
        },
        |_window, cx| cx.new(|_cx| MyView { ... }),
    ).unwrap();
});

// Render trait
impl Render for MyView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e2e))
            .size_full()
            .child(...)
    }
}

// Layout primitives (Tailwind-like)
div().flex().flex_row().flex_col()
div().w_full().h(px(40.0))
div().bg(rgb(0xHHHHHH))
div().text_color(rgb(0xHHHHHH)).text_sm().text_xl()
div().px(px(12.0)).py(px(8.0))
div().child("text").child(another_div)
div().children(iter)  // Vec<impl IntoElement>

// cx.update on a Model<T> or Entity<T>
cx.update(|cx| { view_handle.update(cx, |view, cx| { view.field = val; cx.notify(); }); });
```

<!-- Transport types (from src/transport.rs after Plan 1) -->
```rust
pub enum TransportCmd { Status, PlayFrom { pos: f64 }, ... }

pub enum TransportReply {
    Status {
        playing: bool,
        pos: f64,
        active: Vec<String>,
        solo: Vec<String>,
        mute: Vec<String>,
        amplitudes: HashMap<String, f32>,
    },
    ...
}

pub async fn send_cmd(cmd: TransportCmd) -> Result<TransportReply>
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add gpui dependency and watch subcommand wiring</name>
  <files>Cargo.toml, src/main.rs</files>
  <action>
**Cargo.toml:** Add to `[dependencies]`:
```toml
gpui = "0.2"
```

**src/main.rs — add `mod watch;` and `watch` CLI arm:**

Add `mod watch;` at the top of main.rs alongside other mod declarations.

In `run_cli`, add a new arm before the `_ =>` catch-all:
```rust
Some("watch") => {
    watch::run_watch();
    return Ok(());
}
```

`watch::run_watch()` is synchronous (GPUI runs its own event loop). Do NOT wrap in tokio — call it directly and return.

Also update the usage string in the `_ =>` arm to include `watch`.
  </action>
  <verify>cargo check 2>&amp;1 | grep -E "^error" | head -10</verify>
  <done>
- `cargo check` passes (watch module will be a stub at this point if created as empty)
- `src/main.rs` has `mod watch;` and `Some("watch")` arm
  </done>
</task>

<task type="auto">
  <name>Task 2: Build WatchView — GPUI window with timeline, VU meters, transport bar</name>
  <files>src/watch.rs</files>
  <action>
Create `src/watch.rs`. This module is 100% GPUI — no tokio runtime of its own (GPUI uses its own executor).

**Data model:**
```rust
struct WatchView {
    playing: bool,
    pos: f64,
    active: Vec<String>,
    amplitudes: std::collections::HashMap<String, f32>,
    all_things: Vec<String>,  // ordered list from last Status reply
    error: Option<String>,    // "daemon not running" etc
}
```

**`run_watch()` entry point:**

```rust
pub fn run_watch() {
    // Force X11 on WSL2 (WSLg Wayland too old for gpui 0.2)
    std::env::remove_var("WAYLAND_DISPLAY");

    Application::new().run(|cx: &mut App| {
        let win = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.0), px(0.0)),
                    size: size(px(900.0), px(500.0)),
                })),
                ..Default::default()
            },
            |_window, cx| cx.new(|_cx| WatchView {
                playing: false,
                pos: 0.0,
                active: vec![],
                amplitudes: Default::default(),
                all_things: vec![],
                error: None,
            }),
        ).unwrap();

        // Spawn background poll task using GPUI's executor
        cx.spawn(|mut cx| async move {
            let mut interval = std::time::Duration::from_millis(50);
            loop {
                tokio::time::sleep(interval).await;
                // send_cmd is async, use gpui's background executor
                match crate::transport::send_cmd(crate::transport::TransportCmd::Status).await {
                    Ok(crate::transport::TransportReply::Status {
                        playing, pos, active, amplitudes, all_things_hint, ..
                    }) => {
                        let _ = win.update(&mut cx, |view, cx| {
                            // Merge all_things: union of active + existing known things
                            for name in &active {
                                if !view.all_things.contains(name) {
                                    view.all_things.push(name.clone());
                                }
                            }
                            view.playing = playing;
                            view.pos = pos;
                            view.active = active;
                            view.amplitudes = amplitudes;
                            view.error = None;
                            cx.notify();
                        });
                    }
                    Err(_) => {
                        let _ = win.update(&mut cx, |view, cx| {
                            view.error = Some("daemon not running".to_string());
                            cx.notify();
                        });
                    }
                    _ => {}
                }
            }
        }).detach();
    });
}
```

NOTE: GPUI 0.2's `cx.spawn` gives access to an async context. Use `gpui::Timer::after` if `tokio::time::sleep` is not available in the GPUI executor context. The exact async primitive depends on what GPUI 0.2 exposes — check imports and use `gpui::Timer::after(duration).await` as the preferred sleep.

**`Render` implementation:**

Layout: flex column, full size, dark background `rgb(0x1e1e2e)`.

Three sections stacked vertically:

1. **Transport bar** (h: 48px, flex row):
   - Left: position display `format!("{:.1}s", self.pos)` in bright white
   - Center: state badge — "PLAYING" in green `rgb(0xa6e3a1)` or "STOPPED" in gray `rgb(0x6c7086)`
   - Right: "hum watch" label in dim text `rgb(0x6c7086)`
   - Background: slightly lighter than body `rgb(0x24273a)`

2. **Timeline / thing list** (flex: 1, scrollable region):
   For each thing in `self.all_things` (or `self.active` if all_things is empty), render a row (h: 36px, flex row, gap):
   - Thing name label (w: 160px, text `rgb(0xcdd6f4)`)
   - Active indicator dot: filled circle `rgb(0xa6e3a1)` if in `self.active`, else dim `rgb(0x313244)`
   - VU meter bar: a background track (w: 200px, h: 8px, `rgb(0x313244)`) with a filled bar inside width = `amplitude * 200px`. Color: `rgb(0x89b4fa)` (blue) for low, `rgb(0xa6e3a1)` (green) for mid, `rgb(0xf38ba8)` (red) if > 0.85.
   - Active rows: slightly highlighted row background `rgb(0x2a2d3e)`

3. **Status line** (h: 24px):
   - If `self.error.is_some()`: show error in red `rgb(0xf38ba8)`
   - Else: show "polling at 20fps" in dim gray

**Color palette (Catppuccin Mocha — matches PROJECT.md aesthetic):**
- Background: `0x1e1e2e`
- Surface: `0x24273a`
- Overlay: `0x313244`
- Text: `0xcdd6f4`
- Green: `0xa6e3a1`
- Blue: `0x89b4fa`
- Red: `0xf38ba8`
- Gray: `0x6c7086`

**VU bar width calculation:**
```rust
let amp = self.amplitudes.get(name).copied().unwrap_or(0.0);
let bar_w = (amp * 200.0).clamp(0.0, 200.0);
// render: div().w(px(bar_w)).h(px(8.0)).bg(color)
```

**GPUI 0.2 import block at top of file:**
```rust
use gpui::*;
use crate::transport::{send_cmd, TransportCmd, TransportReply};
```
  </action>
  <verify>cargo build 2>&amp;1 | grep -E "^error" | head -20</verify>
  <done>
- `cargo build` succeeds
- `WAYLAND_DISPLAY="" DISPLAY=:0 ./target/debug/hum-rt watch` opens an 900x500 dark window
- Window shows transport bar, thing rows, and VU meter placeholders
- No panic on startup
  </done>
</task>

<task type="checkpoint:human-verify" gate="blocking">
  <what-built>
GPUI dashboard window: `hum watch` opens a live window polling the daemon at 20fps. Shows transport bar (position + play state), per-thing rows with active highlighting, and VU meter bars.
  </what-built>
  <how-to-verify>
1. Start daemon: `./target/debug/hum-rt` (from hum project dir)
2. Start playback: `./target/debug/hum-rt play`
3. Launch dashboard: `WAYLAND_DISPLAY="" DISPLAY=:0 ./target/debug/hum-rt watch`
4. Verify window opens (800x500 dark window)
5. Verify transport bar shows current position updating in real time
6. Verify active things appear highlighted in the timeline rows
7. Test seek: `./target/debug/hum-rt play from 1m30s` — position should jump to 90s
8. Test stop: `./target/debug/hum-rt stop` — badge should switch to STOPPED
9. Kill daemon — window should show "daemon not running" error state
  </how-to-verify>
  <resume-signal>Type "approved" if the dashboard works, or describe what's broken</resume-signal>
</task>

</tasks>

<verification>
```
cargo build 2>&1 | grep "^error" | wc -l   # must be 0
cargo test 2>&1 | tail -5                   # existing tests still pass
WAYLAND_DISPLAY="" DISPLAY=:0 ./target/debug/hum-rt watch  # window opens
```
</verification>

<success_criteria>
1. `hum watch` opens a GPUI window with timeline + VU meters + transport bar
2. Active things highlighted in real time
3. Position counter advances while playing
4. VU meter bars populated (may be 0 if amplitude polling not yet wired in daemon)
5. `hum play from 1m30s` is atomic (from Plan 1)
6. All pre-existing tests still pass
</success_criteria>

<output>
After completion, create `.planning/phases/08-gpui-dashboard/08-2-SUMMARY.md`
</output>

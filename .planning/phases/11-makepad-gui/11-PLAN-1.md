---
phase: 11-makepad-gui
plan: 1
type: execute
wave: 1
depends_on: []
files_modified:
  - Cargo.toml
  - src/bin/gui/main.rs
  - src/bin/gui/app.rs
  - src/bin/gui/transport_client.rs
autonomous: true
requirements: [MKPD-01, MKPD-05]

must_haves:
  truths:
    - "Running `hum gui` opens a Makepad window on WSL2"
    - "Transport bar shows Play/Stop/Seek controls and current playback position"
    - "Clicking Play sends TransportCmd::Play to the daemon via unix socket"
    - "Clicking Stop sends TransportCmd::Stop to the daemon"
    - "Playback position updates every 100ms from Status polling"
  artifacts:
    - path: "src/bin/gui/main.rs"
      provides: "Binary entry point calling app_main()"
    - path: "src/bin/gui/app.rs"
      provides: "Makepad App struct with live_design! layout — window, transport bar"
    - path: "src/bin/gui/transport_client.rs"
      provides: "Background thread polling /tmp/hum.sock, exposing Arc<Mutex<GuiState>>"
  key_links:
    - from: "src/bin/gui/app.rs"
      to: "src/bin/gui/transport_client.rs"
      via: "Arc<Mutex<GuiState>> read in handle_event Signal tick"
      pattern: "gui_state\\.lock"
    - from: "src/bin/gui/transport_client.rs"
      to: "/tmp/hum.sock"
      via: "std::os::unix::net::UnixStream JSON newline protocol"
      pattern: "UnixStream::connect.*hum\\.sock"
---

<objective>
Bootstrap hum-gui as a separate Makepad binary. Opens a window on WSL2, connects to the daemon via unix socket, and renders a functional transport bar (play/stop/seek/position display).

Purpose: Establishes the binary structure, Makepad app skeleton, and daemon communication layer that all subsequent GUI plans build on.
Output: `cargo run --bin hum-gui` opens a Makepad window with a working transport bar.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md

<interfaces>
<!-- Transport protocol — JSON newline-delimited on /tmp/hum.sock -->
<!-- Send: {"cmd":"play"}\n  {"cmd":"stop"}\n  {"cmd":"status"}\n  {"cmd":"seek","pos":4.2}\n -->

From src/transport.rs:
```rust
pub const SOCKET_PATH: &str = "/tmp/hum.sock";

pub enum TransportCmd { Play, Stop, Status, Seek { pos: f64 }, Solo { thing: String }, Mute { thing: String } }

pub enum TransportReply {
    Ack,
    Status {
        playing: bool,
        pos: f64,
        active: Vec<String>,
        solo: Vec<String>,
        mute: Vec<String>,
        amplitudes: HashMap<String, f32>,
    },
    Error { message: String },
}
```

From /tmp/makepad-spike/src/lib.rs (confirmed working pattern):
```rust
use makepad_widgets::*;
live_design! {
    use link::theme::*;
    use link::widgets::*;
    App = {{App}} {
        ui: <Window> { body = <View> { ... } }
    }
}
app_main!(App);
#[derive(Live, LiveHook)]
pub struct App { #[live] ui: WidgetRef }
impl LiveRegister for App { fn live_register(cx: &mut Cx) { makepad_widgets::live_design(cx); } }
impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add hum-gui binary target and makepad-widgets dep</name>
  <files>Cargo.toml, src/bin/gui/main.rs</files>
  <action>
In Cargo.toml:
- Add `makepad-widgets = "1.0"` to [dependencies]
- Add a [[bin]] section:
  ```toml
  [[bin]]
  name = "hum-gui"
  path = "src/bin/gui/main.rs"
  ```

Create src/bin/gui/main.rs:
```rust
// Entry point for hum-gui — Makepad requires app logic in a pub fn app_main()
// called from main, not directly in main(), due to its event loop architecture.
mod app;
mod transport_client;

pub fn main() {
    app::app_main();
}
```

Note: Makepad's `app_main!()` macro generates a `pub fn app_main()` function in the module where it's invoked. main.rs delegates to app::app_main() which is what the macro generates. This is the exact pattern from the confirmed spike.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build --bin hum-gui 2>&1 | tail -5</automated>
  </verify>
  <done>cargo build --bin hum-gui succeeds with no errors. makepad-widgets resolves from crates.io.</done>
</task>

<task type="auto">
  <name>Task 2: transport_client — daemon polling thread</name>
  <files>src/bin/gui/transport_client.rs</files>
  <action>
Create a blocking background thread (std::thread::spawn, NOT tokio) that polls the daemon every 100ms and stores the latest status in a shared Arc<Mutex<GuiState>>.

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use serde_json;

#[derive(Default, Clone)]
pub struct GuiState {
    pub playing: bool,
    pub pos: f64,
    pub active: Vec<String>,
    pub solo: Vec<String>,
    pub mute: Vec<String>,
    pub amplitudes: HashMap<String, f32>,
    pub connected: bool,
}

pub fn start_polling(state: Arc<Mutex<GuiState>>) {
    std::thread::spawn(move || loop {
        match poll_once(&state) {
            Ok(_) => {}
            Err(_) => {
                state.lock().unwrap().connected = false;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    });
}

fn poll_once(state: &Arc<Mutex<GuiState>>) -> anyhow::Result<()> {
    let mut stream = UnixStream::connect("/tmp/hum.sock")?;
    stream.write_all(b"{\"cmd\":\"status\"}\n")?;
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let reply: serde_json::Value = serde_json::from_str(line.trim())?;
    // parse reply and update GuiState
    // ...
    Ok(())
}

pub fn send_cmd(cmd: &str) -> anyhow::Result<()> {
    let mut stream = UnixStream::connect("/tmp/hum.sock")?;
    stream.write_all(format!("{}\n", cmd).as_bytes())?;
    Ok(())
}
```

Implement the full Status reply parsing: read `ok` field, if "status" extract playing/pos/active/solo/mute/amplitudes into GuiState, set connected=true.

send_cmd() is used by the GUI for play/stop/seek — fire and forget (no reply needed for Ack commands).
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build --bin hum-gui 2>&1 | tail -5</automated>
  </verify>
  <done>transport_client compiles. GuiState and start_polling exported correctly.</done>
</task>

<task type="auto">
  <name>Task 3: Makepad app with transport bar UI</name>
  <files>src/bin/gui/app.rs</files>
  <action>
Create the Makepad app with a 3-zone vertical layout: spectral zone (placeholder, grey box), arrangement zone (placeholder, grey box), transport bar (bottom, functional).

Transport bar contains:
- Play button (label "PLAY", id: play_btn)
- Stop button (label "STOP", id: stop_btn)
- Position display label (id: pos_label, shows "0.00s" updating from GuiState)
- Status indicator dot (green=playing, grey=stopped, id: status_dot)

In handle_event:
1. On Event::Signal (from Makepad's timer/signal system) or NextFrame: read GuiState arc, update pos_label text, update status_dot color.
2. On button clicks (WidgetAction::ButtonClicked): call transport_client::send_cmd with the appropriate JSON.

Use `cx.start_interval(0.1)` in after_new_from_doc to trigger periodic redraws.

For play button click:
```rust
send_cmd(r#"{"cmd":"play"}"#).ok();
```
For stop:
```rust
send_cmd(r#"{"cmd":"stop"}"#).ok();
```

Store Arc<Mutex<GuiState>> in the App struct:
```rust
pub struct App {
    #[live] ui: WidgetRef,
    #[rust] gui_state: Arc<Mutex<GuiState>>,
}
```

In after_new_from_doc, call transport_client::start_polling(Arc::clone(&self.gui_state)).

Color scheme: dark background #1e1e2e (Catppuccin Mocha base), text #cdd6f4, accent #89b4fa.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build --bin hum-gui 2>&1 | tail -5</automated>
  </verify>
  <done>cargo build --bin hum-gui succeeds. Running it opens a Makepad window with transport bar visible (play/stop buttons, position label). With daemon running, buttons send commands and position updates.</done>
</task>

</tasks>

<verification>
1. `cargo build --bin hum-gui` compiles without errors
2. `cargo run --bin hum-gui` opens a window on WSL2 (DRI3 warnings in stderr are harmless)
3. With daemon running (`hum-rt` in another terminal): click Play → piece starts, position ticks up, click Stop → piece stops
4. Without daemon: window opens, status shows disconnected (no crash)
</verification>

<success_criteria>
- hum-gui binary exists and compiles
- Makepad window opens on WSL2
- Transport bar shows play/stop buttons and live position from daemon
- play/stop clicks round-trip to daemon correctly
</success_criteria>

<output>
After completion, create `.planning/phases/11-makepad-gui/11-1-SUMMARY.md`
</output>

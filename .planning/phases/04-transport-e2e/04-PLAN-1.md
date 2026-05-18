---
phase: 04-transport-e2e
plan: 1
type: execute
wave: 1
depends_on: []
files_modified:
  - src/transport.rs
  - src/main.rs
  - src/events.rs
  - Cargo.toml
autonomous: true
requirements:
  - XPORT-01
  - XPORT-02
  - XPORT-03
  - XPORT-06
  - XPORT-07

must_haves:
  truths:
    - "hum play connects to the unix socket and daemon starts playback"
    - "hum stop connects to the unix socket and daemon stops + frees all nodes"
    - "hum status returns current pos, playing state, active things"
    - "hum play from <t> sends a seek command with the parsed time in seconds"
    - "hum loop <s> <e> sends a loop command with two time bounds"
  artifacts:
    - path: "src/transport.rs"
      provides: "Unix socket server (daemon side) and client dispatch, JSON protocol types"
      exports: ["TransportCmd", "TransportReply", "start_socket_server", "send_cmd"]
    - path: "src/events.rs"
      provides: "DaemonEvent::Transport variant"
      contains: "Transport(TransportCmd)"
  key_links:
    - from: "src/main.rs"
      to: "src/transport.rs"
      via: "start_socket_server(tx.clone())"
      pattern: "start_socket_server"
    - from: "src/transport.rs"
      to: "src/events.rs"
      via: "tx.send(DaemonEvent::Transport(cmd))"
      pattern: "DaemonEvent::Transport"
---

<objective>
Add the unix socket transport layer: a JSON newline-delimited protocol between the `hum` CLI and the `hum-rt` daemon. The daemon listens on `/tmp/hum.sock`; the CLI connects, sends a command, reads the reply, and exits.

Purpose: All transport commands (play, stop, status, seek, loop, solo, mute) share this channel. This plan wires the channel and implements the basic commands (play, stop, status, seek, loop). Solo/mute are wired in Plan 2.

Output: `src/transport.rs` with server + client functions, updated `DaemonEvent`, updated `main.rs` event loop branch.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md

<interfaces>
<!-- Existing types the executor builds against. No exploration needed. -->

From src/events.rs:
```rust
pub enum DaemonEvent {
    FileChanged(PathBuf),
    Tick(f64),
    // ADD: Transport(TransportCmd)
}
```

From src/state.rs:
```rust
pub struct StateStore {
    pub desired: Option<Piece>,
    pub actual: ActualState,
    pub playback_pos: f64,
    // Plan 2 will add: playing, solo_set, mute_set, loop_range
}
impl StateStore {
    pub fn active_things(&self, pos: f64) -> IndexMap<String, &ThingDef>
}
```

From src/main.rs — event loop shape:
```rust
tokio::select! {
    Some(event) = rx.recv() => { match event { FileChanged, Tick } }
    _ = tokio::signal::ctrl_c() => { client.free_all_nodes().await; break; }
}
// ADD: DaemonEvent::Transport(cmd) arm
```

From src/timeline.rs:
```rust
pub async fn run_ticker(tx: Sender<DaemonEvent>, start_pos: f64)
// Returns nothing; ticker task is spawned. To seek, abort handle and respawn.
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: transport.rs — protocol types, socket server, CLI client</name>
  <files>src/transport.rs, Cargo.toml</files>
  <action>
Create `src/transport.rs` with:

**Protocol types** (serde JSON, newline-delimited):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum TransportCmd {
    Play,
    Stop,
    Status,
    Seek { pos: f64 },       // seconds
    Loop { start: f64, end: f64 },
    Solo { thing: String },
    Mute { thing: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "ok", rename_all = "snake_case")]
pub enum TransportReply {
    Ack,
    Status {
        playing: bool,
        pos: f64,
        active: Vec<String>,
        solo: Vec<String>,
        mute: Vec<String>,
    },
    Error { message: String },
}
```

**Socket path constant:**
```rust
pub const SOCKET_PATH: &str = "/tmp/hum.sock";
```

**Server function** (daemon side):
```rust
pub async fn start_socket_server(tx: tokio::sync::mpsc::Sender<crate::events::DaemonEvent>)
```
- Remove stale socket file if exists before binding
- `tokio::net::UnixListener::bind(SOCKET_PATH)`
- Spawn a task that loops: `accept()` → spawn per-connection task
- Per-connection task: read one newline-terminated line, deserialize as `TransportCmd`, send `DaemonEvent::Transport(cmd)` on `tx`, wait for reply on a `oneshot` channel (pass sender with the cmd), write reply as JSON + newline, close connection
- To pass oneshot back: change DaemonEvent::Transport to carry `(TransportCmd, oneshot::Sender<TransportReply>)`

**Client function** (CLI side):
```rust
pub async fn send_cmd(cmd: TransportCmd) -> anyhow::Result<TransportReply>
```
- `tokio::net::UnixStream::connect(SOCKET_PATH)` — if fails, print "hum-rt is not running" and exit 1
- Write `serde_json::to_string(&cmd)? + "\n"`
- Read one line back, deserialize as `TransportReply`
- Return the reply

Add `serde_json = "1"` to `[dependencies]` in Cargo.toml.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build 2>&1 | tail -5</automated>
  </verify>
  <done>src/transport.rs compiles with TransportCmd, TransportReply, start_socket_server, send_cmd exported. Cargo.toml has serde_json.</done>
</task>

<task type="auto">
  <name>Task 2: Wire DaemonEvent::Transport into main.rs + add hum CLI subcommands</name>
  <files>src/events.rs, src/main.rs</files>
  <action>
**src/events.rs** — add Transport variant:
```rust
use tokio::sync::oneshot;
use crate::transport::{TransportCmd, TransportReply};

pub enum DaemonEvent {
    FileChanged(PathBuf),
    Tick(f64),
    Transport(TransportCmd, oneshot::Sender<TransportReply>),
}
```

**src/main.rs** — add `mod transport;` and CLI dispatch before daemon startup:

At top of `main()`, before connecting to scsynth, parse CLI args:
```rust
let args: Vec<String> = std::env::args().collect();
if args.len() > 1 {
    // CLI client mode — dispatch command and exit
    run_cli(&args[1..]).await?;
    return Ok(());
}
```

Add `async fn run_cli(args: &[String]) -> anyhow::Result<()>` that maps subcommand strings to `TransportCmd` and calls `transport::send_cmd(cmd).await`, then prints the reply:
- `"play"` → `TransportCmd::Play`
- `"stop"` → `TransportCmd::Stop`
- `"status"` → `TransportCmd::Status` → print pos, playing, active list
- `"play" "from" "<t>"` → parse `<t>` as seconds (strip trailing 's' if present, else parse as f64), → `TransportCmd::Seek { pos }`
- `"loop" "<s>" "<e>"` → parse both as seconds → `TransportCmd::Loop { start, end }`
- `"solo" "<thing>"` → `TransportCmd::Solo { thing }`
- `"mute" "<thing>"` → `TransportCmd::Mute { thing }`
- unknown → print usage and exit 1

**Daemon event loop** — add the Transport arm and call `start_socket_server` before the loop:

In daemon path (after ticker spawn), add:
```rust
tokio::spawn(transport::start_socket_server(tx.clone()));
```

In the `tokio::select!` loop, add:
```rust
Some(event) = rx.recv() => {
    match event {
        DaemonEvent::FileChanged(path) => { ... }
        DaemonEvent::Tick(pos) => { ... }
        DaemonEvent::Transport(cmd, reply_tx) => {
            handle_transport(&mut state, &mut client, cmd, reply_tx).await;
        }
    }
}
```

Add `async fn handle_transport(state, client, cmd, reply_tx)`:
- `TransportCmd::Play` → set `state.playing = true`, restart ticker from `state.playback_pos` (store handle), reconcile, send `TransportReply::Ack`
- `TransportCmd::Stop` → set `state.playing = false`, abort ticker handle, `client.free_all_nodes()`, clear `state.actual.nodes`, send `TransportReply::Ack`
- `TransportCmd::Status` → collect active things, send `TransportReply::Status { playing, pos, active, solo, mute }`
- `TransportCmd::Seek { pos }` → set `state.playback_pos = pos`, restart ticker, reconcile, send `Ack`
- `TransportCmd::Loop { start, end }` → set `state.loop_range = Some((start, end))`, send `Ack`
- Solo/Mute → delegate (will be implemented in Plan 2 state fields; for now, store in state and send Ack)

The ticker task handle needs to be stored so it can be aborted on Stop/Seek. Add `ticker_handle: Option<tokio::task::JoinHandle<()>>` as a local variable in `main()`, passed into handlers.

Since StateStore doesn't yet have `playing`/`loop_range`/`solo_set`/`mute_set` fields (Plan 2 adds those), add them directly in this task OR coordinate: add the fields to StateStore here as part of the wiring. Add to `src/state.rs`:
```rust
pub playing: bool,
pub loop_range: Option<(f64, f64)>,
pub solo_set: std::collections::HashSet<String>,
pub mute_set: std::collections::HashSet<String>,
```
And initialize in `StateStore::new()` with `playing: false, loop_range: None, solo_set: HashSet::new(), mute_set: HashSet::new()`.

The ticker should only send Ticks when `state.playing = true`. Since the ticker runs in a separate task and can't read state, implement this in `handle_tick`: at top of `handle_tick`, return early if `!state.playing`.

For loop wrapping: at top of `handle_tick`, if `state.loop_range = Some((s, e))` and `pos >= e`, seek back to `s` by aborting and restarting ticker.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build 2>&1 | tail -10</automated>
  </verify>
  <done>cargo build passes. `hum-rt` with no args starts daemon with socket listener. `hum-rt play` (as CLI mode) connects to socket. All transport arms compile.</done>
</task>

</tasks>

<verification>
Run full test suite after both tasks: `cargo test 2>&1 | tail -20`. All existing tests must still pass. Build must be clean (no warnings that indicate unused imports or dead code in the new module).
</verification>

<success_criteria>
- `cargo build` succeeds with no errors
- `cargo test` passes all existing tests
- `src/transport.rs` exists with TransportCmd, TransportReply, start_socket_server, send_cmd
- DaemonEvent has Transport variant
- StateStore has playing, loop_range, solo_set, mute_set fields
- handle_transport dispatches all 7 command variants
</success_criteria>

<output>
After completion, create `.planning/phases/04-transport-e2e/04-1-SUMMARY.md` using the summary template.
</output>

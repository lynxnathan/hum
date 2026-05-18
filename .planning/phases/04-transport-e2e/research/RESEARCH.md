# Phase 4: Transport + E2E - Research

**Researched:** 2026-03-20
**Domain:** Tokio Unix socket IPC, CLI transport controls, solo/mute runtime state, seek/loop timeline, clap subcommands
**Confidence:** HIGH — all patterns drawn from existing codebase analysis + well-established tokio/clap idioms

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None — all decisions are delegated to Claude's discretion.

### Claude's Discretion
- Unix socket protocol — JSON lines vs custom wire format
- CLI binary — same binary with subcommands or separate client binary
- Solo/mute state persistence — how to survive file reloads
- Seek implementation — how to reset timeline and recompute active things
- Loop implementation — how timeline wraps between start/end
- Status output format — human-readable vs JSON, what info to show

### Deferred Ideas (OUT OF SCOPE)
None
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| XPORT-01 | play command via unix socket | UnixListener + TransportCommand::Play → set playback_state = Playing, restart ticker |
| XPORT-02 | stop command via unix socket | TransportCommand::Stop → set playback_state = Stopped, free all nodes |
| XPORT-03 | status command (current time, active things) | TransportCommand::Status → serialize StateStore fields into JSON response over socket |
| XPORT-04 | solo per-thing (survives file reload) | SoloMuteState layer in StateStore, consulted in active_things() filtering |
| XPORT-05 | mute per-thing (survives file reload) | Same SoloMuteState layer |
| XPORT-06 | seek (play from time) | TransportCommand::Seek(f64) → restart ticker at new start_pos, recompute active set |
| XPORT-07 | loop between two points | StateStore::loop_range: Option<(f64,f64)>, ticker wraps when pos >= end |
| E2E-01 | multi-thing timeline plays correctly | Existing reconciler + timeline sufficient; E2E test with real piece.hum |
| E2E-02 | editing piece.hum while playing changes sound within ~1s | Existing file watcher + reconciler sufficient; measure round-trip latency |
| E2E-03 | editing .scd while playing changes sound via crossfade | Existing handle_scd_change hot-swap path sufficient; verify crossfade works |
</phase_requirements>

---

## Summary

Phase 4 adds the last missing layer: a CLI-to-daemon IPC channel and runtime transport state. The existing codebase (Phases 1-3) already has a complete reconcile-on-tick loop, a file watcher, and OSC bridge. Phase 4 needs only three additions: (1) a `transport.rs` Unix socket server that injects `TransportCommand` events into the existing mpsc channel; (2) runtime solo/mute state in `StateStore` that `active_things()` consults but file reloads cannot overwrite; and (3) controllable timeline state (playing/stopped/looping/seeking) that the existing ticker can be replaced or redirected to honor.

The E2E scenarios (E2E-01 through E2E-03) are not new code — they are integration tests of what already exists. The existing hot-swap path in `handle_scd_change` and the file-watcher reconcile path already deliver E2E-02 and E2E-03. What's missing is a test harness and a real `piece.hum` to drive them.

**Primary recommendation:** Same binary with clap subcommands. `hum` runs as daemon with no subcommand; `hum play`, `hum stop`, etc. connect to the Unix socket as a client, send one JSON line, and print the response. JSON-lines over Unix socket is the correct protocol — simple, debuggable with `nc`, and requires no extra crate beyond `serde_json`.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio (already present) | 1 (full) | UnixListener, UnixStream, mpsc | Already a dependency; `tokio::net::UnixListener` is stable |
| clap | 4.x | CLI subcommands and argument parsing | De facto standard Rust CLI library; derive macros minimize boilerplate |
| serde_json | 1 | JSON-lines protocol over Unix socket | Already have serde; json lines are the simplest line-framed protocol |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio::io::BufReader + lines() | (tokio stdlib) | Async line-by-line socket reads | Read one JSON command per connection |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| JSON-lines | Raw newline-delimited text ("play\n") | JSON wins: status response naturally needs structured data (pos, active list); worth the tiny overhead |
| Same binary | Separate `hum-ctl` binary | Same binary is simpler: one `cargo install`, one binary on PATH, subcommand dispatch is trivial with clap |
| UnixListener | Named pipe / TCP loopback | Unix socket is the correct IPC primitive on Linux; standard for daemons (systemd, mpd, etc.) |

**Installation:**
```bash
# clap and serde_json are the only new dependencies
# In Cargo.toml [dependencies]:
# clap = { version = "4", features = ["derive"] }
# serde_json = "1"
```

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── main.rs          # clap dispatch: daemon mode vs client subcommand
├── transport.rs     # NEW: UnixListener server + TransportCommand enum
├── client.rs        # NEW: thin Unix socket client (used by CLI subcommands)
├── events.rs        # ADD: DaemonEvent::Transport(TransportCommand)
├── state.rs         # ADD: playback_state, loop_range, solo/mute sets
├── timeline.rs      # ADD: controllable ticker (restart on seek/play)
├── ...              # unchanged: parser, reconciler, osc, watcher, scd
```

### Pattern 1: Unix Socket Server with tokio

**What:** `transport.rs` binds a `UnixListener` at a known path (e.g. `/tmp/hum-rt.sock`), accepts one connection at a time (or spawns per-connection tasks), reads one JSON line, sends one JSON response, closes connection. Each connection maps to one `TransportCommand` injected into the mpsc channel.

**When to use:** This is the standard daemon IPC pattern on Linux. One connection per command keeps the server stateless and dead connections never block the daemon.

**Example:**
```rust
// transport.rs
use tokio::net::UnixListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub async fn run_transport_server(
    socket_path: &str,
    tx: tokio::sync::mpsc::Sender<DaemonEvent>,
) -> anyhow::Result<()> {
    // Remove stale socket from previous run
    let _ = std::fs::remove_file(socket_path);
    let listener = UnixListener::bind(socket_path)?;
    loop {
        let (stream, _) = listener.accept().await?;
        let tx = tx.clone();
        tokio::spawn(async move {
            let (reader, mut writer) = stream.into_split();
            let mut lines = BufReader::new(reader).lines();
            if let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<TransportRequest>(&line) {
                    Ok(req) => {
                        // Send command, await response via oneshot
                        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                        let _ = tx.send(DaemonEvent::Transport(req.cmd, resp_tx)).await;
                        if let Ok(resp) = resp_rx.await {
                            let _ = writer.write_all(
                                (serde_json::to_string(&resp).unwrap() + "\n").as_bytes()
                            ).await;
                        }
                    }
                    Err(e) => {
                        let _ = writer.write_all(
                            format!("{{\"error\":\"{e}\"}}\n").as_bytes()
                        ).await;
                    }
                }
            }
        });
    }
}
```

**Key detail:** Use a `oneshot::channel` in the event payload so the event loop can send a response back to the socket handler task. This avoids shared state.

### Pattern 2: Response-capable DaemonEvent

**What:** Add a `Transport` variant to `DaemonEvent` carrying both the command and a `oneshot::Sender<TransportResponse>`. The event loop processes the command, builds a response, and sends it back through the oneshot. The socket handler task is suspended on `resp_rx.await`.

**Example:**
```rust
// events.rs — updated
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum DaemonEvent {
    FileChanged(PathBuf),
    Tick(f64),
    Transport(TransportCommand, oneshot::Sender<TransportResponse>),
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum TransportCommand {
    Play,
    Stop,
    Status,
    Seek { pos: f64 },
    Loop { start: f64, end: f64 },
    LoopOff,
    Solo { thing: String },
    Unsolo { thing: String },
    Mute { thing: String },
    Unmute { thing: String },
}

#[derive(Debug, serde::Serialize)]
#[serde(tag = "ok", rename_all = "snake_case")]
pub enum TransportResponse {
    Ok,
    Status {
        playing: bool,
        pos: f64,
        active: Vec<String>,
        muted: Vec<String>,
        soloed: Vec<String>,
        loop_range: Option<(f64, f64)>,
    },
    Error { message: String },
}
```

### Pattern 3: Solo/Mute State in StateStore

**What:** Add two `HashSet<String>` fields to `StateStore`: `muted` and `soloed`. These are consulted by `active_things()` — if any soloed things exist, only soloed things are active; muted things are filtered out regardless. File reloads only update `desired` — they never touch `muted`/`soloed`.

**When to use:** This is the only correct placement. Keeping solo/mute in StateStore (not in the reconciler or elsewhere) means the existing `reconcile_now()` path automatically respects it on every file reload.

**Example:**
```rust
// state.rs — additions
use std::collections::HashSet;

pub struct StateStore {
    pub desired: Option<Piece>,
    pub actual: ActualState,
    pub playback_pos: f64,
    pub playback_state: PlaybackState,
    pub loop_range: Option<(f64, f64)>,
    pub muted: HashSet<String>,
    pub soloed: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing,
}

impl StateStore {
    pub fn active_things(&self, pos: f64) -> IndexMap<String, &ThingDef> {
        if self.playback_state == PlaybackState::Stopped {
            return IndexMap::new();
        }
        let Some(piece) = &self.desired else {
            return IndexMap::new();
        };
        let has_solo = !self.soloed.is_empty();
        piece
            .iter()
            .filter(|(name, thing)| {
                // Timeline filter
                if !is_active(thing, pos) { return false; }
                // Solo filter: if any thing is soloed, only soloed things pass
                if has_solo && !self.soloed.contains(*name) { return false; }
                // Mute filter
                if self.muted.contains(*name) { return false; }
                true
            })
            .map(|(name, thing)| (name.clone(), thing))
            .collect()
    }
}
```

**Critical:** `active_things()` already drives every reconcile call. Inserting the solo/mute filter here means it is automatically respected on file reload, seek, tick, and every other trigger — no other code needs to know about solo/mute.

### Pattern 4: Controllable Timeline (Seek + Stop)

**What:** The current `run_ticker()` spawns a task that runs forever at 50ms. It cannot be stopped or seeked. Replace it with a controllable design: hold a `JoinHandle` on the spawned task and abort it when stop/seek is received, then spawn a new ticker from the new start position.

**Example:**
```rust
// main.rs — event loop state
let mut ticker_handle: Option<tokio::task::JoinHandle<()>> = None;

fn start_ticker(
    tx: &Sender<DaemonEvent>,
    start_pos: f64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(timeline::run_ticker(tx.clone(), start_pos))
}

// In handle_transport():
TransportCommand::Play => {
    if let Some(h) = ticker_handle.take() { h.abort(); }
    state.playback_state = PlaybackState::Playing;
    ticker_handle = Some(start_ticker(&tx, state.playback_pos));
    reconcile_now(&mut state, &mut client).await;
    TransportResponse::Ok
}
TransportCommand::Stop => {
    if let Some(h) = ticker_handle.take() { h.abort(); }
    state.playback_state = PlaybackState::Stopped;
    state.playback_pos = 0.0;
    // Free all nodes
    let _ = client.free_all_nodes().await;
    state.actual = ActualState::default();
    TransportResponse::Ok
}
TransportCommand::Seek { pos } => {
    if let Some(h) = ticker_handle.take() { h.abort(); }
    state.playback_pos = pos;
    if state.playback_state == PlaybackState::Playing {
        ticker_handle = Some(start_ticker(&tx, pos));
    }
    // Free all nodes and reconcile from new position
    let _ = client.free_all_nodes().await;
    state.actual = ActualState::default();
    reconcile_now(&mut state, &mut client).await;
    TransportResponse::Ok
}
```

**Why abort-and-respawn instead of channels:** The ticker is a simple monotonic counter. Pausing it via channel would require it to become stateful. Abort-and-respawn is simpler, has zero state to synchronize, and the ticker's only job is to send ticks — restarting it from a new origin is cheap and correct.

### Pattern 5: Loop Implementation

**What:** When `loop_range = Some((start, end))` is set, `handle_tick` wraps the position. When `pos >= end`, the event loop resets `playback_pos = start`, aborts the ticker, and spawns a new one from `start`. This causes the timeline to replay from the loop start.

**Example:**
```rust
// handle_tick addition
async fn handle_tick(state: &mut StateStore, client: &mut ScsynthClient, pos: f64,
    tx: &Sender<DaemonEvent>, ticker_handle: &mut Option<JoinHandle<()>>) {
    // Loop wrap
    if let Some((loop_start, loop_end)) = state.loop_range {
        if pos >= loop_end {
            if let Some(h) = ticker_handle.take() { h.abort(); }
            state.playback_pos = loop_start;
            // Free all nodes so reconciler re-adds at loop_start
            let _ = client.free_all_nodes().await;
            state.actual = ActualState::default();
            reconcile_now(state, client).await;
            *ticker_handle = Some(tokio::spawn(timeline::run_ticker(tx.clone(), loop_start)));
            return;
        }
    }
    // ... existing tick logic
}
```

### Pattern 6: CLI Subcommand Dispatch (same binary)

**What:** `main()` parses clap args first. If a transport subcommand is present (`play`, `stop`, `status`, etc.), open a `UnixStream`, write one JSON line, read one JSON line, print, exit. If no subcommand, run as daemon.

**Example:**
```rust
// main.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "hum")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start playback
    Play,
    /// Stop playback
    Stop,
    /// Show playback status
    Status,
    /// Seek to position
    Seek { pos: f64 },
    /// Loop between two positions
    Loop { start: f64, end: f64 },
    /// Solo a thing
    Solo { thing: String },
    /// Mute a thing
    Mute { thing: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(cmd) => client::send_command(cmd).await,
        None => daemon::run().await,
    }
}
```

### Anti-Patterns to Avoid

- **Storing solo/mute in the reconciler:** The reconciler is a pure diff function. Routing solo/mute through it would make it stateful and hard to test. Keep solo/mute in StateStore where `active_things()` can filter before the reconciler ever sees the set.
- **Keeping the ticker running during Stop:** If the ticker keeps ticking during Stop, `handle_tick` fires 20 times/second doing nothing. Abort it on Stop. Resume it on Play.
- **Using TCP loopback instead of Unix socket:** TCP adds overhead and opens a port. Unix socket is local-only, faster, and the correct primitive for this use case.
- **Blocking the event loop on socket I/O:** The transport server must run in its own spawned task and communicate via the existing mpsc channel. Never do socket reads inside the event loop's `tokio::select!`.
- **Re-adding all nodes on seek without freeing first:** Seek must `free_all_nodes()` + clear `actual` before `reconcile_now()`. Otherwise nodes from the pre-seek position continue running.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON parsing | Custom text protocol parser | serde_json | Edge cases in escaping, numbers, nested values |
| CLI argument parsing | Manual argv parsing | clap derive | Automatic help text, error messages, type coercion |
| Unix socket line framing | Custom length-prefix protocol | `tokio::io::BufReader::lines()` | Newline framing is sufficient for single-command-per-connection model |

---

## Common Pitfalls

### Pitfall 1: Stale Socket File on Daemon Restart
**What goes wrong:** `UnixListener::bind()` fails with "address already in use" if the previous daemon crashed without cleaning up the socket file.
**Why it happens:** Unix socket files persist on disk after the binding process exits.
**How to avoid:** Always call `std::fs::remove_file(socket_path)` (ignoring the error) before `UnixListener::bind()`.
**Warning signs:** Daemon fails to start with "Os { code: 98, kind: AddrInUse }".

### Pitfall 2: Solo/Mute Wiped on File Reload
**What goes wrong:** User mutes "bass-drone", edits piece.hum, saves — bass-drone starts playing again.
**Why it happens:** Handler for FileChanged overwrites `state.desired` and calls `reconcile_now()`, which sees bass-drone as desired-active and adds it back.
**How to avoid:** Solo/mute state lives in `StateStore.muted`/`StateStore.soloed`, which are fields separate from `desired`. `active_things()` filters them out. File reload only updates `desired`.
**Warning signs:** Mute stops working after any file save.

### Pitfall 3: Ticker JoinHandle Not Aborted on Seek
**What goes wrong:** After seek, two ticker tasks are running — the old one (continuing from old position) and the new one (starting from seek target). The event loop receives ticks from both. Active-set comparisons jump chaotically.
**Why it happens:** `tokio::spawn()` returns a handle that must be explicitly aborted or awaited.
**How to avoid:** Store `ticker_handle: Option<JoinHandle<()>>` on the event loop. Always abort the current handle before spawning a new one.
**Warning signs:** Playback position seems to jump or oscillate after seek.

### Pitfall 4: Seek Without Clearing Actual State
**What goes wrong:** Seek to t=0 when "lead" was playing at t=15. "lead" keeps playing because `actual.nodes` still has it. Reconciler sees no diff (both desired-active and actual have it). No Remove is emitted.
**Why it happens:** `reconcile_now` only diffs — it doesn't know about position discontinuities.
**How to avoid:** On seek (and on stop), call `client.free_all_nodes()` and reset `state.actual = ActualState::default()` before calling `reconcile_now()`.

### Pitfall 5: Blocking Event Loop on Socket Response
**What goes wrong:** Daemon hangs for the duration of a status request if the CLI client is slow to read.
**Why it happens:** If the transport command handling writes directly to the socket from the event loop.
**How to avoid:** Use the oneshot pattern. The event loop sends the response into the oneshot channel and immediately continues. The spawned socket handler task does the write.

---

## Code Examples

### Verified patterns from tokio docs:

### UnixListener accept loop
```rust
// Source: tokio docs (tokio::net::UnixListener)
use tokio::net::UnixListener;
let _ = std::fs::remove_file("/tmp/hum-rt.sock");
let listener = UnixListener::bind("/tmp/hum-rt.sock")?;
loop {
    let (stream, _addr) = listener.accept().await?;
    tokio::spawn(handle_connection(stream, tx.clone()));
}
```

### JSON-lines request/response
```rust
// Request from CLI client
#[derive(serde::Serialize, serde::Deserialize)]
struct TransportRequest {
    cmd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pos: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thing: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<f64>,
}

// Client side (hum play → sends {"cmd":"play"}\n, reads response)
use tokio::net::UnixStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
let mut stream = UnixStream::connect("/tmp/hum-rt.sock").await?;
let req = serde_json::to_string(&TransportRequest { cmd: "play".into(), ..Default::default() })?;
stream.write_all((req + "\n").as_bytes()).await?;
let (reader, _writer) = stream.split();
let mut lines = BufReader::new(reader).lines();
if let Some(line) = lines.next_line().await? {
    println!("{}", line);
}
```

### Status response example
```json
{
  "ok": "status",
  "playing": true,
  "pos": 12.34,
  "active": ["space-crackle", "bass-drone"],
  "muted": ["pad"],
  "soloed": [],
  "loop_range": null
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Ticker runs unconditionally | Ticker spawned/aborted per play/stop/seek | Phase 4 | Enables seek without duplicate tick streams |
| `active_things()` returns raw timeline filter | `active_things()` additionally filters solo/mute | Phase 4 | Solo/mute respected by all reconcile paths automatically |
| `DaemonEvent` has 2 variants | `DaemonEvent` gains `Transport` variant with oneshot | Phase 4 | CLI commands flow through existing event loop unchanged |

---

## Open Questions

1. **Socket path configurability**
   - What we know: `/tmp/hum-rt.sock` is fine for a single-user local daemon
   - What's unclear: Should the socket path be in `Config`? Needed if user runs multiple pieces simultaneously.
   - Recommendation: Hardcode for Phase 4. Add config key in a future phase if multi-piece emerges as a use case.

2. **Status output format for humans vs tooling**
   - What we know: JSON is machine-readable; plain text is more readable in a terminal
   - What's unclear: Does `hum status` pretty-print or emit raw JSON?
   - Recommendation: Pretty-print by default with `--json` flag for machine consumption. Implement pretty-print as a simple manual format over the JSON struct.

3. **Mute via OSC vs via reconciler exclusion**
   - What we know: Two valid approaches: (a) exclude muted things from `active_things()` so reconciler removes them, or (b) keep them in active set but send `/n_set nodeId amp 0` to silence
   - What's unclear: Approach (a) causes a node to be freed then re-added on unmute (audible gap). Approach (b) requires `set_param` support and a `vol` control in every SynthDef.
   - Recommendation: Use approach (a) (exclusion from active set) for Phase 4. The re-add gap is acceptable for v1. Approach (b) is a v1.x polish item requiring SynthDef convention.

---

## Sources

### Primary (HIGH confidence)
- Existing codebase (`src/main.rs`, `src/events.rs`, `src/state.rs`, `src/reconciler.rs`, `src/timeline.rs`, `src/osc/bridge.rs`) — direct analysis of existing patterns and integration points
- tokio docs: `tokio::net::UnixListener`, `tokio::sync::oneshot` — standard tokio IPC primitives
- clap docs: derive API for subcommand dispatch — standard Rust CLI pattern

### Secondary (MEDIUM confidence)
- Architecture research (`.planning/research/ARCHITECTURE.md`) — confirmed Unix socket in component table, confirmed TransportCommand event bus pattern
- Feature research (`.planning/research/FEATURES.md`) — confirmed solo/mute conflict with reconciler, confirmed status query requirements

### Tertiary (LOW confidence)
- None — all findings grounded in codebase analysis or verified library patterns

---

## Metadata

**Confidence breakdown:**
- Unix socket IPC: HIGH — direct tokio stdlib, no unknowns
- Protocol design (JSON-lines): HIGH — simple, well-understood, serde_json already adjacent
- Solo/mute in StateStore: HIGH — follows directly from existing `active_things()` pattern
- Seek/loop via abort-and-respawn: HIGH — tokio JoinHandle::abort() is stable
- CLI subcommands (clap): HIGH — standard Rust pattern

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (tokio 1.x and clap 4.x are stable; no churn expected)

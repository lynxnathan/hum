# Phase 1: OSC Bridge - Research

**Researched:** 2026-03-20
**Domain:** Rust OSC/UDP bridge to SuperCollider scsynth — rosc, tokio UdpSocket, SC server protocol, config
**Confidence:** HIGH (rosc API from docs.rs; SC OSC protocol from official SC docs 3.14.1; tokio from official docs)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None — all Phase 1 implementation decisions are at Claude's discretion.

### Claude's Discretion
- **Connection behavior** — Startup sequence, retry strategy if scsynth isn't running, blocking vs background connect, health checks
- **Config design** — SCSYNTH_HOST env var and/or config file, CLI flags, defaults (localhost:57110), config file format and location
- **Error reporting** — Log levels, stderr output, exit codes, what's fatal (can't connect) vs recoverable (transient UDP failure)
- **Node ID strategy** — Sequential vs random node IDs, thing_name → node_id registry design, cleanup on disconnect/crash

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| OSC-01 | hum-rt connects to scsynth via UDP on configurable host:port | tokio UdpSocket::bind + connect pattern; verified |
| OSC-02 | scsynth host is config-driven (SCSYNTH_HOST env var or config file, default localhost:57110) | std::env::var + optional TOML file pattern; figment for layered config |
| OSC-03 | hum-rt loads SynthDefs via /d_recv and waits for /sync before instantiation | SC /d_recv + /sync + /synced protocol; rosc OscType::Blob for bytes |
| OSC-04 | hum-rt creates synth instances via /s_new with node ID registry | SC /s_new exact args; node ID 1000+ sequential allocation |
| OSC-05 | hum-rt updates synth parameters via /n_set | SC /n_set exact args; OscType::String for name, OscType::Float for value |
| OSC-06 | hum-rt frees synth nodes via /n_free (no orphaned nodes) | SC /n_free; HashMap<String, i32> registry; SIGINT handler for cleanup |
</phase_requirements>

---

## Summary

The OSC bridge is the lowest-level component of hum-rt: a thin Rust module that encodes typed OSC commands into UDP datagrams and sends them to scsynth, plus a receive loop for awaiting specific replies (`/synced`, `/done`). The rosc 0.11 crate covers all encoding/decoding. tokio's `UdpSocket` handles async send/recv without blocking the event loop.

The critical design constraint is the `/d_recv` → `/sync` → `/synced` handshake. scsynth's SynthDef loading is async; sending `/s_new` before `/synced` silently fails. This must be baked into the bridge from the start — not retrofitted. A companion known bug (SC #4411) means `/done` on `/d_recv` does not confirm success; only the absence of `/s_new` failure or an explicit `/d_query` can verify the definition loaded.

Config is simple: `SCSYNTH_HOST` env var takes precedence over a `~/.config/hum/config.toml` file (or `./hum.toml`), which takes precedence over the compiled-in default `127.0.0.1:57110`. No need for a heavy config crate — `std::env::var` + `serde` + `toml` covers this cleanly.

**Primary recommendation:** Use rosc 0.11 + tokio UdpSocket. Implement a `ScsynthClient` struct owning the socket and a `HashMap<String, i32>` node registry. Use `/sync` with a unique ID (monotonic counter) after every `/d_recv` to gate `/s_new`. Never trust `/done` from `/d_recv` as a success signal.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rosc | 0.11.4 | OSC 1.0 encode/decode | Pure Rust, no C deps, only serious maintained Rust OSC crate; 350k+ downloads |
| tokio | 1.44.0 | Async runtime + UdpSocket | Project-wide async runtime; UdpSocket is zero-copy, `&self` in Tokio 1.x (no mut required) |
| anyhow | 1.0 | Error propagation in bridge | `?` chaining; OscError → anyhow::Error is clean |
| thiserror | 2.0 | Typed OscBridgeError | Separate error type for sync timeout, encode failure, no SynthDef |
| tracing | 0.1 | Structured logging | Log every OSC send/recv at DEBUG level; INFO for load/free events |
| toml | 0.8 | Config file parsing | Lightweight; parse `~/.config/hum/config.toml` into Config struct |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio::time | (tokio sub) | Timeout on /synced wait | `tokio::time::timeout(Duration, recv_loop).await` guards sync waits |
| std::env | stdlib | SCSYNTH_HOST env var | `std::env::var("SCSYNTH_HOST")` — no external crate needed |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| rosc 0.11 | async-osc | async-osc wraps rosc + tokio but adds abstraction; for this project, direct rosc gives full control of encoding; avoid |
| hand-rolled config | figment | figment is excellent but heavyweight for single env-var-plus-file config; toml + std::env is sufficient |
| manual sync ID | uuid | sync IDs are just incrementing u32; no need for uuid |

**Installation (Cargo.toml additions for Phase 1):**
```toml
rosc = "0.11"
toml = "0.8"
# tokio, anyhow, thiserror, tracing already in stack
```

---

## Architecture Patterns

### Recommended Module Structure (Phase 1 scope only)
```
src/
├── main.rs               # startup: config load, ScsynthClient::connect
├── config.rs             # Config struct, load() — env var + optional TOML file
└── osc/
    ├── mod.rs            # pub use; module entry point
    ├── bridge.rs         # ScsynthClient struct, socket, node registry, sync counter
    ├── commands.rs       # typed OscCommand enum + fn to_osc_packet()
    └── error.rs          # OscBridgeError (SyncTimeout, EncodeError, SendError, etc.)
```

### Pattern 1: ScsynthClient — Owned Socket + Registry

**What:** A struct that owns the UDP socket, the node ID registry (`HashMap<String, i32>`), and a monotonic sync counter. All OSC operations are methods on this struct.

**When to use:** Always — single-owner state, no Arc/Mutex needed since it lives in the event loop task.

**Example:**
```rust
// src/osc/bridge.rs
// Source: derived from tokio UdpSocket docs (docs.rs/tokio/latest/tokio/net/struct.UdpSocket.html)
use std::collections::HashMap;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};
use rosc::{OscPacket, OscMessage, OscType, encoder, decoder};
use anyhow::{Result, bail};

pub struct ScsynthClient {
    socket: UdpSocket,
    nodes: HashMap<String, i32>,   // thing_name → node_id
    next_node_id: i32,             // start at 1000, increment
    next_sync_id: i32,             // monotonic counter for /sync
}

impl ScsynthClient {
    pub async fn connect(addr: &str) -> Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(addr).await?;
        Ok(Self {
            socket,
            nodes: HashMap::new(),
            next_node_id: 1000,
            next_sync_id: 1,
        })
    }

    fn alloc_node_id(&mut self) -> i32 {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }

    fn alloc_sync_id(&mut self) -> i32 {
        let id = self.next_sync_id;
        self.next_sync_id += 1;
        id
    }
}
```

### Pattern 2: Async /sync Handshake After /d_recv

**What:** Send `/d_recv <bytes>`, immediately follow with `/sync <id>`, then await `/synced <id>` in a recv loop before proceeding with `/s_new`. This is the canonical SC external-client pattern.

**Why /sync not just /done:** `/done /d_recv` fires when loading *begins processing* in some SC versions, and is unreliable on failure (SC bug #4411). `/sync` fires when ALL preceding async commands have completed — it is the correct gate.

**Example:**
```rust
// src/osc/bridge.rs
// Source: SC Server Command Reference (docs.supercollider.online/Reference/Server-Command-Reference.html)
// Source: SC Sync-Async guide (doc.sccode.org/Guides/Sync-Async.html)

impl ScsynthClient {
    pub async fn load_synthdef(&mut self, synthdef_bytes: Vec<u8>) -> Result<()> {
        // 1. Send /d_recv with SynthDef bytes as OSC Blob
        self.send_message("/d_recv", vec![OscType::Blob(synthdef_bytes)]).await?;

        // 2. Send /sync <id> immediately after
        let sync_id = self.alloc_sync_id();
        self.send_message("/sync", vec![OscType::Int(sync_id)]).await?;

        // 3. Await /synced <id> with timeout
        self.await_synced(sync_id, Duration::from_secs(5)).await?;

        Ok(())
    }

    async fn await_synced(&self, expected_id: i32, deadline: Duration) -> Result<()> {
        let mut buf = vec![0u8; 4096];
        timeout(deadline, async {
            loop {
                let n = self.socket.recv(&mut buf).await?;
                // rosc 0.11: decoder::decode_udp returns (remaining, OscPacket)
                if let Ok((_, OscPacket::Message(msg))) = decoder::decode_udp(&buf[..n]) {
                    if msg.addr == "/synced" {
                        if let Some(OscType::Int(id)) = msg.args.first() {
                            if *id == expected_id {
                                return Ok(());
                            }
                        }
                    }
                    // Log and discard other messages (e.g., /done, /status.reply)
                    tracing::debug!("osc recv (awaiting /synced): {} {:?}", msg.addr, msg.args);
                }
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("timeout waiting for /synced {}", expected_id))?
    }
}
```

### Pattern 3: Node Lifecycle (/s_new, /n_set, /n_free)

**What:** Allocate sequential node IDs starting at 1000 (scsynth reserves 0–999 for groups). Track `thing_name → node_id`. Free before reassigning.

**Example:**
```rust
// Source: SC Server Command Reference — /s_new, /n_set, /n_free
impl ScsynthClient {
    /// Creates a new synth. Returns the allocated node ID.
    pub async fn new_synth(&mut self, thing_name: &str, synthdef_name: &str) -> Result<i32> {
        // Free existing node for this thing if present
        if let Some(&old_id) = self.nodes.get(thing_name) {
            self.free_node_by_id(old_id).await?;
        }
        let node_id = self.alloc_node_id();
        // /s_new <defName:String> <id:Int> <addAction:Int> <target:Int> [<param:String> <val:Float>...]
        // addAction 0 = add to head of group 1 (default group)
        self.send_message("/s_new", vec![
            OscType::String(synthdef_name.to_string()),
            OscType::Int(node_id),
            OscType::Int(0),  // addAction: head
            OscType::Int(1),  // target: default group
        ]).await?;
        self.nodes.insert(thing_name.to_string(), node_id);
        tracing::info!("s_new: {} -> node {}", thing_name, node_id);
        Ok(node_id)
    }

    /// Sets a named control parameter on a running synth.
    pub async fn set_param(&self, thing_name: &str, param: &str, value: f32) -> Result<()> {
        let &node_id = self.nodes.get(thing_name)
            .ok_or_else(|| anyhow::anyhow!("no node for thing: {}", thing_name))?;
        // /n_set <nodeID:Int> <paramName:String> <value:Float>
        self.send_message("/n_set", vec![
            OscType::Int(node_id),
            OscType::String(param.to_string()),
            OscType::Float(value),
        ]).await?;
        Ok(())
    }

    /// Frees a synth node by thing name. Removes from registry.
    pub async fn free_node(&mut self, thing_name: &str) -> Result<()> {
        if let Some(node_id) = self.nodes.remove(thing_name) {
            self.free_node_by_id(node_id).await?;
        }
        Ok(())
    }

    async fn free_node_by_id(&self, node_id: i32) -> Result<()> {
        // /n_free <nodeID:Int>
        self.send_message("/n_free", vec![OscType::Int(node_id)]).await
    }

    /// Frees ALL tracked nodes. Call on daemon shutdown.
    pub async fn free_all_nodes(&mut self) -> Result<()> {
        let ids: Vec<i32> = self.nodes.values().copied().collect();
        for id in ids {
            self.free_node_by_id(id).await?;
        }
        self.nodes.clear();
        Ok(())
    }
}
```

### Pattern 4: OSC Send Helper

**What:** Central method that encodes OscMessage via rosc and sends over the UDP socket.

**Example:**
```rust
// Source: rosc encoder docs (docs.rs/rosc/latest/rosc/encoder/fn.encode.html)
impl ScsynthClient {
    async fn send_message(&self, addr: &str, args: Vec<OscType>) -> Result<()> {
        let packet = OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args,
        });
        let bytes = encoder::encode(&packet)
            .map_err(|e| anyhow::anyhow!("OSC encode error: {:?}", e))?;
        self.socket.send(&bytes).await?;
        tracing::debug!("osc send: {} ({} bytes)", addr, bytes.len());
        Ok(())
    }
}
```

### Pattern 5: Config Loading

**What:** Priority: env var `SCSYNTH_HOST` > `./hum.toml` > `~/.config/hum/config.toml` > compiled default.

**Example:**
```rust
// src/config.rs
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_scsynth_host")]
    pub scsynth_host: String,
}

fn default_scsynth_host() -> String {
    "127.0.0.1:57110".to_string()
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        // 1. Load from file if present (local > home)
        let mut cfg: Config = load_from_file().unwrap_or_default();

        // 2. Env var overrides file
        if let Ok(host) = std::env::var("SCSYNTH_HOST") {
            cfg.scsynth_host = host;
        }

        tracing::info!("scsynth host: {}", cfg.scsynth_host);
        Ok(cfg)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self { scsynth_host: default_scsynth_host() }
    }
}

fn load_from_file() -> Option<Config> {
    // Try ./hum.toml first, then ~/.config/hum/config.toml
    let candidates = [
        std::path::PathBuf::from("hum.toml"),
        dirs::config_dir()?.join("hum/config.toml"),
    ];
    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(cfg) = toml::from_str(&content) {
                tracing::debug!("config loaded from {}", path.display());
                return Some(cfg);
            }
        }
    }
    None
}
```

### Anti-Patterns to Avoid

- **Sending /s_new immediately after /d_recv without /sync:** Race condition. scsynth drops the /s_new silently. Always interpose /sync.
- **Treating /done from /d_recv as success confirmation:** SC bug #4411 — /done fires even on SynthDef load failure. Never gate logic on this signal.
- **Hardcoding 172.29.224.1 in config files:** WSL2 gateway IP changes on restart. SCSYNTH_HOST env var must be set at runtime, not committed to config.
- **Single UdpSocket for both send and recv without coordination:** If a background task is calling recv simultaneously with the sync-wait loop, messages will be stolen. Solution: use a single recv loop with a dispatch channel, or ensure only one recv caller at a time (simplest for Phase 1: recv is only called during sync-wait, not concurrently).
- **Starting node IDs at 1 or 0:** scsynth reserves node IDs 0 (root group) and 1 (default group). Start at 1000.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| OSC 1.0 binary encoding | Custom serializer | rosc encoder::encode | OSC alignment rules, type tags, bundle framing are subtle; rosc is tested |
| OSC 1.0 binary decoding | Custom parser | rosc decoder::decode_udp | Same; OSC padding/alignment bugs are common |
| Async UDP event loop | Raw std::net threads | tokio UdpSocket | Non-blocking async recv; cancel-safe with tokio::select! |
| Config layering | Custom env+file merge | std::env + toml crate | Three sources, straightforward merge; no framework needed |

**Key insight:** OSC binary encoding has subtle 4-byte alignment padding rules for strings and blobs. A hand-rolled encoder breaks on odd-length addresses or param names. rosc handles this correctly.

---

## SuperCollider OSC Protocol Reference

### Message Formats (verified against SC 3.14.1 docs)

**Source:** https://docs.supercollider.online/Reference/Server-Command-Reference.html

#### /d_recv — Load SynthDef from bytes
```
/d_recv <buffer:Blob> [<completionMsg:Blob>]
→ /done "/d_recv"          (async; unreliable on failure — SC bug #4411)
```
- `buffer`: raw SynthDef file bytes (`.scsyndef` binary format)
- Do NOT rely on `/done` as success. Use `/sync` after.

#### /sync — Synchronization gate
```
/sync <id:Int>
→ /synced <id:Int>         (fires when all preceding async commands complete)
```
- Send after `/d_recv`. Await `/synced <same_id>` before `/s_new`.
- Use a monotonic counter for IDs; any i32 works.

#### /s_new — Instantiate synth
```
/s_new <defName:String> <nodeID:Int> <addAction:Int> <target:Int>
        [<paramName:String> <paramValue:Float>...]
→ (no reply by default; /n_go fires if /notify is on)
```
- `defName`: SynthDef name (matches filename without extension, e.g., `"space-crackle"`)
- `nodeID`: client-allocated integer >= 1000 (avoid 0 and 1, reserved groups)
- `addAction`: 0=head, 1=tail, 2=before, 3=after, 4=replace
- `target`: 1 for default group (standard usage)
- Use nodeID = -1 to let server assign (but then you can't track it — don't do this)

#### /n_set — Set synth control values
```
/n_set <nodeID:Int> <paramName:String> <value:Float> [<paramName:String> <value:Float>...]
```
- Multiple name/value pairs in one message are valid
- `value` must be OscType::Float (f32), not Int

#### /n_free — Free synth node
```
/n_free <nodeID:Int> [<nodeID:Int>...]
```
- Multiple node IDs in one message are valid
- Freeing a non-existent node is silently ignored (safe to call)
- `/n_free` on an already-freed node does not produce an error

#### /status — Health check
```
/status
→ /status.reply <unused:Int> <numUGens:Int> <numSynths:Int> <numGroups:Int>
                <numSynthDefs:Int> <avgCPU:Float> <peakCPU:Float>
                <nominalSampleRate:Double> <actualSampleRate:Double>
```
- Use at startup to verify scsynth is reachable before attempting SynthDef loads

### Node ID Allocation Strategy (Claude's recommendation)

Use sequential allocation starting at 1000. Track in `HashMap<String, i32>`:

```
thing_name ("space-crackle") → node_id (1000)
thing_name ("bass-drone")    → node_id (1001)
```

On hot reload (same thing replaces with new SynthDef): free old node_id, allocate new one. Sequential IDs avoid collisions with reserved IDs (0, 1) and are easy to debug (log shows node 1000, 1001, etc.).

---

## Common Pitfalls

### Pitfall 1: /d_recv Race — /s_new Before /synced
**What goes wrong:** Synth instantiation silently fails; no audio, no error.
**Why it happens:** `/d_recv` is async; the SynthDef isn't available until scsynth finishes compiling it.
**How to avoid:** Always use the `/sync` → `/synced` handshake. Never skip it.
**Warning signs:** Synths fail intermittently; adding `tokio::time::sleep(100ms)` makes them work.

### Pitfall 2: /done from /d_recv Is Unreliable (SC bug #4411)
**What goes wrong:** `/done` arrives even when the SynthDef failed to load (malformed bytecode). Bridge proceeds to `/s_new` which silently fails.
**Why it happens:** scsynth exception handling in the async loader swallows errors.
**How to avoid:** Use `/sync` (not `/done`) as the gate. After receiving `/synced`, if `/s_new` produces no audio, log a warning and surface it — don't assume success.
**Warning signs:** Malformed `.scd` updated, synth still "starts" (sends /s_new), but no audio.

### Pitfall 3: WSL2 Gateway IP Changes on Restart
**What goes wrong:** Hardcoded IP in config stops working after WSL2 restarts. OSC sends succeed (UDP is connectionless) but packets arrive nowhere.
**Why it happens:** WSL2 NAT reassigns the gateway subnet on each launch.
**How to avoid:** Document that `SCSYNTH_HOST` must be set at runtime. Never commit a `172.x.x.x` address to git. Consider auto-detection via `ip route | grep default` as a convenience helper.
**Warning signs:** OSC sends complete without error but scsynth never responds; `/status` times out.

### Pitfall 4: Orphaned Nodes Accumulate
**What goes wrong:** Hot reloads leave old synth nodes running; CPU climbs; audio corrupts.
**Why it happens:** scsynth node lifecycle is 100% client responsibility.
**How to avoid:** Always call `free_node(thing_name)` before `new_synth(thing_name, ...)`. On daemon shutdown (SIGINT/SIGTERM), call `free_all_nodes()`.
**Warning signs:** CPU on scsynth host climbs after repeated saves; old sounds audible under new ones.

### Pitfall 5: Concurrent recv Stealing /synced Messages
**What goes wrong:** If a background recv loop runs while `await_synced` is also calling recv, one of them steals the `/synced` message and the other loops forever.
**Why it happens:** tokio UdpSocket is not a broadcast bus — each recv call gets exactly one datagram.
**How to avoid:** In Phase 1, don't run any background recv loop. OSC send is fire-and-forget except during sync-wait. For Phase 2+ (if adding a background event receiver), use a single recv task that dispatches to channels.
**Warning signs:** `await_synced` times out consistently even though scsynth is healthy.

---

## Code Examples

### Complete /d_recv + /sync + /s_new sequence
```rust
// Source: rosc docs.rs/rosc/latest + SC Server Command Reference
// This is the safe SynthDef load + instantiation sequence

async fn load_and_play(
    client: &mut ScsynthClient,
    thing_name: &str,
    synthdef_name: &str,
    synthdef_bytes: Vec<u8>,
) -> anyhow::Result<()> {
    // Phase A: Load SynthDef with sync handshake
    client.load_synthdef(synthdef_bytes).await?;  // /d_recv + /sync + await /synced

    // Phase B: Instantiate (safe: SynthDef is confirmed loaded)
    client.new_synth(thing_name, synthdef_name).await?;  // /s_new node_id

    Ok(())
}
```

### rosc OscType quick reference
```rust
// All OscType variants (rosc 0.11, source: docs.rs/rosc/latest/rosc/enum.OscType.html)
OscType::Int(i32)           // OSC 'i' — node IDs, sync IDs, add actions
OscType::Float(f32)         // OSC 'f' — control values, frequencies, amplitudes
OscType::String(String)     // OSC 's' — synthdef names, param names
OscType::Blob(Vec<u8>)      // OSC 'b' — SynthDef binary data for /d_recv
OscType::Long(i64)          // OSC 'h' — rarely needed
OscType::Double(f64)        // OSC 'd' — rarely needed
OscType::Bool(bool)         // OSC 'T'/'F'
OscType::Nil                // OSC 'N'
OscType::Inf                // OSC 'I'
OscType::Time(OscTime)      // OSC timetag
OscType::Char(char)         // OSC 'c'
OscType::Color(OscColor)    // OSC 'r'
OscType::Midi(OscMidiMessage) // OSC 'm'
OscType::Array(OscArray)    // OSC '[...]'
```

### decoder::decode_udp signature (rosc 0.11)
```rust
// Source: docs.rs/rosc/latest/rosc/decoder/fn.decode_udp.html
// Returns (&[u8] remainder, OscPacket) — use the packet, ignore remainder for simple UDP
pub fn decode_udp(msg: &[u8]) -> Result<(&[u8], OscPacket), OscError>

// Usage:
let mut buf = vec![0u8; 65535];
let n = socket.recv(&mut buf).await?;
match decoder::decode_udp(&buf[..n]) {
    Ok((_, OscPacket::Message(msg))) => { /* handle msg */ }
    Ok((_, OscPacket::Bundle(bundle))) => { /* handle bundle */ }
    Err(e) => tracing::warn!("OSC decode error: {:?}", e),
}
```

### Startup health check
```rust
// Send /status and wait for /status.reply to confirm scsynth is reachable
pub async fn check_alive(&self) -> anyhow::Result<()> {
    self.send_message("/status", vec![]).await?;
    let mut buf = vec![0u8; 4096];
    match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        self.socket.recv(&mut buf)
    ).await {
        Ok(Ok(n)) => {
            if let Ok((_, OscPacket::Message(msg))) = decoder::decode_udp(&buf[..n]) {
                if msg.addr == "/status.reply" {
                    tracing::info!("scsynth alive: {:?}", msg.args);
                    return Ok(());
                }
            }
            anyhow::bail!("unexpected response to /status")
        }
        Ok(Err(e)) => anyhow::bail!("socket error: {}", e),
        Err(_) => anyhow::bail!("scsynth not responding at configured host (timeout 2s)"),
    }
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| serde_yaml for YAML | yaml_serde (yaml/yaml-serde) or serde-saphyr | 2024 | serde_yaml deprecated; must migrate |
| tokio UdpSocket requires &mut self | tokio 1.x: &self for send/recv | Tokio 1.0 (2020) | Can share socket reference; Arc not needed for single-owner |
| rosc decoder::decode() | rosc decoder::decode_udp() | rosc 0.8+ | decode_udp is the correct entry point for UDP datagrams; decode() is for stream framing |

**Deprecated/outdated:**
- `rosc::decoder::decode()` — use `decoder::decode_udp()` for UDP packets; `decode()` is for streaming contexts
- `serde_yaml` — deprecated by dtolnay March 2024; flagged by cargo audit

---

## Open Questions

1. **SynthDef binary format: .scd vs .scsyndef**
   - What we know: scsynth's `/d_recv` expects raw `.scsyndef` binary bytes (the compiled binary, NOT the `.scd` source text). sclang compiles `.scd` → `.scsyndef`.
   - What's unclear: The REQUIREMENTS.md says "read .scd files from out/sc/" — but it's likely the convention will be to store compiled `.scsyndef` bytes inside or alongside the `.scd` source. This needs clarification before Phase 1 implementation.
   - Recommendation: For Phase 1, assume `out/sc/` contains compiled `.scsyndef` binary files (not raw sclang source). Verify with a real sclang compilation before implementing SCD-01.

2. **/notify for async node events**
   - What we know: scsynth has a `/notify` command that enables push notifications for node lifecycle events (`/n_go`, `/n_end`). Without it, the client is purely fire-and-forget.
   - What's unclear: Phase 1 doesn't require event-driven node tracking. But if future phases need to detect when a synth ends naturally, `/notify` must be sent at startup.
   - Recommendation: Skip `/notify` in Phase 1. Add it in future phases if needed. The registry-based approach handles lifecycle tracking client-side.

3. **Concurrent recv during normal operation (Phase 1 scope)**
   - What we know: `await_synced` uses `socket.recv()` synchronously during the handshake window. If any other code calls recv concurrently, messages will be stolen.
   - What's unclear: Phase 1 has no background recv loop, so this isn't a problem yet. Phase 2+ (file watcher integration) may need a recv dispatcher.
   - Recommendation: Phase 1 keeps the bridge purely synchronous (one operation at a time). Document this as a known limitation for Phase 2.

---

## Sources

### Primary (HIGH confidence)
- [SC Server Command Reference 3.14.1](https://docs.supercollider.online/Reference/Server-Command-Reference.html) — /d_recv, /s_new, /n_set, /n_free, /sync formats; add action constants
- [SC Sync-Async Guide](https://doc.sccode.org/Guides/Sync-Async.html) — /sync usage as async gate; confirmed pattern
- [rosc on docs.rs (latest)](https://docs.rs/rosc/latest/rosc/) — OscType variants, encoder::encode, decoder::decode_udp
- [tokio UdpSocket docs](https://docs.rs/tokio/latest/tokio/net/struct.UdpSocket.html) — bind/connect/send/recv API
- [SC bug #4411](https://github.com/supercollider/supercollider/issues/4411) — /done unreliable on /d_recv failure; confirmed

### Secondary (MEDIUM confidence)
- [rosc on crates.io](https://crates.io/crates/rosc) — version 0.11.4 confirmed
- [rosc GitHub (klingtnet)](https://github.com/klingtnet/rosc) — examples and README
- [tokio examples/udp-client.rs](https://github.com/tokio-rs/tokio/blob/master/examples/udp-client.rs) — connect + send + recv pattern

### Tertiary (LOW confidence)
- [rosc_supercollider crate](https://docs.rs/rosc_supercollider/latest/rosc_supercollider/) — SC-specific OSC types; LOW because it's a small unmaintained crate, but useful for reference on SC address deviations

---

## Metadata

**Confidence breakdown:**
- Standard stack (rosc, tokio): HIGH — versions verified via docs.rs and crates.io
- SC OSC protocol (message formats, args): HIGH — official SC 3.14.1 docs
- /sync handshake pattern: HIGH — official SC Sync-Async guide
- Config pattern (env + toml): HIGH — stdlib + established toml crate pattern
- Node ID allocation: MEDIUM — conventional 1000+ start from SC community; not in official docs but consistent across all SC client libraries
- Concurrent recv pitfall: MEDIUM — derived from tokio UdpSocket semantics; verified against tokio forums

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable crates; SC protocol doesn't change)

---
phase: 01-osc-bridge
plan: 2
type: execute
wave: 2
depends_on:
  - "01-PLAN-1"
files_modified:
  - src/osc/mod.rs
  - src/osc/bridge.rs
  - src/osc/commands.rs
  - src/osc/error.rs
  - src/main.rs
autonomous: true
requirements:
  - OSC-03
  - OSC-04
  - OSC-05
  - OSC-06

must_haves:
  truths:
    - "hum-rt connects to scsynth and survives the startup health check (/status.reply received)"
    - "A hardcoded SynthDef can be loaded via /d_recv + /sync handshake (waits for /synced before proceeding)"
    - "A synth node can be created (/s_new), have a parameter updated (/n_set), and be freed (/n_free)"
    - "On clean shutdown (Ctrl-C), all tracked nodes are freed before exit"
    - "Attempting to connect to an unreachable host fails fast with a clear error message"
  artifacts:
    - path: "src/osc/bridge.rs"
      provides: "ScsynthClient struct with connect, check_alive, load_synthdef, new_synth, set_param, free_node, free_all_nodes"
      exports: ["ScsynthClient"]
    - path: "src/osc/error.rs"
      provides: "OscBridgeError enum for typed errors"
      exports: ["OscBridgeError"]
    - path: "src/osc/mod.rs"
      provides: "pub use re-exports for the osc module"
    - path: "src/osc/commands.rs"
      provides: "send_message helper (encoding OscPacket via rosc encoder)"
  key_links:
    - from: "src/main.rs"
      to: "src/osc/bridge.rs"
      via: "ScsynthClient::connect(&cfg.scsynth_host)"
      pattern: "ScsynthClient::connect"
    - from: "src/osc/bridge.rs"
      to: "scsynth UDP socket"
      via: "tokio::net::UdpSocket"
      pattern: "UdpSocket::bind"
    - from: "src/osc/bridge.rs"
      to: "/sync handshake"
      via: "await_synced"
      pattern: "await_synced"
---

<objective>
Implement the full OSC bridge: ScsynthClient with UDP socket, /d_recv+/sync SynthDef loading, node lifecycle (/s_new, /n_set, /n_free), and clean shutdown.

Purpose: Delivers the entire scsynth communication layer. After this plan, hum-rt can speak the SuperCollider OSC protocol end-to-end.
Output: A working ScsynthClient that can be exercised via a hardcoded smoke test in main.rs (load SynthDef, create synth, update param, free node, exit cleanly).
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/phases/01-osc-bridge/01-CONTEXT.md
@.planning/phases/01-osc-bridge/research/RESEARCH.md
@.planning/phases/01-osc-bridge/01-1-SUMMARY.md

<interfaces>
<!-- From src/config.rs (Plan 1 output) -->
```rust
pub struct Config {
    pub scsynth_host: String,  // e.g. "127.0.0.1:57110"
}
impl Config {
    pub fn load() -> anyhow::Result<Self>;
}
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: ScsynthClient — OSC bridge core</name>
  <files>src/osc/mod.rs, src/osc/error.rs, src/osc/bridge.rs</files>
  <action>
Create the osc module with three files. All patterns directly from RESEARCH.md.

**src/osc/error.rs** — typed error enum:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OscBridgeError {
    #[error("sync timeout waiting for /synced {0}")]
    SyncTimeout(i32),
    #[error("OSC encode error: {0}")]
    EncodeError(String),
    #[error("socket error: {0}")]
    SocketError(#[from] std::io::Error),
    #[error("scsynth unreachable at configured host (timeout 2s)")]
    Unreachable,
    #[error("no node registered for thing: {0}")]
    UnknownThing(String),
}
```

**src/osc/mod.rs** — re-exports:
```rust
mod bridge;
mod error;

pub use bridge::ScsynthClient;
pub use error::OscBridgeError;
```

**src/osc/bridge.rs** — full ScsynthClient implementation following all four patterns from RESEARCH.md:

- `ScsynthClient` struct: owns `UdpSocket`, `HashMap<String, i32>` node registry, `next_node_id: i32` (start at 1000), `next_sync_id: i32` (start at 1)
- `connect(addr: &str) -> Result<Self>`: `UdpSocket::bind("0.0.0.0:0")` + `socket.connect(addr)`
- `check_alive(&self) -> Result<()>`: send `/status`, await `/status.reply` with 2s timeout; bail with `OscBridgeError::Unreachable` on timeout
- `load_synthdef(&mut self, bytes: Vec<u8>) -> Result<()>`: send `/d_recv` (OscType::Blob), send `/sync <id>`, call `await_synced(id, 5s)`
- `await_synced(&self, expected_id: i32, deadline: Duration) -> Result<()>`: recv loop with `tokio::time::timeout`; match `/synced` messages by ID; discard others with `tracing::debug!`; return `OscBridgeError::SyncTimeout(id)` on timeout
- `new_synth(&mut self, thing_name: &str, synthdef_name: &str) -> Result<i32>`: free existing node for thing_name if present; allocate node_id; send `/s_new defName nodeID 0 1`; insert into registry
- `set_param(&self, thing_name: &str, param: &str, value: f32) -> Result<()>`: look up node_id; send `/n_set nodeID paramName value`
- `free_node(&mut self, thing_name: &str) -> Result<()>`: remove from registry, send `/n_free nodeID`
- `free_all_nodes(&mut self) -> Result<()>`: free all registry entries, clear map
- `send_message(&self, addr: &str, args: Vec<OscType>) -> Result<()>`: encode with `rosc::encoder::encode`, socket.send; tracing::debug

CRITICAL CONSTRAINTS (from RESEARCH.md anti-patterns):
- Never send /s_new before /synced arrives — always use the full /d_recv + /sync + await_synced sequence
- Never treat /done from /d_recv as success confirmation (SC bug #4411)
- Node IDs start at 1000 (0 and 1 are reserved scsynth groups)
- No concurrent recv loop in Phase 1 — recv is only called during sync-wait and check_alive
- Use `decoder::decode_udp()` not `decoder::decode()` for UDP datagrams
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build 2>&1 | tail -10</automated>
  </verify>
  <done>cargo build exits 0. All four ScsynthClient methods (connect, load_synthdef, new_synth, set_param, free_node, free_all_nodes) compile with no errors.</done>
</task>

<task type="auto">
  <name>Task 2: Smoke test in main.rs + graceful shutdown</name>
  <files>src/main.rs</files>
  <action>
Update src/main.rs to wire the full OSC lifecycle and handle Ctrl-C gracefully. This is the end-to-end smoke test for the phase.

The main flow:
1. Load config (Config::load())
2. Connect: ScsynthClient::connect(&cfg.scsynth_host)
3. Health check: client.check_alive() — if it fails, print a clear error and exit 1 (do NOT panic)
4. Load a minimal hardcoded SynthDef binary — use an actual compiled scsyndef blob:
   - Define `const SINE_SCSYNDEF: &[u8]` as the raw bytes of a minimal sine SynthDef
   - For the smoke test, use a known-good minimal scsyndef. The simplest approach: encode the bytes inline as a byte literal for a SynthDef named "sine-test" with a freq control (440Hz default) and amplitude 0.1. If you cannot embed a real binary, use an empty Vec<u8> and note that /d_recv with empty bytes will fail the /sync handshake — but the flow structure should still compile and the timeout error is expected without a real scsynth.
   - Comment clearly: "Replace SINE_SCSYNDEF with real compiled bytes from sclang for live testing"
5. Load the SynthDef: client.load_synthdef(SINE_SCSYNDEF.to_vec())
6. Create synth: client.new_synth("smoke-test", "sine-test")
7. Update param: client.set_param("smoke-test", "freq", 880.0)
8. Sleep 2 seconds (so the synth can be heard if scsynth is running)
9. Free node: client.free_node("smoke-test")
10. Print "smoke test complete — no orphaned nodes"

Graceful shutdown (tokio::signal): Wrap the main logic in a select! that catches SIGINT/SIGTERM:
```rust
tokio::select! {
    result = run_smoke_test(&mut client) => { result? }
    _ = tokio::signal::ctrl_c() => {
        tracing::info!("shutting down — freeing all nodes");
        client.free_all_nodes().await?;
    }
}
```

Extract the smoke test steps into `async fn run_smoke_test(client: &mut ScsynthClient) -> anyhow::Result<()>`.

Error handling: Use `?` throughout. If check_alive() fails, return a descriptive error. Do not unwrap anywhere in main.rs.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build 2>&1 | tail -10</automated>
  </verify>
  <done>
- cargo build exits 0 with no errors or warnings
- `cargo run` prints the resolved scsynth host, attempts connection, and either completes the smoke test (if scsynth is running) or fails with a clear "scsynth unreachable" message and exits 1 (not a panic/backtrace)
- If Ctrl-C is pressed during the 2-second sleep, free_all_nodes() is called before exit
  </done>
</task>

<task type="checkpoint:human-verify" gate="blocking">
  <what-built>
Full OSC bridge: ScsynthClient with /d_recv+/sync handshake, /s_new, /n_set, /n_free, graceful shutdown, and a smoke test main.rs that exercises the full lifecycle.
  </what-built>
  <how-to-verify>
With scsynth running on Windows (or locally):

1. Set the gateway IP: `export SCSYNTH_HOST=172.29.224.1:57110` (adjust IP via `ip route | grep default`)
2. Run: `cd ~/code/hum && cargo run`
3. Expected output sequence:
   - `hum-rt: scsynth host = 172.29.224.1:57110`
   - `scsynth alive: [...]` (status.reply args)
   - `s_new: smoke-test -> node 1000`
   - (2 second pause — you may hear the sine tone if audio is connected)
   - `smoke test complete — no orphaned nodes`
4. In scsynth post window, verify no "FAILURE" or node error messages
5. Run again with an invalid host: `SCSYNTH_HOST=127.0.0.1:9999 cargo run`
   - Expected: "scsynth unreachable at configured host (timeout 2s)" printed, exit code 1

Without scsynth running:
- `cargo run` should fail with the clear unreachable error (not a panic)
- Ctrl-C during the 2s sleep should print "freeing all nodes" before exit
  </how-to-verify>
  <resume-signal>Type "approved" when the OSC smoke test passes, or describe any issues to fix</resume-signal>
</task>

</tasks>

<verification>
1. `cargo build` — exits 0, zero errors, zero warnings
2. `cargo test` — all tests pass
3. With scsynth running: `cargo run` completes the full lifecycle without orphaned nodes
4. Without scsynth: `SCSYNTH_HOST=127.0.0.1:9999 cargo run` exits 1 with readable error (no backtrace, no panic)
5. SIGINT during run calls free_all_nodes() before exit
6. Changing SCSYNTH_HOST between runs targets a different host (OSC-02 verification)
</verification>

<success_criteria>
- ScsynthClient connects to scsynth via UDP with configurable host (OSC-01, OSC-02)
- /d_recv + /sync + /synced handshake gate SynthDef loading (OSC-03)
- /s_new with node ID registry creates synth instances (OSC-04)
- /n_set updates control parameters on running synths (OSC-05)
- /n_free cleans up nodes; free_all_nodes() on shutdown prevents orphans (OSC-06)
- Human verifies live smoke test with real scsynth
</success_criteria>

<output>
After completion, create `.planning/phases/01-osc-bridge/01-2-SUMMARY.md` summarizing:
- Files created and their public API surface
- ScsynthClient method signatures
- Any deviations from research patterns (and why)
- Open questions for future phases (concurrent recv, /notify)
</output>

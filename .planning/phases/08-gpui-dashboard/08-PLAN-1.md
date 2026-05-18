---
phase: 08-gpui-dashboard
plan: 1
type: execute
wave: 1
depends_on: []
files_modified:
  - src/transport.rs
  - src/main.rs
  - src/osc/bridge.rs
autonomous: true
requirements: [XFIX-01, TUI-02]

must_haves:
  truths:
    - "`hum play from 1m30s` seeks and starts in one command"
    - "TransportReply::Status includes per-thing amplitude data"
    - "`hum play from` is atomic — no manual seek + play two-step"
  artifacts:
    - path: "src/transport.rs"
      provides: "PlayFrom variant in TransportCmd; amplitudes field in TransportReply::Status"
      contains: "PlayFrom"
    - path: "src/main.rs"
      provides: "handle_transport arm for PlayFrom; amplitude collection in Status arm; parse_time_arg supports Xm Ys format"
      contains: "TransportCmd::PlayFrom"
    - path: "src/osc/bridge.rs"
      provides: "get_node_amplitude method querying /n_get on amp bus"
      contains: "get_node_amplitude"
  key_links:
    - from: "src/main.rs run_cli"
      to: "src/transport.rs TransportCmd::PlayFrom"
      via: "send_cmd dispatch on 'play from'"
    - from: "src/main.rs handle_transport Status arm"
      to: "src/osc/bridge.rs get_node_amplitude"
      via: "amplitude query per active node"
---

<objective>
Extend the transport protocol for the two things the GPUI dashboard needs from the daemon: (1) atomic PlayFrom command so `hum play from <time>` works correctly, and (2) amplitude data in Status reply so the watch client can render VU meters without a separate OSC connection.

Purpose: Unblocks both XFIX-01 (correctness fix) and TUI-02 (VU meter data). All changes are in the daemon/transport layer — no GPUI code yet.
Output: Updated protocol types, daemon handler, CLI dispatch, and a new `get_node_amplitude` helper on ScsynthClient.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md

@src/transport.rs
@src/main.rs
@src/osc/bridge.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add PlayFrom to TransportCmd and amplitude to TransportReply::Status</name>
  <files>src/transport.rs</files>
  <action>
In `TransportCmd` enum, add a new variant:
```rust
PlayFrom { pos: f64 },
```

In `TransportReply::Status`, add a new field:
```rust
amplitudes: std::collections::HashMap<String, f32>,
```

Keep all existing variants and fields exactly as-is. The `amplitudes` map keys are thing names; values are 0.0–1.0 linear amplitude. If amplitude is unavailable for a node, omit its key (sparse map is fine).

`serde` derives already present — no additional derives needed. Protocol stays JSON newline-delimited.
  </action>
  <verify>cargo check 2>&amp;1 | grep -E "^error" | head -20</verify>
  <done>
- `TransportCmd::PlayFrom { pos: f64 }` compiles
- `TransportReply::Status` has `amplitudes: HashMap&lt;String, f32&gt;` field
- `cargo check` passes with no new errors
  </done>
</task>

<task type="auto">
  <name>Task 2: Implement PlayFrom handler + amplitude polling in daemon; fix CLI dispatch</name>
  <files>src/main.rs, src/osc/bridge.rs</files>
  <action>
**src/osc/bridge.rs — add `get_node_amplitude`:**

Add a method to `ScsynthClient` that sends `/n_get` for a node's `amp` parameter and returns the value as `f32`. Pattern: send OSC message `/n_get [node_id, "amp"]`, wait for `/n_info` or `/n_set` reply (scsynth responds to /n_get with /n_set). If no reply within 20ms, return `None`. Use existing `send_recv` pattern in the file. Signature:
```rust
pub async fn get_node_amplitude(&mut self, node_id: i32) -> Option<f32>
```

**src/main.rs — handle_transport: add PlayFrom arm:**

In `handle_transport` match block, add:
```rust
TransportCmd::PlayFrom { pos } => {
    state.playback_pos = pos;
    state.playing = true;
    restart_ticker(ticker_handle, tx, pos);
    reconcile_now(state, client, seq_tx, sequencer_handles).await;
    tracing::info!("transport: play from {:.2}s", pos);
    TransportReply::Ack
}
```

**src/main.rs — handle_transport: fix Status arm to collect amplitudes:**

In the `TransportCmd::Status` arm, after collecting `active`, iterate over `state.actual.nodes` and call `client.get_node_amplitude(node_id)` for each. Build the amplitudes HashMap. Because `handle_transport` takes `&mut ScsynthClient`, this is straightforward. Use `futures::future::join_all` or a simple sequential loop (sequential is fine — only 6-10 nodes max).

Update the `TransportReply::Status { ... }` construction to include `amplitudes`.

**src/main.rs — run_cli: fix `play from` dispatch:**

Current code (lines 209-212) sends only `Seek` when `play from` is given. Replace with `PlayFrom`:
```rust
if args.len() >= 3 && args[1] == "from" {
    let pos = parse_time_arg(&args[2])?;
    TransportCmd::PlayFrom { pos }
} else {
    TransportCmd::Play
}
```

**src/main.rs — parse_time_arg: support `Xm Ys` and `XmYs` format:**

Current parser handles `10s` and `10`. Extend to handle:
- `1m30s` → 90.0
- `1m` → 60.0
- `30s` → 30.0 (already works)
- bare `90` → 90.0 (already works)

Implementation: if string contains `m`, split on `m`, parse minutes part, then parse seconds part (strip trailing `s`). Return minutes*60 + seconds.

**print_reply: update Status arm** to handle the new `amplitudes` field (add it to the destructure pattern, can ignore the value in print for now — amplitudes are for the GPUI watch client, not terminal output).
  </action>
  <verify>cargo build 2>&amp;1 | grep -E "^error" | head -20</verify>
  <done>
- `cargo build` succeeds with zero errors
- `hum play from 1m30s` sends `PlayFrom { pos: 90.0 }` (verify by adding a tracing::info! log and running daemon + CLI)
- `TransportReply::Status` JSON includes `"amplitudes":{}` field
- `hum status` still works (print_reply handles new field)
  </done>
</task>

</tasks>

<verification>
```
cargo test 2>&1 | tail -5
# Existing tests still pass

# Manual smoke test (daemon running):
# hum play from 1m30s   → logs "transport: play from 90.00s"
# hum status            → returns JSON with amplitudes field
```
</verification>

<success_criteria>
- `cargo build` clean
- All pre-existing tests pass
- `TransportCmd::PlayFrom` and `TransportReply::Status.amplitudes` are live in the protocol
- `hum play from 1m30s` is atomic (seek + play in one round trip)
</success_criteria>

<output>
After completion, create `.planning/phases/08-gpui-dashboard/08-1-SUMMARY.md`
</output>

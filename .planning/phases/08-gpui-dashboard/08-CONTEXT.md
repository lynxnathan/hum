# Phase 8: GPUI Dashboard + Transport Fix - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

`hum watch` opens a GPU-accelerated GPUI window showing live timeline, per-thing VU meters, and transport state. Also: `hum play from <time>` as single atomic command. Does NOT cover: pipe language, ref resolution, instruments, or stage effects.

</domain>

<decisions>
## Implementation Decisions

### Confirmed by Spike Test

- **GPUI 0.2.2** on crates.io, compiles and runs on WSL2
- Must force X11: `WAYLAND_DISPLAY="" DISPLAY=:0` (WSLg Wayland too old)
- System deps: libxkbcommon-x11-dev libwayland-dev libxcb-xkb-dev
- API: `Application::new().run(|cx: &mut App| { ... })`
- `cx.open_window(opts, |window, cx| cx.new(|_| View))`
- Render trait: `fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement`

### Claude's Discretion

- Dashboard layout — timeline on top, VU meters below, transport bar at bottom?
- How to poll scsynth amplitudes — `/s_get` per node, or `/c_get` on control buses?
- Communication between daemon and GPUI window — shared state, unix socket, or channels?
- Color scheme and visual design

</decisions>

<specifics>
## Specific Ideas

- `hum watch` should connect to the running daemon via the existing unix socket
- Poll `/status` and per-thing amplitude data at ~20-60fps
- The GPUI window is a separate process (client mode, like `hum play`)
- For `hum play from <time>`: modify CLI dispatch to send Seek then Play as two commands atomically, or add a PlayFrom transport command

</specifics>

<code_context>
## Existing Code

- `src/transport.rs` — TransportCmd/TransportReply, unix socket server/client
- `src/main.rs` — CLI dispatch (run_cli), daemon event loop
- `src/osc/bridge.rs` — ScsynthClient (can query node state)
- Spike test at `/tmp/gpui-spike/` — working GPUI hello world

### Integration Points
- GPUI window connects as unix socket client (like `hum status`)
- Needs new TransportCmd::Watch that streams status updates (or poll Status repeatedly)
- `hum play from` needs PlayFrom variant or atomic Seek+Play

</code_context>

<deferred>
## Deferred Ideas

- Embed terminal (Claude Code) inside GPUI window — v3 feature, requires Zed's terminal component
- Waveform rendering from scsynth audio bus — requires SharedMemory or buffer reads

</deferred>

---
*Phase: 08-gpui-dashboard*
*Context gathered: 2026-03-22*

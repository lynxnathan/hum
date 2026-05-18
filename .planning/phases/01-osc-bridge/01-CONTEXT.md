# Phase 1: OSC Bridge - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

hum-rt connects to a configurable scsynth instance and communicates via OSC over UDP. Covers: connection establishment, SynthDef loading with /sync handshake, synth node lifecycle (create/update/free), config-driven host targeting. Does NOT cover: file watching, .hum parsing, timeline, or transport.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion

User trusts Claude's judgment on all Phase 1 implementation decisions. The following areas are open:

- **Connection behavior** — Startup sequence, retry strategy if scsynth isn't running, blocking vs background connect, health checks
- **Config design** — SCSYNTH_HOST env var and/or config file, CLI flags, defaults (localhost:57110), config file format and location
- **Error reporting** — Log levels, stderr output, exit codes, what's fatal (can't connect) vs recoverable (transient UDP failure)
- **Node ID strategy** — Sequential vs random node IDs, thing_name → node_id registry design, cleanup on disconnect/crash

</decisions>

<specifics>
## Specific Ideas

- WSL2 → Windows scsynth is a confirmed working setup (gateway IP 172.29.224.1:57110, tested with /status.reply)
- scsynth /d_recv is async — must use /sync before /s_new (from pitfalls research)
- scsynth /d_recv sends /done even on failure (SC bug #4411) — cannot trust /done as success
- Orphaned nodes are the client's responsibility — must track and free

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- None — greenfield project

### Established Patterns
- None yet — Phase 1 establishes the patterns

### Integration Points
- scsynth on Windows at configurable host:port (default localhost:57110)
- OSC over UDP (rosc crate for encoding/decoding)
- tokio async runtime for UDP socket and future event loop

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 01-osc-bridge*
*Context gathered: 2026-03-20*

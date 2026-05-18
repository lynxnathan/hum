# Phase 4: Transport + E2E - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

CLI transport controls via unix socket (play, stop, status, solo, mute, seek, loop) and verification of all three end-to-end scenarios. This is the final phase — after this, the core value proposition works: edit .hum, hear it change.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion

User trusts Claude's judgment on all Phase 4 decisions. Open areas:

- **Unix socket protocol** — JSON, newline-delimited, or custom. Request/response format for hum CLI → hum-rt daemon.
- **CLI binary** — Same binary with subcommands (hum play, hum stop) or separate client binary?
- **Solo/mute state persistence** — How to survive file reloads. Stored in StateStore alongside actual state.
- **Seek implementation** — Reset timeline position, recompute active things, reconcile.
- **Loop implementation** — Timeline wraps between start/end, re-reconciles on wrap.
- **Status output format** — Human-readable vs JSON. What info to show.

</decisions>

<specifics>
## Specific Ideas

- Phase 3 event loop already handles Ctrl-C graceful shutdown
- StateStore has desired (Piece) and actual (thing_name → node_id)
- Solo/mute are runtime state — must NOT be overwritten when piece.hum reloads
- Timeline ticker already runs at 50ms intervals
- Reconciler diff() already computes Add/Remove ops
- Need to add a DaemonEvent variant for transport commands

</specifics>

<code_context>
## Existing Code

### From Phase 3
- `src/main.rs` — tokio::select! event loop handling FileChanged, Tick, Ctrl-C
- `src/events.rs` — DaemonEvent enum
- `src/state.rs` — StateStore, ActualState, active_things()
- `src/reconciler.rs` — diff(), ReconcileOp
- `src/watcher.rs` — start_watcher()
- `src/timeline.rs` — run_ticker()

### From Phase 2
- `src/parser/` — parse_hum(), Piece, ThingDef
- `src/scd/` — ScdStore

### From Phase 1
- `src/osc/bridge.rs` — ScsynthClient
- `src/config.rs` — Config::load()

</code_context>

<deferred>
## Deferred Ideas

None

</deferred>

---
*Phase: 04-transport-e2e*
*Context gathered: 2026-03-20*

---
phase: 01-osc-bridge
plan: 2
subsystem: osc
tags: [rust, osc, udp, scsynth, rosc, tokio, supercollider]

# Dependency graph
requires:
  - phase: 01-osc-bridge plan 1
    provides: "Cargo workspace with dependencies, Config module with scsynth_host resolution"
provides:
  - "ScsynthClient struct with full OSC lifecycle: connect, check_alive, load_synthdef, new_synth, set_param, free_node, free_all_nodes"
  - "OscBridgeError enum for typed error handling"
  - "Smoke test main.rs exercising full lifecycle with graceful shutdown"
affects: [02-hum-parser, 03-file-watcher, 04-timeline]

# Tech tracking
tech-stack:
  added: []
  patterns: [osc-sync-handshake, node-id-registry, graceful-shutdown-select]

key-files:
  created: [src/osc/mod.rs, src/osc/error.rs, src/osc/bridge.rs]
  modified: [src/main.rs]

key-decisions:
  - "OscBridgeError as separate thiserror enum (not anyhow-only) for typed error matching"
  - "Node IDs start at 1000 with sequential allocation, HashMap<String, i32> registry"
  - "No concurrent recv loop in Phase 1 — recv only during sync-wait and check_alive"
  - "Empty SINE_SCSYNDEF placeholder — requires real .scsyndef bytes for live testing"
  - "Suppressed unused_imports warning on OscBridgeError re-export (public API for future consumers)"

patterns-established:
  - "/d_recv + /sync + await_synced handshake before any /s_new"
  - "tokio::select! for graceful shutdown with free_all_nodes on SIGINT"
  - "send_message helper for all OSC encoding via rosc encoder::encode"
  - "decode_udp (not decode) for all UDP datagram decoding"

requirements-completed: [OSC-03, OSC-04, OSC-05, OSC-06]

# Metrics
duration: 3min
completed: 2026-03-20
---

# Phase 1 Plan 2: OSC Bridge Summary

**ScsynthClient with /d_recv+/sync handshake, node lifecycle (/s_new, /n_set, /n_free), and graceful shutdown smoke test**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-20T06:33:39Z
- **Completed:** 2026-03-20T06:36:45Z
- **Tasks:** 2 (of 3; Task 3 is human-verify checkpoint)
- **Files modified:** 4

## Accomplishments
- ScsynthClient struct with 7 public methods covering full OSC lifecycle (connect, check_alive, load_synthdef, new_synth, set_param, free_node, free_all_nodes)
- /d_recv + /sync + /synced handshake correctly gates SynthDef loading (never trusts /done per SC bug #4411)
- Smoke test in main.rs exercises the full lifecycle with graceful Ctrl-C shutdown
- Clean error handling: unreachable scsynth prints readable message and exits 1 (no panic/backtrace)

## Task Commits

Each task was committed atomically:

1. **Task 1: ScsynthClient -- OSC bridge core** - `507b68c` (feat)
2. **Task 2: Smoke test in main.rs + graceful shutdown** - `3169c0e` (feat)

## Files Created/Modified
- `src/osc/error.rs` - OscBridgeError enum (SyncTimeout, EncodeError, SocketError, Unreachable, UnknownThing)
- `src/osc/mod.rs` - Module re-exports for ScsynthClient and OscBridgeError
- `src/osc/bridge.rs` - ScsynthClient struct with UDP socket, node registry, sync counter, and all OSC methods
- `src/main.rs` - Smoke test wiring full lifecycle with tokio::select! graceful shutdown

## Public API Surface

```rust
// src/osc/bridge.rs
impl ScsynthClient {
    pub async fn connect(addr: &str) -> Result<Self>;
    pub async fn check_alive(&self) -> Result<()>;
    pub async fn load_synthdef(&mut self, synthdef_bytes: Vec<u8>) -> Result<()>;
    pub async fn new_synth(&mut self, thing_name: &str, synthdef_name: &str) -> Result<i32>;
    pub async fn set_param(&self, thing_name: &str, param: &str, value: f32) -> Result<()>;
    pub async fn free_node(&mut self, thing_name: &str) -> Result<()>;
    pub async fn free_all_nodes(&mut self) -> Result<()>;
}
```

## Decisions Made
- OscBridgeError as a typed thiserror enum rather than raw anyhow strings, enabling downstream match on specific error variants
- Node IDs start at 1000 (sequential), avoiding scsynth reserved IDs 0 (root group) and 1 (default group)
- No concurrent recv loop in Phase 1: recv is called only during sync-wait and check_alive, avoiding message-stealing race condition
- Empty SINE_SCSYNDEF placeholder: real .scsyndef binary bytes needed for live testing (documented in code comments)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Suppressed unused_imports warning on OscBridgeError re-export**
- **Found during:** Task 2 (smoke test compilation)
- **Issue:** OscBridgeError is re-exported from osc/mod.rs as public API but not yet used in main.rs, causing a compiler warning
- **Fix:** Added `#[allow(unused_imports)]` on the re-export since it's intentionally part of the public API for future consumers
- **Files modified:** src/osc/mod.rs
- **Verification:** cargo build produces zero warnings
- **Committed in:** 3169c0e (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Trivial warning suppression. No scope creep.

## Issues Encountered
None beyond the warning fix above.

## Open Questions for Future Phases
- **Concurrent recv:** Phase 2+ may need a single recv task with dispatch channels if background event listening is added
- **/notify:** Not enabled in Phase 1; needed if future phases require async node lifecycle events (/n_go, /n_end)
- **Real SynthDef bytes:** SINE_SCSYNDEF is empty; live testing requires compiled .scsyndef from sclang

## User Setup Required
None - no external service configuration required. Live testing requires scsynth running (see Task 3 checkpoint).

## Next Phase Readiness
- ScsynthClient fully compiled and ready for integration with file watcher and timeline
- Config module pipes scsynth_host into ScsynthClient::connect
- Graceful shutdown pattern established for reuse in daemon mode
- Human verification (Task 3) pending: live smoke test with real scsynth

## Self-Check: PASSED

- All 5 files verified present on disk
- Commit 507b68c (Task 1) verified in git log
- Commit 3169c0e (Task 2) verified in git log
- cargo build: 0 errors, 0 warnings
- cargo test: 2 passed, 0 failed
- cargo run with invalid host: exits 1 with clean error message

---
*Phase: 01-osc-bridge*
*Completed: 2026-03-20*

---
phase: 04-transport-e2e
plan: 1
subsystem: transport
tags: [unix-socket, json-lines, cli, tokio, serde_json]

requires:
  - phase: 03-state-reconciler-watcher
    provides: event loop, StateStore, DaemonEvent, timeline ticker, reconciler

provides:
  - Unix socket transport layer (JSON newline-delimited protocol)
  - TransportCmd/TransportReply protocol types
  - CLI client dispatch (play, stop, status, seek, loop, solo, mute)
  - Daemon-side handle_transport with all 7 command handlers
  - Ticker handle management for seek/stop/loop-wrap
  - Playing guard and loop wrapping in handle_tick

affects: [04-transport-e2e]

tech-stack:
  added: [serde_json]
  patterns: [oneshot reply channel per transport command, JSON-lines over Unix socket, same-binary CLI/daemon dispatch]

key-files:
  created: [src/transport.rs]
  modified: [src/main.rs, src/events.rs, src/state.rs, Cargo.toml]

key-decisions:
  - "Same binary dispatch: no args = daemon, subcommand = CLI client"
  - "JSON newline-delimited protocol over /tmp/hum.sock"
  - "DaemonEvent::Transport carries oneshot::Sender for reply channel"
  - "Solo/mute toggle behavior: sending same thing twice unsolos/unmutes"
  - "Ticker always runs but handle_tick gates on state.playing"
  - "Loop wrapping restarts ticker from loop_start when pos >= loop_end"

patterns-established:
  - "Transport protocol: one JSON line per command, one JSON line reply, connection closed"
  - "Ticker handle management: abort + respawn pattern for seek and loop wrap"
  - "Playing guard: handle_tick returns early when !state.playing"

requirements-completed: [XPORT-01, XPORT-02, XPORT-03, XPORT-06, XPORT-07]

duration: 5min
completed: 2026-03-20
---

# Phase 4 Plan 1: Transport + CLI Summary

**Unix socket transport with JSON-lines protocol, CLI dispatch for 7 commands, ticker management for seek/stop/loop**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-20T17:28:03Z
- **Completed:** 2026-03-20T17:33:12Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Transport protocol types (TransportCmd, TransportReply) with serde JSON serialization
- Unix socket server on /tmp/hum.sock with per-connection JSON-lines handling
- CLI client mode: same binary dispatches play/stop/status/seek/loop/solo/mute
- All 7 transport command handlers wired into daemon event loop
- Ticker handle management: abort and respawn for seek, stop, and loop wrapping
- Playing state guard in handle_tick prevents reconciliation when stopped
- Loop wrapping: pos >= loop_end triggers ticker restart from loop_start

## Task Commits

Each task was committed atomically:

1. **Task 1: transport.rs -- protocol types, socket server, CLI client** - `45582e2` (feat)
2. **Task 2: Wire DaemonEvent::Transport into main.rs + add hum CLI subcommands** - `729526f` (feat)

## Files Created/Modified
- `src/transport.rs` - TransportCmd/TransportReply types, start_socket_server, send_cmd
- `src/main.rs` - CLI dispatch (run_cli), handle_transport with all 7 commands, ticker management, playing guard, loop wrap
- `src/events.rs` - DaemonEvent::Transport(TransportCmd, oneshot::Sender<TransportReply>)
- `src/state.rs` - Added playing and loop_range fields to StateStore
- `Cargo.toml` - Added serde_json dependency

## Decisions Made
- Same binary dispatch: no args = daemon, subcommand = CLI client (simple, no separate binary)
- JSON newline-delimited protocol over Unix socket (human-readable, easy to debug)
- Solo/mute as toggles: sending same thing twice reverses the operation
- Ticker always runs but handle_tick gates on state.playing (simpler than start/stop ticker)
- Loop wrapping via ticker restart rather than modulo arithmetic (clean position reset)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed non-exhaustive match in main.rs for Task 1 compilation**
- **Found during:** Task 1 (transport.rs creation)
- **Issue:** Adding Transport variant to DaemonEvent required a match arm in main.rs event loop for compilation
- **Fix:** Added placeholder Transport arm that sends Ack (replaced in Task 2 with full handler)
- **Files modified:** src/main.rs
- **Verification:** cargo build succeeded
- **Committed in:** 45582e2 (Task 1 commit)

**2. [Rule 1 - Bug] Fixed deprecated IndexMap::remove to shift_remove**
- **Found during:** Task 2 (main.rs rewrite)
- **Issue:** IndexMap::remove is deprecated, compiler warning about using swap_remove or shift_remove
- **Fix:** Changed to shift_remove to preserve insertion order
- **Files modified:** src/main.rs
- **Verification:** cargo build with no deprecation warning
- **Committed in:** 729526f (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both fixes necessary for correct compilation. No scope creep.

## Issues Encountered
- Plan 2 (solo/mute) ran in parallel and a linter merged some of its changes into state.rs and main.rs (solo_set, mute_set, active_things_filtered). These were compatible with Plan 1's work and no conflicts arose.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Transport layer fully wired, ready for E2E verification
- Solo/mute fields already in StateStore (Plan 2 parallel work)
- All 7 command variants compile and dispatch correctly

## Self-Check: PASSED

- All 6 files verified present on disk
- Commits 45582e2 and 729526f verified in git log
- cargo build succeeds, cargo test passes 49/49

---
*Phase: 04-transport-e2e*
*Completed: 2026-03-20*

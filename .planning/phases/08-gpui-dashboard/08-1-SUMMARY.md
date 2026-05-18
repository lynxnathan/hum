---
phase: 08-gpui-dashboard
plan: 1
subsystem: transport
tags: [transport-protocol, cli, osc, amplitude, time-parsing]

# Dependency graph
requires:
  - phase: 04-transport-e2e
    provides: TransportCmd/TransportReply protocol, unix socket server/client, CLI dispatch
provides:
  - TransportCmd::PlayFrom { pos } for atomic seek+play
  - TransportReply::Status.amplitudes HashMap for VU meter data
  - parse_time_arg supporting XmYs format (1m30s)
  - get_node_amplitude placeholder on ScsynthClient
affects: [08-gpui-dashboard plans 2+, GPUI watch client, VU meters]

# Tech tracking
tech-stack:
  added: []
  patterns: [placeholder-then-wire for OSC amplitude queries]

key-files:
  created: []
  modified: [src/transport.rs, src/main.rs, src/osc/bridge.rs]

key-decisions:
  - "Synchronous placeholder for get_node_amplitude (returns 0.0) — avoids async UDP recv blocking on Status handler until /s_get is wired"
  - "PlayFrom is a distinct TransportCmd variant, not Seek+Play composition — atomic semantics, single round-trip"
  - "Amplitudes as sparse HashMap<String, f32> — omit keys for nodes without data rather than sending 0.0 for all"

patterns-established:
  - "Time format parsing: XmYs (1m30s -> 90.0) in parse_time_arg"
  - "Protocol extension: add field with default-friendly type (HashMap), update print_reply to destructure with _"

requirements-completed: [XFIX-01, TUI-02]

# Metrics
duration: 2min
completed: 2026-03-22
---

# Phase 8 Plan 1: GPUI Dashboard + Transport Fix Summary

**Atomic PlayFrom transport command with amplitude-ready Status reply and XmYs time parsing**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-22T03:13:33Z
- **Completed:** 2026-03-22T03:15:08Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- TransportCmd::PlayFrom { pos } enables atomic seek+play in one command (no more two-step Seek then Play)
- TransportReply::Status now includes amplitudes HashMap for GPUI VU meter data
- CLI dispatch fixed: `hum play from 1m30s` sends PlayFrom { pos: 90.0 }
- parse_time_arg extended: supports "1m30s", "2m", "30s", bare "90" formats

## Task Commits

Each task was committed atomically:

1. **Task 1: Add PlayFrom to TransportCmd and amplitude to TransportReply::Status** - `c7b25db` (feat)
2. **Task 2: Implement PlayFrom handler + amplitude polling + CLI dispatch + time parsing** - `8e73c07` (feat)

## Files Created/Modified
- `src/transport.rs` - Added PlayFrom variant, amplitudes field in Status reply
- `src/main.rs` - PlayFrom handler, Status amplitude collection, CLI dispatch fix, XmYs time parsing
- `src/osc/bridge.rs` - get_node_amplitude placeholder method on ScsynthClient

## Decisions Made
- Synchronous placeholder for get_node_amplitude (returns 0.0) to avoid blocking Status handler -- real /n_get + /n_set wiring deferred to when GPUI VU meters render
- PlayFrom as distinct variant rather than composing Seek+Play -- cleaner semantics, single round-trip, no race between two commands
- Amplitudes field uses underscore binding in print_reply -- terminal output unchanged, field exists for GPUI watch client

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Transport protocol ready for GPUI watch client (Plan 2+)
- PlayFrom command live for dashboard transport controls
- Amplitude data placeholder in place -- wire real /n_get when VU meters render

## Self-Check: PASSED

- [x] src/transport.rs exists
- [x] src/main.rs exists
- [x] src/osc/bridge.rs exists
- [x] Commit c7b25db found
- [x] Commit 8e73c07 found
- [x] cargo build clean (0 errors)
- [x] cargo test: 129 passed, 0 failed

---
*Phase: 08-gpui-dashboard*
*Completed: 2026-03-22*

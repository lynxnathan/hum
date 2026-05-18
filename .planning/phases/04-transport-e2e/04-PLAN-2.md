---
phase: 04-transport-e2e
plan: 2
type: execute
wave: 1
depends_on: []
files_modified:
  - src/state.rs
  - src/reconciler.rs
  - src/main.rs
autonomous: true
requirements:
  - XPORT-04
  - XPORT-05

must_haves:
  truths:
    - "hum solo <thing> causes all other active things to be freed from scsynth"
    - "hum mute <thing> causes that specific thing to be freed from scsynth"
    - "After piece.hum reloads, solo/mute sets are NOT cleared — effect persists"
    - "The reconciler diff respects solo/mute when computing Add ops"
  artifacts:
    - path: "src/state.rs"
      provides: "solo_set and mute_set fields on StateStore, active_things_filtered() method"
      exports: ["active_things_filtered"]
    - path: "src/reconciler.rs"
      provides: "No change needed — diff() works on whatever active map it receives"
  key_links:
    - from: "src/main.rs handle_transport"
      to: "src/state.rs solo_set / mute_set"
      via: "state.solo_set.insert(thing) / state.mute_set.insert(thing)"
      pattern: "solo_set.insert|mute_set.insert"
    - from: "src/main.rs reconcile_now"
      to: "src/state.rs active_things_filtered"
      via: "state.active_things_filtered(state.playback_pos)"
      pattern: "active_things_filtered"
---

<objective>
Implement solo/mute logic so it persists across file reloads. The key insight: solo_set and mute_set live in StateStore (not derived from the file), so they survive `piece.hum` reparses. The reconciler receives an already-filtered active map and needs no changes.

Purpose: Users can solo a drone while tweaking other things, save piece.hum, and the solo stays locked.

Output: `active_things_filtered()` in StateStore that applies solo/mute, updated reconcile_now and handle_tick to use it, solo/mute TransportCmd handlers in main.rs.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md

<interfaces>
<!-- Types this plan builds on. Plan 1 adds playing/solo_set/mute_set/loop_range to StateStore.
     If Plan 1 and Plan 2 execute in parallel, this plan should add those fields here if not present,
     or verify they were added. Either plan may add them — coordinate by checking what's in state.rs. -->

From src/state.rs (after Plan 1 additions):
```rust
pub struct StateStore {
    pub desired: Option<Piece>,
    pub actual: ActualState,
    pub playback_pos: f64,
    pub playing: bool,
    pub loop_range: Option<(f64, f64)>,
    pub solo_set: std::collections::HashSet<String>,
    pub mute_set: std::collections::HashSet<String>,
}

// existing method:
pub fn active_things(&self, pos: f64) -> IndexMap<String, &ThingDef>
```

From src/reconciler.rs:
```rust
pub fn diff(active: &IndexMap<String, &ThingDef>, actual: &ActualState) -> Vec<ReconcileOp>
// No changes needed — receives filtered map
```

From src/main.rs:
```rust
// reconcile_now uses state.active_things() — must be changed to active_things_filtered()
// handle_tick uses state.active_things() — must be changed to active_things_filtered()
// handle_transport solo/mute arms: state.solo_set.insert / state.mute_set.insert then reconcile
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: active_things_filtered() in StateStore with unit tests</name>
  <files>src/state.rs</files>
  <behavior>
    - With empty solo_set and empty mute_set: result equals active_things() (no filtering)
    - With solo_set = {"drone"}: only "drone" appears if it is active; all other active things are excluded
    - With mute_set = {"pad"}: "pad" is excluded even if active; others remain
    - With solo_set = {"drone"} and mute_set = {"drone"}: mute wins, "drone" is excluded (mute takes precedence)
    - Solo of a thing not currently active: result is empty (solo active set intersected with solo_set)
    - After solo_set and mute_set are cleared (both empty), returns full active set again
  </behavior>
  <action>
Add `active_things_filtered()` to StateStore in `src/state.rs`:

```rust
/// Return active things at `pos`, filtered by solo_set and mute_set.
/// Solo logic: if solo_set is non-empty, ONLY things in solo_set are allowed through.
/// Mute logic: things in mute_set are always excluded (mute overrides solo).
pub fn active_things_filtered(&self, pos: f64) -> IndexMap<String, &ThingDef> {
    let all_active = self.active_things(pos);
    all_active
        .into_iter()
        .filter(|(name, _)| {
            // Mute always excludes
            if self.mute_set.contains(name) {
                return false;
            }
            // Solo: if solo_set is non-empty, only allow things in solo_set
            if !self.solo_set.is_empty() && !self.solo_set.contains(name) {
                return false;
            }
            true
        })
        .collect()
}
```

Write tests in `src/state.rs` `#[cfg(test)]` block covering all behavior cases above.

Also ensure StateStore has `solo_set` and `mute_set` fields if not already added by Plan 1. If the fields are missing, add them:
```rust
pub solo_set: std::collections::HashSet<String>,
pub mute_set: std::collections::HashSet<String>,
```
And initialize in `new()`:
```rust
solo_set: std::collections::HashSet::new(),
mute_set: std::collections::HashSet::new(),
```
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo test state:: 2>&1 | tail -20</automated>
  </verify>
  <done>All state:: tests pass including new active_things_filtered tests. RED (failing test) committed before implementation, GREEN after.</done>
</task>

<task type="auto">
  <name>Task 2: Wire active_things_filtered into reconcile paths + solo/mute handlers</name>
  <files>src/main.rs</files>
  <action>
**Replace `active_things()` calls with `active_things_filtered()`** in two places:

1. `reconcile_now()` function:
```rust
// Before:
let active = state.active_things(state.playback_pos);
// After:
let active = state.active_things_filtered(state.playback_pos);
```

2. `handle_tick()` function:
```rust
// Both the old_active_keys computation and the new_active computation:
let old_active_keys: Vec<String> = state
    .active_things_filtered(state.playback_pos)  // was active_things
    .into_keys()
    .collect();
// ...
let new_active = state.active_things_filtered(pos);  // was active_things
```

**Solo handler** in `handle_transport()` (Solo arm):
```rust
TransportCmd::Solo { thing } => {
    state.solo_set.clear();
    state.solo_set.insert(thing);
    reconcile_now(state, client).await;
    let _ = reply_tx.send(TransportReply::Ack);
}
```
Note: solo replaces any existing solo (single-thing solo model). To un-solo, the user would need a `hum unsolo` command — out of scope for this phase. For now, `hum solo <thing>` always replaces.

**Mute handler** in `handle_transport()` (Mute arm):
```rust
TransportCmd::Mute { thing } => {
    state.mute_set.insert(thing);
    reconcile_now(state, client).await;
    let _ = reply_tx.send(TransportReply::Ack);
}
```
Note: mute is additive (can mute multiple things). This is the right behavior per XPORT-05.

**Status handler** — ensure it reads solo/mute sets:
```rust
TransportCmd::Status => {
    let active: Vec<String> = state
        .active_things_filtered(state.playback_pos)
        .into_keys()
        .collect();
    let _ = reply_tx.send(TransportReply::Status {
        playing: state.playing,
        pos: state.playback_pos,
        active,
        solo: state.solo_set.iter().cloned().collect(),
        mute: state.mute_set.iter().cloned().collect(),
    });
}
```
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo test 2>&1 | tail -20</automated>
  </verify>
  <done>cargo test passes all tests. reconcile_now and handle_tick use active_things_filtered. Solo and mute handlers reconcile immediately after state update.</done>
</task>

</tasks>

<verification>
`cargo test` passes all tests including new active_things_filtered unit tests. `cargo build` is clean. Solo/mute survive the reconcile path because the filter is applied at reconcile time using current solo_set/mute_set — which are never cleared on file reload.
</verification>

<success_criteria>
- active_things_filtered() exists in StateStore with 6+ unit tests covering all filter combinations
- reconcile_now() and handle_tick() use active_things_filtered()
- Solo handler: clears solo_set, inserts new thing, reconciles
- Mute handler: inserts into mute_set (additive), reconciles
- Status reply includes solo and mute sets
- All cargo tests pass
</success_criteria>

<output>
After completion, create `.planning/phases/04-transport-e2e/04-2-SUMMARY.md` using the summary template.
</output>

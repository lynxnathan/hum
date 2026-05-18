---
phase: 03-state-reconciler-watcher
plan: 1
type: tdd
wave: 1
depends_on: []
files_modified:
  - Cargo.toml
  - src/events.rs
  - src/state.rs
  - src/reconciler.rs
autonomous: true
requirements:
  - WATCH-02
  - TIME-01
  - TIME-02
  - TIME-03
must_haves:
  truths:
    - "ReconcileOp::Add is produced when a thing is active but not running"
    - "ReconcileOp::Remove is produced when a thing is running but no longer active"
    - "active_things returns only things where at <= pos AND (no until OR pos < until)"
    - "Things with no until: remain in active_things indefinitely once activated"
    - "parse_seconds('10s') == Some(10.0); parse_seconds('10') == None"
  artifacts:
    - path: "src/events.rs"
      provides: "DaemonEvent enum (FileChanged, Tick)"
      exports: ["DaemonEvent"]
    - path: "src/state.rs"
      provides: "StateStore with desired/actual/playback_pos + active_things filter"
      exports: ["StateStore", "ActualState", "parse_seconds"]
    - path: "src/reconciler.rs"
      provides: "diff() pure function returning Vec<ReconcileOp>"
      exports: ["ReconcileOp", "diff"]
  key_links:
    - from: "src/reconciler.rs"
      to: "src/state.rs"
      via: "ActualState.nodes and active_things return type"
      pattern: "use crate::state::ActualState"
    - from: "src/state.rs"
      to: "src/parser/types.rs"
      via: "Piece and ThingDef from existing parser"
      pattern: "use crate::parser::types"
---

<objective>
Build the pure-logic core: event enum, state store, and reconciler diff function. No I/O touches this plan — all three modules are unit-testable in isolation.

Purpose: Establishes the data contracts (DaemonEvent, StateStore, ReconcileOp) that watcher, timeline, and main all depend on. Pure logic = fast feedback via tests.
Output: events.rs, state.rs, reconciler.rs — all tested, Cargo deps added.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/phases/03-state-reconciler-watcher/03-CONTEXT.md
@.planning/phases/03-state-reconciler-watcher/research/RESEARCH.md

<interfaces>
<!-- Existing types the executor must use directly — no codebase exploration needed. -->

From src/parser/types.rs:
```rust
pub type Piece = IndexMap<String, ThingDef>;

pub struct ThingDef {
    pub at: Option<String>,      // e.g. "0s", "10s"
    pub until: Option<String>,   // e.g. "30s" — absent = open-ended
    pub does: Option<DoesField>,
    pub location: Option<String>,
    pub has: Option<IndexMap<String, ThingDef>>,
    pub within: Option<String>,
    pub every: Option<String>,
    pub like: Option<String>,
    pub reference: Option<String>,
    pub mood: Option<String>,
}
```

From src/osc/bridge.rs (ScsynthClient — actuator, used later in Plan 3):
```rust
pub async fn load_synthdef(&mut self, synthdef_bytes: Vec<u8>) -> Result<()>
pub async fn new_synth(&mut self, thing_name: &str, synthdef_name: &str) -> Result<i32>
pub async fn free_node(&mut self, thing_name: &str) -> Result<()>
pub async fn free_all_nodes(&mut self) -> Result<()>
```
</interfaces>
</context>

<feature>
  <name>Event enum + StateStore + reconciler diff</name>
  <files>src/events.rs, src/state.rs, src/reconciler.rs</files>
  <behavior>
    parse_seconds:
    - "10s" -> Some(10.0)
    - "0s" -> Some(0.0)
    - "10" -> None (no suffix)
    - "" -> None

    active_things(pos):
    - thing with at="0s", no until -> active at pos=0.0, pos=999.0
    - thing with at="10s", no until -> inactive at pos=5.0, active at pos=10.0
    - thing with at="0s", until="30s" -> active at pos=15.0, inactive at pos=30.0
    - thing with at="10s", until="20s" -> inactive at pos=9.9, active at pos=10.0, inactive at pos=20.0
    - thing with no at field -> treated as at=0s (active from start)

    diff(active, actual):
    - active has "foo", actual.nodes does not -> [Add { thing_name: "foo", synthdef_name: "foo" }]
    - active is empty, actual.nodes has "foo" -> [Remove { thing_name: "foo" }]
    - active has "foo", actual.nodes has "foo" -> [] (no op — thing name is identity key)
    - active has "foo" and "bar", actual.nodes has "foo" -> [Add { thing_name: "bar", synthdef_name: "bar" }]
  </behavior>
  <implementation>
    1. Add to Cargo.toml: `notify = "7"` and `notify-debouncer-full = "0.4"` (needed by Plan 2).

    2. Create src/events.rs:
    ```rust
    use std::path::PathBuf;

    pub enum DaemonEvent {
        FileChanged(PathBuf),
        Tick(f64),
    }
    ```

    3. Create src/state.rs following Pattern 3 from RESEARCH.md exactly:
    - StateStore { desired: Option<Piece>, actual: ActualState, playback_pos: f64 }
    - ActualState { nodes: IndexMap<String, i32> }
    - active_things(&self, pos: f64) -> IndexMap<String, &ThingDef>
    - is_active(thing, pos) helper: at defaults to 0.0 if absent or unparseable
    - pub fn parse_seconds(s: &str) -> Option<f64> — strip_suffix('s') then parse

    4. Create src/reconciler.rs following Pattern 4 from RESEARCH.md:
    - ReconcileOp enum: Add { thing_name, synthdef_name }, Remove { thing_name }, Swap { thing_name, new_synthdef_name }
    - diff(active: &IndexMap<String, &ThingDef>, actual: &ActualState) -> Vec<ReconcileOp>
    - Thing name == SynthDef name by convention (name is the stable identity key)
    - Swap variant is emitted externally (SCD hot-swap path) — diff only emits Add/Remove

    Write tests inline in each file using #[cfg(test)] mod tests.
  </implementation>
</feature>

<verification>
```
cargo test state -- --nocapture
cargo test reconciler -- --nocapture
cargo check
```
All tests green, cargo check clean.
</verification>

<success_criteria>
- `cargo test` passes with tests covering all behavior cases above
- `cargo check` has zero errors
- DaemonEvent, StateStore, ActualState, ReconcileOp, diff, parse_seconds all exported from their modules
- No I/O, no async in these three files
</success_criteria>

<output>
After completion, create `.planning/phases/03-state-reconciler-watcher/03-1-SUMMARY.md`
</output>

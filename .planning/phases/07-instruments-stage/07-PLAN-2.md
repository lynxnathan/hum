---
phase: 07-instruments-stage
plan: 2
type: execute
wave: 2
depends_on: [07-PLAN-1]
files_modified:
  - src/stage.rs
  - src/osc/bridge.rs
  - src/reconciler.rs
  - src/main.rs
autonomous: true
requirements: [STAGE-01, STAGE-02, STAGE-03]

must_haves:
  truths:
    - "A type: stage thing with applies-to: [a, b] routes things a and b through a shared scsynth Group"
    - "The stage's fx: chain runs as an effect node on the group bus tail"
    - "Things not in any stage continue to play in the default group (node 1) — no regression"
  artifacts:
    - path: "src/stage.rs"
      provides: "StageStore: maps stage names to StageConfig (applies_to, fx chain)"
      exports: ["StageStore", "StageConfig"]
    - path: "src/osc/bridge.rs"
      provides: "ScsynthClient::create_group(), add_node_to_group(), load_effect_synthdef()"
      contains: "create_group"
    - path: "src/reconciler.rs"
      provides: "ReconcileOp::AddStage variant, stage diff logic"
      contains: "AddStage"
    - path: "src/main.rs"
      provides: "startup creates stage groups, routes things, applies fx chain"
      contains: "StageStore"
  key_links:
    - from: "src/main.rs"
      to: "src/osc/bridge.rs"
      via: "client.create_group(group_id) before /s_new for staged things"
      pattern: "create_group"
    - from: "src/osc/bridge.rs"
      to: "scsynth OSC"
      via: "/g_new for group creation, /s_new with target group_id for effect node"
      pattern: "g_new"
---

<objective>
Implement type: stage things that route a list of things through a shared scsynth Group node and apply an fx chain on the group bus.

Purpose: Lets composers apply reverb/delay to a group of instruments with one declaration instead of per-thing fx fields.
Output: StageStore + ScsynthClient group OSC methods + reconciler stage ops + main.rs wiring.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/phases/07-instruments-stage/07-CONTEXT.md
@.planning/phases/07-instruments-stage/07-1-SUMMARY.md

<interfaces>
<!-- Key contracts the executor needs. -->

From src/parser/types.rs (after Plan 1):
```rust
pub struct ThingDef {
    // ...existing fields...
    #[serde(rename = "type")]
    pub thing_type: Option<ThingType>,  // Some(ThingType::Stage) for stage things
    pub instrument: Option<String>,
    #[serde(rename = "applies-to")]
    pub applies_to: Option<Vec<String>>,
    pub synth: Option<SynthBlock>,      // fx: is inside synth block on stage things
}

pub enum ThingType { Instrument, Stage }
```

Stage .hum example:
```yaml
haunted-stage:
  type: stage
  applies-to: [ghost-machine, glass, bass-drop]
  fx: reverb(mix: 0.7, room: 0.95)
```

Note: `fx:` on a stage thing is a top-level field, not inside synth:. ThingDef needs a top-level `fx` field OR stage things put fx: inside synth:. Simplest: add `pub fx: Option<FxPrimitive>` directly to ThingDef (alongside synth:), so stages write `fx: reverb(...)` without needing a synth: wrapper. Add this field to ThingDef in src/parser/types.rs as part of this plan.

scsynth OSC for groups:
- /g_new [group_id: Int, add_action: Int, target: Int] — creates a group node
  - add_action 0 = add to head of target group
  - target 1 = default group (root)
- /s_new [def_name: Str, node_id: Int, add_action: Int, target: Int, ...params]
  - add_action 1 = add to tail of target (use for effect node on group tail)
  - add_action 0 = add to head of target (use for source nodes within group)

Existing ScsynthClient methods:
```rust
pub async fn load_synthdef(&mut self, bytes: Vec<u8>) -> Result<()>
pub async fn start_synth(&mut self, name: &str, params: &[(&str, f32)]) -> Result<i32>
pub async fn set_params(&self, node_id: i32, params: &[(&str, f32)]) -> Result<()>
pub async fn free_node(&mut self, node_id: i32) -> Result<()>
```

From src/ir/compiler.rs (existing):
```rust
pub fn compile_synth_block(name: &str, block: &SynthBlock) -> Result<Vec<u8>>
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: StageStore + ScsynthClient group methods</name>
  <files>src/stage.rs, src/osc/bridge.rs, src/parser/types.rs</files>
  <action>
    **src/parser/types.rs:**
    Add top-level `fx` field to ThingDef (for stage things that write `fx:` without a synth: wrapper):
    ```rust
    use crate::ir::types::FxPrimitive;
    pub fx: Option<FxPrimitive>,
    ```
    This field is None for normal things and instruments, Some for stages.

    **src/stage.rs:**
    ```rust
    use crate::ir::types::FxPrimitive;

    #[derive(Debug, Clone)]
    pub struct StageConfig {
        pub applies_to: Vec<String>,
        pub fx: Option<FxPrimitive>,
        pub group_id: i32,         // allocated scsynth node ID for the Group
        pub effect_node_id: i32,   // allocated scsynth node ID for the effect synth
    }

    pub struct StageStore {
        stages: HashMap<String, StageConfig>,
    }

    impl StageStore {
        pub fn new() -> Self;
        /// Returns which group_id a thing should be spawned into (or None = default group 1)
        pub fn group_for_thing(&self, thing_name: &str) -> Option<i32>;
        pub fn insert(&mut self, name: String, config: StageConfig);
        pub fn iter(&self) -> impl Iterator<Item = (&String, &StageConfig)>;
    }
    ```

    group_for_thing: iterates all stages, finds one whose applies_to contains thing_name, returns its group_id.

    **src/osc/bridge.rs:**
    Add three methods to ScsynthClient:

    ```rust
    /// Create a scsynth Group node. /g_new [id, 0, 1] (head of default group)
    pub async fn create_group(&mut self) -> Result<i32> {
        let id = self.alloc_node_id();
        self.send_message("/g_new", vec![
            OscType::Int(id),
            OscType::Int(0),  // add_action: add to head
            OscType::Int(1),  // target: default group
        ]).await?;
        Ok(id)
    }

    /// Spawn a synth node inside a specific group (add to head of group).
    /// Used for routing source things into a stage group.
    pub async fn start_synth_in_group(
        &mut self, def_name: &str, group_id: i32, params: &[(&str, f32)]
    ) -> Result<i32>;

    /// Spawn an effect synth at the tail of a group (processes group output).
    pub async fn start_effect_at_tail(
        &mut self, def_name: &str, group_id: i32, params: &[(&str, f32)]
    ) -> Result<i32>;
    ```

    start_synth_in_group: /s_new [def_name, node_id, 0 (addToHead), group_id, ...params]
    start_effect_at_tail: /s_new [def_name, node_id, 1 (addToTail), group_id, ...params]

    alloc_node_id is the same as existing next_node_id pattern. Add private helper if not already factored.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build 2>&1 | tail -20</automated>
  </verify>
  <done>cargo build succeeds. StageStore, ScsynthClient group methods compile with no errors.</done>
</task>

<task type="auto">
  <name>Task 2: Stage detection + main.rs wiring</name>
  <files>src/main.rs</files>
  <action>
    Wire stage lifecycle into the startup and event loop in main.rs. Stages are identified by `thing_type: Some(ThingType::Stage)` in ThingDef.

    **Startup (after instrument_store, before reconciler loop):**

    1. Parse piece.hum (already done). Filter things where `thing.thing_type == Some(ThingType::Stage)`.

    2. For each stage thing:
       a. `let group_id = client.create_group().await?;`
       b. Compile an effect SynthDef for the stage's fx chain. Strategy: build a minimal SynthBlock with only `fx` set, compile it under name `"stage-{stage_name}"`. Use the existing compile_synth_block.
       c. `client.load_synthdef(effect_bytes).await?;`
       d. `let effect_node_id = client.start_effect_at_tail("stage-{name}", group_id, &[]).await?;`
       e. Insert into StageStore.

    3. When routing normal (non-stage) things through reconciler, check `stage_store.group_for_thing(name)`:
       - If Some(group_id) → use `client.start_synth_in_group(def_name, group_id, params)` instead of `client.start_synth(def_name, params)`
       - If None → use existing `client.start_synth(def_name, params)` (no change)

    **Hot-swap (piece.hum file change):**
    Stage things are structural (not time-based), so on piece.hum reload: if stage definitions changed, log a warning "stage hot-swap not supported — restart hum-rt to reconfigure stages". This is acceptable for v2. Do not attempt live stage reconfiguration.

    **Effect SynthDef compilation:**
    The effect node reads from the group bus and applies the fx. For simplicity in v2: generate a SynthDef that uses `In.ar` on a bus, applies the FxPrimitive, and outputs via `Out.ar`. Since the IR compiler generates SCgf binary from SynthBlock, create a helper `compile_stage_effect(name: &str, fx: &FxPrimitive) -> Result<Vec<u8>>` in src/stage.rs that builds a minimal SynthBlock (fx field only, osc: sine as placeholder carrier for the bus read) and calls compile_synth_block. The executor should use best judgment on the scsynth bus architecture — the critical requirement is that the group exists and effect node is at tail.

    Add `mod stage;` and `use stage::StageStore;` to main.rs.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build 2>&1 | tail -20</automated>
  </verify>
  <done>
    cargo build succeeds. Stage things detected from piece.hum. Group created in scsynth. Effect node spawned at group tail. Source things in applies-to list spawned inside group. Non-staged things unaffected.
  </done>
</task>

</tasks>

<verification>
1. `cargo test` — all tests pass (no regressions)
2. `cargo build` — clean build
3. Manual: add a stage thing to piece.hum with `type: stage`, `applies-to: [thing-a]`, `fx: reverb(mix: 0.5, room: 0.8)` → hum-rt starts without error, scsynth receives /g_new followed by /d_recv + /s_new for the effect node
4. thing-a is spawned with /s_new targeting the group node, not the default group
</verification>

<success_criteria>
- Stage things parsed from piece.hum via thing_type: Stage
- scsynth Group created per stage; effect SynthDef loaded at group tail
- Things in applies-to spawned into the group (not default group 1)
- Things NOT in any stage are unaffected
- Non-fatal on missing instruments/ dir, non-fatal on no stages in piece.hum
</success_criteria>

<output>
After completion, create `.planning/phases/07-instruments-stage/07-2-SUMMARY.md`
</output>

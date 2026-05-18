---
phase: 05-synth-ir
plan: 3
type: execute
wave: 3
depends_on: [05-1, 05-2]
files_modified:
  - src/main.rs
  - src/reconciler.rs
  - src/ir/sequencer.rs
  - src/ir/mod.rs
autonomous: false
requirements: [IR-08, IR-10, IR-11]

must_haves:
  truths:
    - "A thing with synth: block plays sound without any .scd file present"
    - "Editing synth: fields while playing updates the sound within ~1 second"
    - "When out/sc/<thing>.scd exists, .scd takes precedence over synth: block"
    - "A thing with neither synth: nor .scd logs a clear error and is skipped"
  artifacts:
    - path: "src/main.rs"
      provides: "startup IR compilation + .hum hot-swap with IR recompilation"
      contains: "compile_synth_block"
    - path: "src/reconciler.rs"
      provides: "ReconcileOp::Add carries SynthDef bytes or signals lookup"
  key_links:
    - from: "src/main.rs"
      to: "src/ir/compiler.rs"
      via: "compile_synth_block() called during startup and on .hum file change"
      pattern: "compile_synth_block"
    - from: "src/main.rs"
      to: "src/scd/store.rs"
      via: "scd_store.get(name) checked first (escape hatch precedence)"
      pattern: "scd_store.get"
---

<objective>
Wire the IR compiler into the runtime: load SynthDef bytes from IR (or .scd escape hatch) at startup and on .hum file change. Hot-swap on edit triggers recompilation and /d_recv reload.

Purpose: Closes the loop — synth: blocks actually play through scsynth. This is the Phase 5 success criterion.
Output: hum-rt plays a .hum file with `synth:` block and no .scd file. Editing synth: fields while playing updates sound within ~1s.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/phases/05-synth-ir/05-CONTEXT.md
@.planning/phases/05-synth-ir/research/RESEARCH.md
@.planning/phases/05-synth-ir/05-1-SUMMARY.md
@.planning/phases/05-synth-ir/05-2-SUMMARY.md
@src/main.rs
@src/reconciler.rs
@src/scd/store.rs

<interfaces>
<!-- Key contracts this plan wires together -->

From src/ir/compiler.rs (Plan 2):
```rust
pub fn compile_synth_block(name: &str, block: &SynthBlock) -> anyhow::Result<Vec<u8>>;
```

From src/scd/store.rs (existing):
```rust
pub fn get(&self, thing_name: &str) -> Option<&[u8]>;
```

From src/osc/bridge.rs (existing):
```rust
pub async fn load_synthdef(&mut self, synthdef_bytes: Vec<u8>) -> Result<()>;
pub async fn new_synth(&mut self, thing_name: &str, synthdef_name: &str) -> Result<i32>;
```

From src/parser/types.rs (Plan 1):
```rust
pub struct ThingDef {
    // ... existing fields ...
    pub synth: Option<SynthBlock>,
}
```

Escape hatch precedence rule (from RESEARCH.md):
```
// 1. scd_store.get(name) is_some() → use .scd bytes (escape hatch wins)
// 2. thing.synth.is_some() → compile_synth_block(name, synth)
// 3. else → log error, skip this thing
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Startup IR compilation + escape hatch precedence</name>
  <files>src/main.rs</files>
  <action>
    Modify the startup sequence in `main()` to compile SynthDefs from IR when no .scd override exists.

    After `ScdStore::load_dir()` and before `reconcile_now()`, add an IR compilation pass:

    ```rust
    // After parsing piece.hum into state.desired:
    if let Some(piece) = &state.desired {
        for (name, thing) in piece.iter() {
            if scd_store.get(name).is_some() {
                // Escape hatch: .scd exists, already loaded above, skip IR
                tracing::info!("'{}': using .scd escape hatch", name);
                continue;
            }
            if let Some(synth_block) = &thing.synth {
                match ir::compile_synth_block(name, synth_block) {
                    Ok(bytes) => {
                        match client.load_synthdef(bytes).await {
                            Ok(()) => tracing::info!("'{}': compiled IR and loaded SynthDef", name),
                            Err(e) => tracing::error!("'{}': failed to load compiled IR: {}", name, e),
                        }
                    }
                    Err(e) => tracing::error!("'{}': IR compilation failed: {}", name, e),
                }
            } else {
                tracing::warn!("'{}': no synth: block and no .scd file — will not play", name);
            }
        }
    }
    ```

    The ScdStore startup loop (which calls load_synthdef for .scd files) remains unchanged — it runs first. The IR loop only runs for things NOT covered by .scd.

    No changes to reconciler needed: `ReconcileOp::Add` already uses thing_name == synthdef_name by convention. The SynthDef is loaded before /s_new is called, so new_synth() will find it.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo check 2>&1 | tail -20</automated>
  </verify>
  <done>cargo check passes; startup path compiles IR for synth: things; .scd escape hatch is checked first</done>
</task>

<task type="auto">
  <name>Task 2: Hot-swap IR on .hum file change</name>
  <files>src/main.rs</files>
  <action>
    Modify `handle_file_change()` in the "hum" branch to recompile IR when piece.hum changes:

    After `state.desired = Some(piece)` and before `reconcile_now(state, client)`, add IR recompilation for any thing that changed:

    ```rust
    // Recompile IR for things with synth: blocks (no .scd override)
    // Check out/sc/ for escape hatch on each thing
    let scd_dir = std::path::Path::new("out/sc");
    for (name, thing) in piece_ref.iter() {
        // Escape hatch: if .scd file exists on disk, skip IR
        let scd_path = scd_dir.join(format!("{}.scd", name));
        if scd_path.exists() {
            continue;
        }
        if let Some(synth_block) = &thing.synth {
            match ir::compile_synth_block(name, synth_block) {
                Ok(bytes) => {
                    match client.load_synthdef(bytes).await {
                        Ok(()) => {
                            tracing::info!("'{}': hot-swap IR recompiled", name);
                            // If thing is running, trigger a node swap
                            if state.actual.nodes.contains_key(name.as_str()) {
                                match client.new_synth(name, name).await {
                                    Ok(node_id) => {
                                        state.actual.nodes.insert(name.clone(), node_id);
                                        tracing::info!("'{}': hot-swapped node after IR change", name);
                                    }
                                    Err(e) => tracing::error!("'{}': hot-swap new_synth failed: {}", name, e),
                                }
                            }
                        }
                        Err(e) => tracing::error!("'{}': IR load failed on hot-swap: {}", name, e),
                    }
                }
                Err(e) => tracing::error!("'{}': IR recompilation failed: {}", name, e),
            }
        }
    }
    ```

    Note: the `piece_ref` is the newly-parsed piece (held in a variable before moving into `state.desired`). Restructure the "hum" branch to keep a reference: parse into `piece`, iterate over it for IR recompilation, then assign `state.desired = Some(piece)`, then call `reconcile_now`.

    This preserves the existing .scd hot-swap path in `handle_scd_change()` unchanged.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo test 2>&1 | tail -20</automated>
  </verify>
  <done>cargo test passes; .hum file change triggers IR recompilation and node hot-swap if thing is active</done>
</task>

<task type="auto">
  <name>Task 3: Note sequencer — tempo + notes scheduling</name>
  <files>src/ir/sequencer.rs, src/ir/mod.rs, src/main.rs</files>
  <action>
    Implement note sequencing for things with `notes:` and `tempo:` fields in their synth: block.

    **src/ir/sequencer.rs** — note scheduling engine:
    ```rust
    use tokio::sync::mpsc::Sender;
    use crate::ir::notes::note_to_midi;

    pub struct NoteSequencer {
        pub thing_name: String,
        pub notes: Vec<String>,      // ["D4", "D4", "Eb4", "-"]
        pub tempo: f64,              // seconds per note
    }

    impl NoteSequencer {
        /// Spawn a tokio task that sends /n_set freq commands at tempo intervals.
        /// Returns the JoinHandle so it can be aborted on stop/hot-swap.
        pub fn spawn(
            &self,
            client_tx: Sender<SequencerEvent>,
        ) -> tokio::task::JoinHandle<()> {
            let notes = self.notes.clone();
            let tempo = self.tempo;
            let thing_name = self.thing_name.clone();

            tokio::spawn(async move {
                let mut interval = tokio::time::interval(
                    std::time::Duration::from_secs_f64(tempo)
                );
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                let mut idx = 0;
                loop {
                    interval.tick().await;
                    let note = &notes[idx % notes.len()];
                    if note != "-" {
                        if let Some(midi) = note_to_midi(note) {
                            let freq = 440.0 * 2.0_f64.powf((midi as f64 - 69.0) / 12.0);
                            let _ = client_tx.send(SequencerEvent::SetFreq {
                                thing_name: thing_name.clone(),
                                freq: freq as f32,
                            }).await;
                        }
                    }
                    idx += 1;
                }
            })
        }
    }

    pub enum SequencerEvent {
        SetFreq { thing_name: String, freq: f32 },
    }
    ```

    **src/ir/mod.rs** — add `pub mod sequencer;`

    **src/main.rs** — wire sequencer into the event loop:
    - Add a `HashMap<String, JoinHandle<()>>` for active sequencer tasks
    - When a thing with `notes:` + `tempo:` is Added by reconciler, spawn its sequencer
    - When a thing is Removed, abort its sequencer handle
    - Add a `SequencerEvent` channel that the event loop receives from and calls `client.set_param(thing, "freq", freq)`
    - On hot-swap, abort old sequencer and spawn new one with updated notes/tempo

    The sequencer sends /n_set freq commands to the already-running synth node. The SynthDef must have a `freq` control parameter (which all our IR-compiled SynthDefs do since osc: primitives use freq).
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build 2>&1 | tail -10</automated>
  </verify>
  <done>cargo build passes; sequencer module compiles; main.rs wires sequencer spawn/abort for things with notes+tempo</done>
</task>

<task type="checkpoint:human-verify" gate="blocking">
  <what-built>
    Full Phase 5 pipeline: synth: block in .hum → IR parse → SCgf binary → /d_recv → /s_new → audible sound. Hot-swap on edit. .scd escape hatch precedence.
  </what-built>
  <how-to-verify>
    1. Create a test piece at `/tmp/test-synth-ir.hum`:
       ```yaml
       test-sine:
         at: 0s
         synth:
           osc: sine
           filter: lpf(cutoff: 800)
           env: perc(attack: 0.01, release: 0.5)
           pan: center
           amp: 0.2
           notes: [D4 D4 Eb4 D4]
           tempo: 0.35s/note
       ```
    2. Make sure scsynth is running, then run: `cd /tmp && hum-rt` (pointing to test-synth-ir.hum)
    3. Run `hum-rt play` — you should hear a filtered sine playing the D4 D4 Eb4 D4 pattern
    4. Edit `amp: 0.2` → `amp: 0.5` in the .hum file and save — sound should get louder within ~1 second
    5. Edit `osc: sine` → `osc: saw` and save — timbre should change within ~1 second
    6. Create `out/sc/test-sine.scd` (any valid .scsyndef binary) — it should take precedence over the synth: block
    7. Delete `out/sc/test-sine.scd` — IR should resume

    Acceptance signals:
    - Step 3: audible sound, no scsynth error in logs
    - Step 4: amplitude change heard
    - Step 5: timbre change heard
    - Step 6: .scd version heard (different timbre than IR version)
    - Step 7: IR version resumes on next play
  </how-to-verify>
  <resume-signal>Type "approved" if all steps pass, or describe what failed</resume-signal>
</task>

</tasks>

<verification>
- cargo test passes
- A .hum file with only `synth:` block (no out/sc/ directory) produces sound
- A .hum file with both synth: block AND matching .scd file uses the .scd (escape hatch wins)
- Editing synth: fields while playing updates sound within ~1 second
</verification>

<success_criteria>
Phase 5 complete: hum-rt compiles synth: blocks directly to scsynth OSC with no sclang. All four success criteria from ROADMAP verified.
</success_criteria>

<output>
After completion, create `.planning/phases/05-synth-ir/05-3-SUMMARY.md`
</output>

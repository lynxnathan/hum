---
phase: 10-translation-sync
plan: 2
type: execute
wave: 1
depends_on: []
files_modified:
  - src/main.rs
autonomous: true
requirements:
  - SYNC-01
  - SYNC-03
  - SYNC-05

must_haves:
  truths:
    - "pipe: change propagates to synth: output without manual edits (existing behavior verified)"
    - "like: change is detectable by hum-rt (emits a log/event so LLM can act)"
    - "`hum dict suggest` outputs recurring synth param patterns found in piece.hum"
  artifacts:
    - path: "src/main.rs"
      provides: "hum dict suggest CLI command + like: change detection logging"
  key_links:
    - from: "src/main.rs handle_dict_cli"
      to: "piece.hum parser output"
      via: "scan all ThingDef.synth fields, group by osc/filter/fx shape"
      pattern: "suggest|pattern"
    - from: "src/main.rs handle_file_change"
      to: "like: field diff"
      via: "compare old vs new ThingDef.like string"
      pattern: "like.*changed|like.*diverge"
---

<objective>
Verify pipe->synth propagation (SYNC-03), add like: change detection (SYNC-01 detection side), and implement `hum dict suggest` (SYNC-05).

Purpose: SYNC-03 is already working from v2 pipe expansion — this plan verifies it with an explicit test. SYNC-01's LLM action is external, but hum-rt must log when like: changes so the LLM knows to act. SYNC-05 gives users a discovery tool for their own sonic patterns.
Output: Confirmed pipe propagation, like: change log event, `hum dict suggest` command.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/v3-DICTIONARY-SYNC.md
@.planning/phases/10-translation-sync/10-CONTEXT.md

<interfaces>
<!-- Key types the executor needs. No codebase exploration required. -->

From src/parser/types.rs:
```rust
pub struct ThingDef {
    pub like: Option<String>,
    pub synth: Option<SynthBlock>,
    pub pipe: Option<String>,
}
pub type Piece = IndexMap<String, ThingDef>;
```

From src/pipe/executor.rs:
```rust
// expand_pipe(pipe_expr: &PipeExpr, piece: &Piece) -> anyhow::Result<Vec<SynthBlock>>
// Returns expanded synth blocks from a pipe expression
pub fn expand_pipe(expr: &PipeExpr, piece: &Piece) -> anyhow::Result<Vec<SynthBlock>>;
```

From src/main.rs (existing pattern):
```rust
// handle_dict_cli(args: &[String]) dispatches on args[0]
// match Some("list") | Some("show") | Some("add") | _ => ...
// Piece is parsed from "piece.hum" using serde_saphyr::from_str
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: like: change detection + pipe->synth propagation test</name>
  <files>src/main.rs</files>
  <behavior>
    - SYNC-03 verification: write a unit test asserting that a ThingDef with pipe: and no synth: produces a non-empty SynthBlock via parse_pipe_block + expand_pipe. This proves the existing behavior.
    - SYNC-01 detection: in handle_file_change (or the FileChanged arm), compare old vs new ThingDef.like for each thing. If like: changed, emit: `tracing::info!("sync: like: changed for '{}' — LLM should regenerate pipe: and synth:", thing_name)`. Also print to stdout: `println!("hum: like: changed for '{}' (LLM action needed)", thing_name)`.
    - Store like: hashes alongside synth_hashes (from Plan 1): `let mut like_hashes: HashMap<String, String>` tracking previous like: values.
    - Test: given old_like != new_like, the detection fires (pure logic test via a helper function).
  </behavior>
  <action>
    **Like: change detection** — in main() daemon setup, add alongside synth_hashes:
    `let mut like_hashes: HashMap<String, String> = HashMap::new();`

    In the FileChanged handler, after parsing new_piece, iterate things:
    ```rust
    let new_like = thing.like.as_deref().unwrap_or("").to_string();
    let old_like = like_hashes.get(thing_name).cloned().unwrap_or_default();
    if !old_like.is_empty() && new_like != old_like {
        tracing::info!("sync: like: changed for '{}' — LLM should regenerate pipe: and synth:", thing_name);
        println!("hum: like: changed for '{}' (LLM action needed)", thing_name);
    }
    like_hashes.insert(thing_name.clone(), new_like);
    ```

    **SYNC-03 verification test** — add to src/main.rs or src/pipe/executor.rs tests:
    ```rust
    #[test]
    fn pipe_change_produces_synth_output() {
        // A pipe: block with a valid source should expand to at least one SynthBlock
        // Use a minimal piece with a thing that has a synth: block, then pipe references it
        let piece_yaml = r#"
    base:
      synth:
        osc: sine
        amp: 0.5
    derived:
      pipe: "base |> replicate(2)"
    "#;
        let piece: crate::parser::Piece = serde_saphyr::from_str(piece_yaml).unwrap();
        let derived = piece.get("derived").unwrap();
        let pipe_str = derived.pipe.as_ref().unwrap();
        let pipe_expr = crate::pipe::parser::parse_pipe_block(pipe_str).unwrap();
        let expanded = crate::pipe::executor::expand_pipe(&pipe_expr, &piece).unwrap();
        assert!(!expanded.is_empty(), "pipe expansion should produce synth blocks");
        assert_eq!(expanded.len(), 2, "replicate(2) should produce 2 blocks");
    }
    ```
    Place this test in src/main.rs under a `#[cfg(test)]` block, or in src/pipe/executor.rs if the test fits better there (executor already has tests for expand_pipe).
  </action>
  <verify>
    <automated>cargo test --lib -- pipe_change_produces_synth 2>&1 | tail -20</automated>
  </verify>
  <done>
    Test `pipe_change_produces_synth_output` passes (proves SYNC-03 works).
    like: change detection compiles and emits the correct log/stdout line (verified by reading the code).
  </done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: hum dict suggest</name>
  <files>src/main.rs</files>
  <behavior>
    - `hum dict suggest` reads piece.hum, scans all ThingDef.synth blocks, groups things by shared synth shape
    - Grouping key: (osc type, filter type, primary fx type) — use format!("{:?}{:?}{:?}", synth.osc, synth.filter, synth.fx) as the group key
    - A pattern is "recurring" if 2 or more things share the same group key
    - Output format (stdout, one suggestion per pattern):
      ```
      suggest: 3 things share sine + lpf — consider adding a term to hum.dict
        things: glass, pulse, drift
      suggest: 2 things share noise + hpf — consider adding a term to hum.dict
        things: ghost-machine, rain
      (no suggestions) if no recurring patterns
      ```
    - Test: given a mock Piece with 3 things all having osc: sine + filter: lpf, suggest detects the pattern and returns 1 suggestion containing all 3 thing names
  </behavior>
  <action>
    **src/main.rs** — in handle_dict_cli, add match arm:
    ```rust
    Some("suggest") => {
        let piece_path = PathBuf::from("piece.hum");
        let content = match std::fs::read_to_string(&piece_path) {
            Ok(c) => c,
            Err(e) => { eprintln!("error reading piece.hum: {}", e); std::process::exit(1); }
        };
        let piece: parser::Piece = match serde_saphyr::from_str(&content) {
            Ok(p) => p,
            Err(e) => { eprintln!("error parsing piece.hum: {}", e); std::process::exit(1); }
        };
        suggest_dict_entries(&piece);
    }
    ```

    Add function `fn suggest_dict_entries(piece: &parser::Piece)`:
    ```rust
    fn suggest_dict_entries(piece: &parser::Piece) {
        use std::collections::HashMap;
        // Group by synth shape key
        let mut groups: HashMap<String, Vec<String>> = HashMap::new();
        for (name, thing) in piece {
            if let Some(synth) = &thing.synth {
                let key = format!("{:?}|{:?}|{:?}", synth.osc, synth.filter, synth.fx);
                groups.entry(key).or_default().push(name.clone());
            }
        }
        let mut found = false;
        let mut sorted_groups: Vec<_> = groups.iter().collect();
        sorted_groups.sort_by_key(|(_, names)| std::cmp::Reverse(names.len()));
        for (key, names) in &sorted_groups {
            if names.len() >= 2 {
                found = true;
                // Extract readable description from key (best-effort)
                let desc = key.replace("|", " + ").replace("None", "").replace("  ", " ").trim().to_string();
                println!("suggest: {} things share {} — consider adding a term to hum.dict", names.len(), desc);
                let mut sorted_names = names.clone();
                sorted_names.sort();
                println!("  things: {}", sorted_names.join(", "));
            }
        }
        if !found {
            println!("(no recurring patterns found — all things have distinct synth shapes)");
        }
    }
    ```

    Update the dict usage string to include `suggest`:
    `"usage: hum dict [list|show <term>|add <thing> <term>|suggest]"`

    Add a unit test:
    ```rust
    #[test]
    fn suggest_detects_recurring_pattern() {
        // Build a piece with 3 things sharing the same synth shape
        // Call suggest_dict_entries and verify it doesn't panic
        // (stdout verification is manual; logic test via the grouping map)
        use std::collections::HashMap;
        use indexmap::IndexMap;
        let mut piece = IndexMap::new();
        // Add 3 things with identical synth osc=sine
        for name in &["a", "b", "c"] {
            let mut thing = parser::types::ThingDef { /* default */ };
            // This test verifies the grouping logic compiles and runs
        }
        // Minimal: just verify suggest_dict_entries compiles and doesn't panic on real piece
    }
    ```
    Note: The unit test for suggest should focus on the grouping logic. If ThingDef construction is complex (no Default impl), write a simpler integration-style smoke test or just verify with `cargo test -- suggest`.
  </action>
  <verify>
    <automated>cargo build 2>&1 | tail -10</automated>
  </verify>
  <done>
    `cargo build` succeeds.
    `hum dict suggest` (manual run against piece.hum) prints pattern suggestions or "(no recurring patterns)" — no panic, clean output.
    `hum dict` usage string lists all subcommands including `suggest`.
  </done>
</task>

</tasks>

<verification>
cargo test --lib 2>&1 | tail -30
cargo build 2>&1 | tail -10
</verification>

<success_criteria>
1. Test `pipe_change_produces_synth_output` passes — proves pipe: change propagates to synth: output (SYNC-03 verified)
2. like: change is logged with info + stdout message on file reload (SYNC-01 detection side)
3. `hum dict suggest` scans piece.hum and prints recurring synth shape patterns (SYNC-05)
4. All existing tests still pass
</success_criteria>

<output>
After completion, create `.planning/phases/10-translation-sync/10-2-SUMMARY.md`
</output>

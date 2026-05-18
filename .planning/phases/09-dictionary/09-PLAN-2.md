---
phase: 09-dictionary
plan: 2
type: execute
wave: 2
depends_on: [09-PLAN-1]
files_modified:
  - src/transport.rs
  - src/main.rs
  - hum.dict
autonomous: true
requirements: [DICT-05, DICT-06]

must_haves:
  truths:
    - "hum dict list prints all vocabulary terms, one per line"
    - "hum dict show laser prints the synth mapping and context for laser"
    - "hum dict show nonexistent prints an error message and exits non-zero"
  artifacts:
    - path: "src/transport.rs"
      provides: "DictList and DictShow(term) variants in TransportCmd; DictVocab and DictEntry replies in TransportReply"
    - path: "src/main.rs"
      provides: "dict subcommand routing in run_cli; DictList/DictShow handling in handle_transport"
    - path: "hum.dict"
      provides: "Sample vocabulary file committed to repo for testing"
      contains: "laser:"
  key_links:
    - from: "src/main.rs run_cli"
      to: "src/transport.rs TransportCmd::DictList / DictShow"
      via: "args[0] == dict, args[1] == list|show"
    - from: "src/main.rs handle_transport"
      to: "dict_store.all_terms() / dict_store.get(term)"
      via: "DictList -> all_terms(), DictShow -> get(term)"
---

<objective>
Add `hum dict list` and `hum dict show <term>` CLI commands by extending the transport protocol and wiring them to DictStore.

Purpose: Gives the human and LLM a way to inspect the shared vocabulary from the command line.
Output: Updated transport.rs with dict variants, run_cli routing, handle_transport handler, sample hum.dict file.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/phases/09-dictionary/09-CONTEXT.md
@.planning/phases/09-dictionary/09-1-SUMMARY.md
</context>

<interfaces>
<!-- Existing transport pattern — add dict variants following the same style -->

From src/transport.rs (TransportCmd enum):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum TransportCmd {
    Play,
    Stop,
    Status,
    Seek { pos: f64 },
    PlayFrom { pos: f64 },
    Loop { start: f64, end: f64 },
    Solo { thing: String },
    Mute { thing: String },
    // ADD:
    // DictList,
    // DictShow { term: String },
}
```

From src/transport.rs (TransportReply enum):
```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "ok", rename_all = "snake_case")]
pub enum TransportReply {
    Ack,
    Status { ... },
    Error { message: String },
    // ADD:
    // DictVocab { terms: Vec<String> },
    // DictEntry { term: String, synth: String, context: Option<String> },
}
```

From src/main.rs run_cli (pattern for new subcommand "dict"):
```rust
Some("dict") => {
    match args.get(1).map(|s| s.as_str()) {
        Some("list") => TransportCmd::DictList,
        Some("show") => {
            let term = args.get(2).ok_or_else(|| anyhow::anyhow!("usage: hum dict show <term>"))?;
            TransportCmd::DictShow { term: term.clone() }
        }
        _ => { eprintln!("usage: hum dict [list|show <term>]"); std::process::exit(1); }
    }
}
```

From src/main.rs handle_transport (new match arms):
```rust
TransportCmd::DictList => {
    let terms = dict_store.all_terms().iter().map(|s| s.to_string()).collect();
    TransportReply::DictVocab { terms }
}
TransportCmd::DictShow { term } => {
    match dict_store.get(&term) {
        Some(entry) => TransportReply::DictEntry {
            term: term.clone(),
            synth: format!("{:?}", entry.synth),  // human-readable debug repr
            context: entry.context.clone(),
        },
        None => TransportReply::Error {
            message: format!("term '{}' not found in dictionary", term),
        },
    }
}
```

From src/dict.rs (created in Plan 1):
```rust
pub struct DictStore { ... }
impl DictStore {
    pub fn all_terms(&self) -> Vec<&str>;
    pub fn get(&self, term: &str) -> Option<&DictEntry>;
}
pub struct DictEntry {
    pub synth: SynthBlock,
    pub context: Option<String>,
}
```
</interfaces>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Transport protocol dict variants</name>
  <files>src/transport.rs</files>
  <behavior>
    - Test 1: TransportCmd::DictList serializes to JSON with `"cmd":"dict_list"`
    - Test 2: TransportCmd::DictShow { term: "laser" } serializes to JSON with `"cmd":"dict_show","term":"laser"`
    - Test 3: TransportReply::DictVocab { terms: vec!["laser".into()] } round-trips through JSON serde
    - Test 4: TransportReply::DictEntry { term, synth, context: Some(...) } round-trips through JSON serde
  </behavior>
  <action>
Add to `TransportCmd` enum in src/transport.rs:
```rust
DictList,
DictShow { term: String },
```

Add to `TransportReply` enum in src/transport.rs:
```rust
DictVocab { terms: Vec<String> },
DictEntry { term: String, synth: String, context: Option<String> },
```

The `#[serde(tag = "cmd", rename_all = "snake_case")]` on TransportCmd will serialize DictList as `{"cmd":"dict_list"}` and DictShow as `{"cmd":"dict_show","term":"laser"}` automatically.

Add tests in the existing `#[cfg(test)]` block (or create one) covering the 4 behaviors. Use `serde_json::to_string` / `serde_json::from_str` for round-trip verification.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo test transport:: -- --nocapture 2>&1 | tail -20</automated>
  </verify>
  <done>4 new transport serde tests pass; `cargo check` succeeds</done>
</task>

<task type="auto">
  <name>Task 2: CLI routing + handler wiring + sample hum.dict</name>
  <files>src/main.rs, hum.dict</files>
  <action>
**src/main.rs — run_cli**: Add `"dict"` match arm before the `_` fallback:
```rust
Some("dict") => {
    match args.get(1).map(|s| s.as_str()) {
        Some("list") => TransportCmd::DictList,
        Some("show") => {
            let term = match args.get(2) {
                Some(t) => t.clone(),
                None => {
                    eprintln!("usage: hum dict show <term>");
                    std::process::exit(1);
                }
            };
            TransportCmd::DictShow { term }
        }
        _ => {
            eprintln!("usage: hum dict [list|show <term>]");
            std::process::exit(1);
        }
    }
}
```

Update the `_` fallback usage string to include `dict list` and `dict show <term>`.

**src/main.rs — print_reply**: Add match arms for new reply variants:
```rust
TransportReply::DictVocab { terms } => {
    if terms.is_empty() {
        println!("(dictionary is empty)");
    } else {
        for term in &terms {
            println!("  {}", term);
        }
        println!("{} term(s)", terms.len());
    }
}
TransportReply::DictEntry { term, synth, context } => {
    println!("{}:", term);
    println!("  synth: {}", synth);
    if let Some(ctx) = context {
        println!("  context: {}", ctx);
    }
}
```

**src/main.rs — handle_transport**: Add dict match arms. The function needs `dict_store` as a parameter. Add `dict_store: &DictStore` to its signature and pass it from the event loop:
```rust
TransportCmd::DictList => {
    let terms: Vec<String> = dict_store.all_terms().iter().map(|s| s.to_string()).collect();
    TransportReply::DictVocab { terms }
}
TransportCmd::DictShow { term } => {
    match dict_store.get(&term) {
        Some(entry) => TransportReply::DictEntry {
            term: term.clone(),
            synth: format!("{:?}", entry.synth),
            context: entry.context.clone(),
        },
        None => TransportReply::Error {
            message: format!("term '{}' not found in dictionary", term),
        },
    }
}
```

Update the event loop `DaemonEvent::Transport` arm to pass `&dict_store` to `handle_transport`.

**hum.dict**: Create sample vocabulary file in repo root with entries from v3-DICTIONARY-SYNC.md (laser, warm, haunted-echo, breathing). This file is the canonical test fixture and documents the dict format for users.
```yaml
# hum.dict — shared vocabulary for this project
# Format: term -> synth: block + context description

laser:
  synth:
    osc: "sine"
  context: bright, cutting, sci-fi
  learned-from: glass (2026-03-22)

warm:
  synth:
    filter: "lpf(cutoff: 800)"
  context: soft, intimate, analog
  learned-from: pulse (2026-03-22)

haunted-echo:
  synth:
    fx: "delay(time: 0.5, feedback: 0.7)"
  context: eerie, distant, ghostly
  learned-from: ghost-machine (2026-03-22)

breathing:
  synth:
    env: "adsr(a: 0.5, d: 0.1, s: 0.8, r: 1.0)"
  context: alive, organic, pulsing
  learned-from: pulse (2026-03-22)
```

Note: The dict entries in v3-DICTIONARY-SYNC.md use multi-field synth blocks some of which reference SynthBlock fields not yet supported (mod:, osc-prefer:, fx2:). Use only the fields SynthBlock actually has: notes, osc, filter, env, distort, fx, pan, amp, tempo. The sample hum.dict above is already scoped to supported fields.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo test 2>&1 | tail -20</automated>
  </verify>
  <done>
All existing tests pass. `cargo build` succeeds.
Manual verification (with daemon running):
- `./target/debug/hum-rt dict list` prints laser, warm, haunted-echo, breathing
- `./target/debug/hum-rt dict show laser` prints synth mapping and "bright, cutting, sci-fi" context
- `./target/debug/hum-rt dict show nonexistent` prints error message
  </done>
</task>

</tasks>

<verification>
```bash
cd ~/code/hum && cargo test 2>&1 | tail -20
```

Full Phase 9 verification (human-verifiable, not a checkpoint task):
1. Start daemon: `./target/debug/hum-rt`
2. `./target/debug/hum-rt dict list` → prints 4 terms
3. `./target/debug/hum-rt dict show laser` → prints osc and context
4. Add `style: laser` to a thing in piece.hum → thing plays with laser's osc as base
5. Edit hum.dict and save → daemon logs "dict reloaded"
</verification>

<success_criteria>
- `hum dict list` prints all vocabulary terms
- `hum dict show <term>` prints synth mapping and context
- `hum dict show <missing>` returns an error
- All prior tests still pass (no regressions)
- hum.dict committed to repo as sample vocabulary
</success_criteria>

<output>
After completion, create `.planning/phases/09-dictionary/09-2-SUMMARY.md`
</output>

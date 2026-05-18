---
phase: 06-ref-pipe
plan: 2
type: execute
wave: 1
depends_on: []
files_modified:
  - src/pipe/types.rs
  - src/pipe/parser.rs
  - src/pipe/mod.rs
  - src/parser/types.rs
autonomous: true
requirements: [PIPE-01, PIPE-02, PIPE-03, PIPE-04, PIPE-05, PIPE-06, PIPE-07, PIPE-08, PIPE-09]

must_haves:
  truths:
    - "pipe: block parses to a typed PipeExpr AST with source + Vec<Transform>"
    - "All required transforms parse: replicate, each, shift, spread, tempo, take, repeat"
    - "Pipe source can be a bare thing name or thing.field (e.g. glass.notes)"
    - "Unknown transforms return a parse error, not silent failure"
  artifacts:
    - path: "src/pipe/types.rs"
      provides: "PipeExpr, PipeSource, Transform enum"
      exports: ["PipeExpr", "PipeSource", "Transform"]
    - path: "src/pipe/parser.rs"
      provides: "parse_pipe_block(input: &str) -> Result<PipeExpr>"
      exports: ["parse_pipe_block"]
    - path: "src/parser/types.rs"
      provides: "ThingDef.pipe field: Option<String>"
  key_links:
    - from: "src/parser/types.rs"
      to: "src/pipe/parser.rs"
      via: "ThingDef.pipe: Option<String> fed to parse_pipe_block"
      pattern: "parse_pipe_block"
---

<objective>
Parse the `pipe:` block into a typed AST. This plan is purely parsing — no execution yet (that's Plan 3).

Purpose: Establish the data model and parser so Plan 3 can focus on execution/expansion to SynthBlocks.
Output: src/pipe/ module with PipeExpr types and parse_pipe_block(). ThingDef gains a pipe field.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/v2-PIPE-LANG.md

<interfaces>
<!-- Pipe syntax from v2-PIPE-LANG.md — the grammar to implement -->

Pipe block is a multiline string in YAML:
```yaml
glass-swarm:
  pipe: |
    glass
    |> replicate(3)
    |> each(i => shift(semitones: i * 4))
    |> spread(pan: -0.8~0.8)
```

Source line: bare word (thing name) OR `thing.field` accessor (e.g. `glass.notes`).
Transform lines: `|> transform_name(args)` — one per line, leading whitespace ignored.

Transforms to support (PIPE-02 through PIPE-08):
- replicate(n)                    → Transform::Replicate { n: usize }
- each(i => expr)                 → Transform::Each { expr: String }  (store expr as raw string)
- map(n => expr)                  → Transform::Map { expr: String }
- shift(semitones: N)             → Transform::Shift { semitones: i32 }
- spread(pan: lo~hi)              → Transform::Spread { lo: f32, hi: f32 }
- tempo(Xs/note)                  → Transform::Tempo { seconds_per_note: f32 }
- take(n)                         → Transform::Take { n: usize }
- repeat(n)                       → Transform::Repeat { n: usize }

PipeSource variants:
- Thing("glass")                  → bare thing name
- Field("glass", "notes")         → thing.field accessor

From src/parser/types.rs (ThingDef — needs new field):
```rust
pub struct ThingDef {
    // existing fields ...
    pub pipe: Option<String>,   // raw pipe: block string
}
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: pipe/types.rs — PipeExpr AST types</name>
  <files>src/pipe/types.rs, src/pipe/mod.rs</files>
  <behavior>
    - PipeExpr { source: PipeSource, transforms: Vec<Transform> }
    - PipeSource::Thing(String) for bare thing name
    - PipeSource::Field(String, String) for thing.field
    - Transform::Replicate { n } parses replicate(3)
    - Transform::Shift { semitones } parses shift(semitones: 4)
    - Transform::Spread { lo, hi } parses spread(pan: -0.8~0.8)
    - Transform::Tempo { seconds_per_note } parses tempo(0.35s/note)
    - Transform::Take { n } and Transform::Repeat { n }
    - Transform::Each { expr: String } and Transform::Map { expr: String }
  </behavior>
  <action>
Create `src/pipe/types.rs` with:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum PipeSource {
    Thing(String),
    Field(String, String),  // (thing_name, field_name)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Transform {
    Replicate { n: usize },
    Each { expr: String },
    Map { expr: String },
    Shift { semitones: i32 },
    Spread { lo: f32, hi: f32 },
    Tempo { seconds_per_note: f32 },
    Take { n: usize },
    Repeat { n: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PipeExpr {
    pub source: PipeSource,
    pub transforms: Vec<Transform>,
}
```

Create `src/pipe/mod.rs` with:
```rust
pub mod types;
pub mod parser;
```

Add `pub mod pipe;` to `src/lib.rs` or `src/main.rs` (whichever is the crate root).
  </action>
  <verify>
    <automated>cargo check 2>&1 | grep -E "^error" | head -10</automated>
  </verify>
  <done>cargo check passes, types compile, pub mod pipe declared in crate root.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: pipe/parser.rs — parse_pipe_block</name>
  <files>src/pipe/parser.rs</files>
  <behavior>
    - "glass\n|> replicate(3)\n|> shift(semitones: 4)" parses to PipeExpr { source: Thing("glass"), transforms: [Replicate{3}, Shift{4}] }
    - "glass.notes\n|> take(4)" parses to PipeExpr { source: Field("glass","notes"), transforms: [Take{4}] }
    - "spread(pan: -0.8~0.8)" parses Spread { lo: -0.8, hi: 0.8 }
    - "tempo(0.35s/note)" parses Tempo { seconds_per_note: 0.35 }
    - "each(i => shift(semitones: i * 4))" parses Each { expr: "shift(semitones: i * 4)".to_string() }
    - Unknown transform name returns Err
    - Missing source line (only |> lines) returns Err
  </behavior>
  <action>
Create `src/pipe/parser.rs` with `pub fn parse_pipe_block(input: &str) -> anyhow::Result<PipeExpr>`.

**Parsing strategy:**
1. Split input into lines, trim each line, skip empty lines.
2. First non-empty, non-`|>` line is the source. Parse as `thing.field` (split on `.`) or bare word.
3. Remaining lines must start with `|>`. Strip `|>` prefix, trim, parse as transform.

**Transform parsing:**
- Use the same `parse_primitive_call(s)` pattern already in `src/ir/types.rs` (copy the helper or import — prefer copy to avoid coupling).
- `replicate(3)` → strip parens, parse first arg as usize.
- `each(i => ...)` → everything after `=>` is the expr string (trimmed).
- `map(n => ...)` → same as each.
- `shift(semitones: N)` → parse semitones param as i32.
- `spread(pan: lo~hi)` → parse pan param as range using `~` split.
- `tempo(Xs/note)` → parse first arg, strip `s/note` suffix, parse as f32.
- `take(N)` / `repeat(N)` → parse single positional usize arg.
- Unknown name: `bail!("unknown pipe transform: '{}'", name)`.

Add unit tests in the same file covering all the behavior cases above.
  </action>
  <verify>
    <automated>cargo test -p hum-rt --lib pipe::parser -- --nocapture 2>&1 | tail -20</automated>
  </verify>
  <done>All parser unit tests pass. parse_pipe_block exported from src/pipe/parser.rs.</done>
</task>

<task type="auto">
  <name>Task 3: Add pipe: field to ThingDef</name>
  <files>src/parser/types.rs</files>
  <action>
Add `pub pipe: Option<String>` to the `ThingDef` struct in `src/parser/types.rs`. Place it after the `synth` field in the struct body.

The field stores the raw multiline pipe block string. Execution/expansion happens in Plan 3.

Since ThingDef uses `#[serde(deny_unknown_fields)]`, the field must be added to the struct to be accepted during YAML parsing. No other changes needed.
  </action>
  <verify>
    <automated>cargo test -p hum-rt --lib parser -- --nocapture 2>&1 | tail -10</automated>
  </verify>
  <done>Existing parser tests still pass. A YAML thing with `pipe: |` followed by pipe lines deserializes without error.</done>
</task>

</tasks>

<verification>
cargo test -p hum-rt --lib 2>&1 | tail -10
</verification>

<success_criteria>
1. PipeExpr, PipeSource, Transform types compile and are pub-exported from src/pipe/.
2. parse_pipe_block handles all 8 transforms + both source types with unit tests.
3. ThingDef accepts a pipe: field in YAML without parse error.
4. All existing tests still pass.
</success_criteria>

<output>
After completion, create `.planning/phases/06-ref-pipe/06-2-SUMMARY.md`
</output>

---
phase: 02-parser-scd-reader
plan: 1
type: execute
wave: 1
depends_on: []
files_modified:
  - Cargo.toml
  - src/parser/mod.rs
  - src/parser/types.rs
  - src/parser/error.rs
  - src/main.rs
autonomous: true
requirements: [PARSE-01, PARSE-02, PARSE-03, PARSE-04]

must_haves:
  truths:
    - "A valid .hum file with all supported fields parses without error and returns an IndexMap of ThingDef values"
    - "A .hum file with an unknown field is rejected with an error message containing line number and field name"
    - "Runtime-actionable fields (at, until, does, where) are structurally separated from LLM-facing fields (like, ref, mood) in the type definition"
    - "The does: field accepts both a single string and a list of strings"
  artifacts:
    - path: "src/parser/mod.rs"
      provides: "pub fn parse_hum(content: &str) -> Result<Piece, HumParseError>"
      exports: ["parse_hum", "Piece"]
    - path: "src/parser/types.rs"
      provides: "Piece, ThingDef, DoesField type definitions"
      contains: "deny_unknown_fields"
    - path: "src/parser/error.rs"
      provides: "HumParseError with context"
      exports: ["HumParseError"]
  key_links:
    - from: "src/parser/mod.rs"
      to: "serde_saphyr::from_str"
      via: "serde-saphyr crate"
      pattern: "serde_saphyr::from_str"
    - from: "src/parser/types.rs"
      to: "ThingDef"
      via: "#[serde(deny_unknown_fields)]"
      pattern: "deny_unknown_fields"
---

<objective>
Implement the .hum file parser: strict YAML schema with deny_unknown_fields, all supported fields, runtime/LLM field separation, and structured errors with line numbers.

Purpose: Downstream phases (state reconciler, timeline) need a typed Piece value from piece.hum. This plan delivers that contract.
Output: `src/parser/` module with parse_hum(), ThingDef struct, DoesField enum, HumParseError type.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/phases/02-parser-scd-reader/02-CONTEXT.md
@.planning/phases/02-parser-scd-reader/research/RESEARCH.md

<interfaces>
<!-- Existing API from Phase 1 that this plan extends -->

From src/main.rs (current):
```rust
mod config;
mod osc;
// Plan 1 adds: mod parser;
```

From Cargo.toml (current deps — plan adds serde-saphyr + indexmap):
```toml
serde = { version = "1", features = ["derive"] }
thiserror = "2.0"
anyhow = "1.0"
tracing = "0.1"
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Define parser types and error</name>
  <files>src/parser/types.rs, src/parser/error.rs, Cargo.toml</files>
  <behavior>
    - parse_hum("space-crackle:\n  at: \"0s\"\n  like: warm pad") returns Ok(piece) where piece["space-crackle"].at == Some("0s")
    - parse_hum("space-crackle:\n  unknown_field: val") returns Err containing "unknown field"
    - parse_hum("bass:\n  does: builds from silence") returns Ok where does is DoesField::Single
    - parse_hum("bass:\n  does:\n    - builds from silence\n    - fades out") returns Ok where does is DoesField::Multi with 2 items
    - parse_hum("bass:\n  ref: some-ref") returns Ok where thing.reference == Some("some-ref") (ref is a Rust keyword, rename to reference)
    - parse_hum("bass:\n  where: center") returns Ok where thing.location == Some("center") (where is a Rust keyword, rename to location)
  </behavior>
  <action>
Add to Cargo.toml under [dependencies]:
```
serde-saphyr = "0.0.22"
indexmap = { version = "2", features = ["serde"] }
```

Create src/parser/error.rs:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HumParseError {
    #[error("parse error in .hum file: {0}")]
    InvalidSchema(String),

    #[error("IO error reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}
```

Create src/parser/types.rs with:
- `pub type Piece = IndexMap<String, ThingDef>;`
- `ThingDef` struct with `#[serde(deny_unknown_fields)]`
- All fields as `Option<T>`:
  - Runtime fields: `at`, `until`, `does`, `location` (renamed from `where`), `has`, `within`, `every`
  - LLM fields: `like`, `reference` (renamed from `ref`), `mood`
- `where` → field named `location` with `#[serde(rename = "where")]`
- `ref` → field named `reference` with `#[serde(rename = "ref")]`
- `has: Option<IndexMap<String, ThingDef>>` for recursive sub-things
- `DoesField` untagged enum: `Single(String)` | `Multi(Vec<String>)` with `as_vec()` helper

DO NOT use `#[serde(flatten)]` anywhere on ThingDef — incompatible with `deny_unknown_fields`.
DO NOT use `serde_yml` — has RustSec advisory RUSTSEC-2025-0068.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo test parser -- --nocapture 2>&1 | tail -20</automated>
  </verify>
  <done>All 6 behavior tests pass. cargo test parser green. deny_unknown_fields rejects unknown fields at parse time.</done>
</task>

<task type="auto">
  <name>Task 2: Implement parse_hum and wire into main.rs</name>
  <files>src/parser/mod.rs, src/main.rs</files>
  <action>
Create src/parser/mod.rs:
```rust
mod error;
mod types;

pub use error::HumParseError;
pub use types::{DoesField, Piece, ThingDef};

pub fn parse_hum(content: &str) -> Result<Piece, HumParseError> {
    serde_saphyr::from_str(content).map_err(|e| HumParseError::InvalidSchema(e.to_string()))
}
```

Update src/main.rs to add `mod parser;` and a parse validation at startup. After Config::load(), add:
```rust
// Validate piece.hum if present at ./piece.hum
if let Ok(content) = std::fs::read_to_string("piece.hum") {
    match parser::parse_hum(&content) {
        Ok(piece) => {
            tracing::info!("parsed {} things from piece.hum", piece.len());
            println!("hum-rt: parsed {} things from piece.hum", piece.len());
        }
        Err(e) => {
            eprintln!("error parsing piece.hum: {e}");
            // Non-fatal at startup — piece.hum may not exist yet
        }
    }
}
```

Also add `mod parser;` to the module declarations at the top of main.rs.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build 2>&1 | tail -20</automated>
  </verify>
  <done>cargo build succeeds with no errors or warnings. parse_hum is pub-exported from src/parser/mod.rs. main.rs compiles with mod parser declared.</done>
</task>

</tasks>

<verification>
- `cargo test` passes
- `cargo build` produces no errors
- Parsing a valid .hum file (all fields present) returns Ok
- Parsing a .hum file with an unknown field returns Err with "unknown field" in message
- The `does:` field works as both String and Vec<String>
- `ref` and `where` fields deserialize correctly despite being Rust keywords
</verification>

<success_criteria>
1. cargo test parser — all tests green
2. cargo build — clean compile
3. ThingDef has all 10 fields: at, until, does, location(where), has, within, every, like, reference(ref), mood
4. deny_unknown_fields on ThingDef rejects unknown YAML keys
5. parse_hum returns HumParseError::InvalidSchema with serde-saphyr's line/column info in the message
</success_criteria>

<output>
After completion, create `.planning/phases/02-parser-scd-reader/02-1-SUMMARY.md`
</output>

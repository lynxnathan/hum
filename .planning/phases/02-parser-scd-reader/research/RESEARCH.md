# Phase 2: Parser + SCD Reader - Research

**Researched:** 2026-03-20
**Domain:** Rust YAML parsing (serde), data modeling for dynamic-key YAML, binary file reading (.scsyndef)
**Confidence:** HIGH (stack decisions verified via crates.io, RustSec, official SC docs)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None — all Phase 2 decisions are Claude's discretion.

### Claude's Discretion
- YAML crate choice — serde_yaml is deprecated. Research identified yaml_serde 0.10 or serde-saphyr as alternatives. Pick whichever compiles cleanly with deny_unknown_fields.
- Data model design — How to structure the Piece/Thing types. Runtime-actionable fields (at, until, does, where) vs LLM-facing (like, ref, mood).
- Error formatting — How to present parse errors with line/field/suggestion. Level of detail.
- SCD association strategy — How to map .scd/.scsyndef files to thing names (filename convention vs manifest).
- has/within nesting — How deep nesting goes, how sub-things are represented in the data model.

### Deferred Ideas (OUT OF SCOPE)
None
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PARSE-01 | Parse any valid .hum file with strict schema (deny_unknown_fields) | Use `serde-saphyr` + `HashMap<String, ThingDef>` top-level; `deny_unknown_fields` on `ThingDef` inner struct |
| PARSE-02 | All .hum fields: at, until, does, where, has, within, every, mood, like, ref | `ThingDef` struct with `Option<T>` fields; `does` as `DoesField` untagged enum (String or Vec<String>) |
| PARSE-03 | Runtime vs LLM-facing field separation | Split into `RuntimeFields` and `LlmFields` marker groups in data model; or simply include all, label in comments |
| PARSE-04 | Helpful parse errors with line, field, suggestion | `serde-saphyr` provides line/column in errors; wrap with `thiserror` for structured ParseError type |
| SCD-01 | Read .scd files from out/sc/ | `std::fs::read()` — .scsyndef is opaque binary; no parsing needed, just bytes |
| SCD-02 | Associate .scd with thing names | Filename convention: `{thing-name}.scd` in out/sc/; thing name = stem. Verified pattern from project.md. |
| SCD-03 | Load SynthDefs into scsynth on startup | Pass raw bytes to `ScsynthClient::load_synthdef(bytes)` already implemented in Phase 1 |
</phase_requirements>

---

## Summary

Phase 2 requires solving two distinct problems: (1) parsing a YAML file with dynamic top-level keys into a strict typed Rust structure, and (2) reading binary .scsyndef files from disk and associating them with thing names. Both problems are well-solved with existing tools.

The YAML crate situation requires care: `serde_yaml` is deprecated (March 2024), `serde_yml` has a RustSec soundness advisory (RUSTSEC-2025-0068, 2025), and `yaml/yaml-serde` (0.10) is the official YAML org fork. `serde-saphyr` (0.0.22) is an independent alternative built on the saphyr parser that deserializes directly to Rust types without an intermediate tree — it has 1000+ passing tests including the full yaml-test-suite. Both `yaml/yaml-serde` and `serde-saphyr` are viable; the recommendation is `serde-saphyr` for its direct-to-type approach and active test coverage.

The .hum format's dynamic top-level keys (thing names) require deserializing the top level as `HashMap<String, ThingDef>` — `deny_unknown_fields` applies to the `ThingDef` inner struct, not the map. The `does:` field can be a string or list of strings in valid .hum files, which requires an untagged enum. The .scsyndef association is trivially solved by the filename convention already established in the project.

**Primary recommendation:** Use `serde-saphyr` for YAML parsing. Model the piece as `type Piece = IndexMap<String, ThingDef>`. Apply `#[serde(deny_unknown_fields)]` to `ThingDef`. Handle `does:` as an untagged enum. Load .scsyndef as raw bytes keyed by filename stem.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde-saphyr | 0.0.22 | YAML deserialization | Actively maintained, direct-to-type (no intermediate Value tree), 1000+ test cases, panic-free, good error reporting. Supersedes deprecated serde_yaml. |
| serde | 1.0 | Derive macros (Deserialize) | Required companion; `#[derive(Deserialize)]` + `deny_unknown_fields` is the standard schema enforcement approach |
| indexmap | 2.x | `IndexMap<String, ThingDef>` | Preserves insertion order for the top-level thing map — important for predictable playback ordering. Drop-in for HashMap. |
| thiserror | 2.0 | ParseError type | Already in Cargo.toml; wrap serde-saphyr errors into a typed `HumParseError` with context |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::fs | stdlib | Read .scsyndef bytes | `fs::read(path)` returns `Vec<u8>` — exactly what `ScsynthClient::load_synthdef()` expects |
| std::path | stdlib | Filename stem extraction | `path.file_stem()` extracts thing name from `out/sc/space-crackle.scd` |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| serde-saphyr | yaml/yaml-serde 0.10 | yaml/yaml-serde is the official YAML org fork of serde_yaml with drop-in API compatibility. Either works. serde-saphyr has a cleaner architecture (no intermediate tree). yaml/yaml-serde is a safer migration path if API compatibility matters. |
| serde-saphyr | serde_yml | DO NOT USE — RustSec advisory RUSTSEC-2025-0068 marks serde_yml as unsound and unmaintained (2025). |
| indexmap | std::HashMap | HashMap works if ordering doesn't matter. IndexMap preserves declaration order from the .hum file, which is useful for diagnostic output and predictable test assertions. |

**Installation additions to Cargo.toml:**
```toml
serde-saphyr = "0.0.22"
indexmap = { version = "2", features = ["serde"] }
```

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── parser/
│   ├── mod.rs          # pub fn parse_hum(content: &str) -> Result<Piece, HumParseError>
│   ├── types.rs        # Piece, ThingDef, DoesField, WhereField
│   └── error.rs        # HumParseError with context
├── scd/
│   ├── mod.rs          # pub fn load_scd_dir(path: &Path) -> Result<ScdStore, ScdError>
│   └── store.rs        # ScdStore: map of thing_name -> Vec<u8>
├── config.rs           # (existing, Phase 1)
├── osc/                # (existing, Phase 1)
└── main.rs
```

### Pattern 1: Top-level dynamic keys with strict inner schema

The .hum file has dynamic top-level keys (thing names chosen by users). `deny_unknown_fields` cannot apply to the map itself — it applies to the value type.

```rust
// src/parser/types.rs
use indexmap::IndexMap;
use serde::Deserialize;

/// A parsed .hum file. Keys are thing names (e.g. "space-crackle").
pub type Piece = IndexMap<String, ThingDef>;

/// One named thing in a piece. All fields optional — absent = "not decided".
/// deny_unknown_fields enforces the schema: any unrecognized field is a parse error.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThingDef {
    // Runtime-actionable fields
    pub at: Option<String>,
    pub until: Option<String>,
    pub does: Option<DoesField>,
    pub r#where: Option<String>,   // `where` is a Rust keyword — use r#where or rename
    pub has: Option<IndexMap<String, ThingDef>>,  // nested sub-things
    pub within: Option<String>,
    pub every: Option<String>,

    // LLM-facing fields (parsed but not runtime-actionable in Phase 2)
    pub like: Option<String>,
    pub r#ref: Option<String>,
    pub mood: Option<String>,
}
```

**Note on `where`:** `where` is a Rust keyword. Use `r#where` in the field name with `#[serde(rename = "where")]` OR use `#[serde(rename_all = "snake_case")]` plus `r#where`.

```rust
// Cleaner approach: explicit rename
#[serde(rename = "where")]
pub location: Option<String>,
```

### Pattern 2: `does:` as untagged enum (string or list)

The `does:` field in .hum can be:
- A single string: `does: builds from silence, never resolves`
- A list of strings: `does:\n  - volume from very low to moderate over 5s\n  - wah starts slow`

The standard serde pattern for this is an untagged enum:

```rust
// src/parser/types.rs
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum DoesField {
    Single(String),
    Multi(Vec<String>),
}

impl DoesField {
    pub fn as_vec(&self) -> Vec<&str> {
        match self {
            DoesField::Single(s) => vec![s.as_str()],
            DoesField::Multi(v) => v.iter().map(|s| s.as_str()).collect(),
        }
    }
}
```

### Pattern 3: Parse function with structured errors

```rust
// src/parser/mod.rs
use serde_saphyr; // or: use serde_yaml as serde_saphyr (if using yaml/yaml-serde alias)

pub fn parse_hum(content: &str) -> Result<Piece, HumParseError> {
    serde_saphyr::from_str(content).map_err(|e| HumParseError::InvalidSchema {
        message: e.to_string(),
        // serde-saphyr errors include line/column information in Display
    })
}
```

```rust
// src/parser/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HumParseError {
    #[error("Invalid .hum schema at {message}")]
    InvalidSchema { message: String },

    #[error("IO error reading {path}: {source}")]
    Io { path: String, #[source] source: std::io::Error },
}
```

### Pattern 4: SCD store — filename-to-bytes map

```rust
// src/scd/store.rs
use std::collections::HashMap;
use std::path::Path;

/// Maps thing names to their compiled .scsyndef bytes.
pub struct ScdStore {
    pub defs: HashMap<String, Vec<u8>>,
}

impl ScdStore {
    pub fn load_dir(dir: &Path) -> Result<Self, std::io::Error> {
        let mut defs = HashMap::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("scd") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let bytes = std::fs::read(&path)?;
                    defs.insert(stem.to_string(), bytes);
                }
            }
        }
        Ok(ScdStore { defs })
    }

    pub fn get(&self, thing_name: &str) -> Option<&[u8]> {
        self.defs.get(thing_name).map(|v| v.as_slice())
    }
}
```

### Anti-Patterns to Avoid

- **Parsing the entire Piece as a typed struct with named fields:** The top-level keys are dynamic (user-chosen thing names). You cannot model `space-crackle` as a struct field. Use `IndexMap<String, ThingDef>`.
- **Applying `deny_unknown_fields` to the Piece map type:** `deny_unknown_fields` only works on structs, not on HashMap/IndexMap. Apply it to `ThingDef` only.
- **Using `serde_yml`:** Has RustSec advisory RUSTSEC-2025-0068 (unsound + unmaintained, 2025). Do not use.
- **Using `#[serde(flatten)]` with `deny_unknown_fields`:** These are incompatible in serde — flatten + deny_unknown_fields on the same struct causes a compile/runtime conflict. Do not combine them.
- **Parsing .scsyndef binary format:** The daemon does not need to understand the binary format. It loads the file as raw bytes and passes them to scsynth via `d_recv`. scsynth handles interpretation.
- **Using `where` as a Rust field name without escaping:** `where` is a Rust keyword. Either use `r#where` with an explicit `#[serde(rename = "where")]` attribute, or rename the field to `location` with the rename attribute.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| YAML deserialization | Custom YAML parser | serde-saphyr | YAML has 50+ edge cases (anchors, aliases, multiline strings, type coercion). Hand-rolled parsers fail on valid YAML. |
| Schema validation | Field-by-field checks after parsing into Value | `deny_unknown_fields` on `ThingDef` | serde handles validation at deserialize time — no post-parse validation code needed |
| Line number in errors | Manual line counting | serde-saphyr's error Display | serde-saphyr includes line/column in error output automatically |
| Untagged union field (does: string or list) | Custom deserialization | `#[serde(untagged)]` enum | serde's untagged enum tries variants in order — exactly right for string-or-vec |
| SynthDef name extraction | Parse .scsyndef binary header | Filename stem convention | The filename IS the thing name by project convention. No binary parsing needed. |

**Key insight:** The .scsyndef binary format has a magic header `SCgf`, version int32, and stores SynthDef names internally. But hum-rt does NOT need to read SynthDef names from the binary — the project convention is that `out/sc/{thing-name}.scd` encodes the thing name in the filename. Trust the filename.

---

## Common Pitfalls

### Pitfall 1: serde_yml RustSec advisory

**What goes wrong:** Project adds `serde_yml` as the serde_yaml replacement, compiles fine, but `cargo audit` flags RUSTSEC-2025-0068 (unsound, unmaintained).
**Why it happens:** serde_yml was a popular fork but its author stopped maintaining it in 2025.
**How to avoid:** Use `serde-saphyr` or `yaml/yaml-serde` (yaml org fork). Never `serde_yml`.
**Warning signs:** `cargo audit` output mentions RUSTSEC-2025-0068.

### Pitfall 2: `where` is a Rust keyword

**What goes wrong:** Writing `pub where: Option<String>` causes a compile error.
**Why it happens:** `where` is a reserved keyword in Rust used in generic bounds.
**How to avoid:** Rename the struct field to `location` and add `#[serde(rename = "where")]`, OR use `r#where` raw identifier syntax.
**Warning signs:** `error: expected identifier, found keyword 'where'`

### Pitfall 3: deny_unknown_fields + flatten incompatibility

**What goes wrong:** Trying to use `#[serde(flatten)]` on a sub-field of a struct that has `#[serde(deny_unknown_fields)]` — causes a confusing runtime error or silently fails.
**Why it happens:** serde's `deny_unknown_fields` and `flatten` have conflicting internal mechanics.
**How to avoid:** Never combine them. The `has:` sub-things field should be `Option<IndexMap<String, ThingDef>>` — not a flattened field.
**Warning signs:** Test passes on simple input but fails on input with `has:` sub-things.

### Pitfall 4: `does:` field as String when it's actually a list

**What goes wrong:** Declaring `does: Option<String>` — parse fails with a confusing error when a .hum file uses the list form (`does:\n  - trajectory 1`).
**Why it happens:** YAML lists and scalars are different types. serde won't coerce a sequence to a String.
**How to avoid:** Always use the `DoesField` untagged enum. Never `Option<String>` for `does:`.
**Warning signs:** `Error: invalid type: sequence, expected a string` during parse.

### Pitfall 5: Missing `scd` files treated as hard errors

**What goes wrong:** `ScdStore::load_dir()` panics or returns `Err` when a thing in the .hum has no matching .scd file.
**Why it happens:** Treating missing .scd as a fatal error breaks the authoring workflow — a user may add a thing to the .hum before the LLM has compiled it.
**How to avoid:** Missing .scd is a warning, not an error. `ScdStore::get()` returns `Option<&[u8]>`. Log a warning but continue. Only fail on startup if zero .scd files exist.
**Warning signs:** `hum play` errors out when piece.hum references a thing with no .scd yet.

---

## Code Examples

### Full parse example

```rust
// Source: serde-saphyr API (github.com/bourumir-wyngs/serde-saphyr)
use serde::Deserialize;
use indexmap::IndexMap;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThingDef {
    pub at: Option<String>,
    pub until: Option<String>,
    pub does: Option<DoesField>,
    #[serde(rename = "where")]
    pub location: Option<String>,
    pub has: Option<IndexMap<String, ThingDef>>,
    pub within: Option<String>,
    pub every: Option<String>,
    pub like: Option<String>,
    #[serde(rename = "ref")]
    pub reference: Option<String>,
    pub mood: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DoesField {
    Single(String),
    Multi(Vec<String>),
}

type Piece = IndexMap<String, ThingDef>;

fn parse_hum(content: &str) -> Result<Piece, serde_saphyr::Error> {
    serde_saphyr::from_str(content)
}
```

### SCD directory scan

```rust
// Source: std::fs, std::path — stdlib
use std::collections::HashMap;
use std::path::Path;

fn load_scd_dir(dir: &Path) -> std::io::Result<HashMap<String, Vec<u8>>> {
    let mut store = HashMap::new();
    if !dir.exists() {
        return Ok(store); // out/sc/ may not exist yet — that's ok
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("scd") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                store.insert(stem.to_string(), std::fs::read(&path)?);
            }
        }
    }
    Ok(store)
}
```

### Associating SCD with parsed things

```rust
// Source: derived from project.md conventions
fn find_missing_scds(piece: &Piece, scd_store: &HashMap<String, Vec<u8>>) -> Vec<String> {
    piece.keys()
        .filter(|name| !scd_store.contains_key(*name))
        .cloned()
        .collect()
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| serde_yaml 0.9 | serde-saphyr 0.0.22 or yaml/yaml-serde 0.10 | March 2024 (deprecation), 2025 (alternatives matured) | Must migrate; serde_yaml is +deprecated on crates.io |
| serde_yml fork | Avoid entirely | 2025 | RustSec RUSTSEC-2025-0068: unsound + unmaintained |

**Deprecated/outdated:**
- `serde_yaml`: Marked +deprecated by dtolnay (March 2024). No future fixes.
- `serde_yml`: RustSec advisory 2025. Do not use.

---

## Open Questions

1. **serde-saphyr API surface for deny_unknown_fields**
   - What we know: serde-saphyr implements the serde Deserializer trait; `deny_unknown_fields` is a serde derive attribute, not a YAML-crate attribute — it should work with any serde-compatible backend.
   - What's unclear: Whether serde-saphyr has any known incompatibilities with `deny_unknown_fields` on nested structs (the `has:` recursive case).
   - Recommendation: Add a test early in Wave 0 that parses a .hum file with an unknown field and asserts the parse returns an error. This validates the deny_unknown_fields behavior against the chosen crate before building on top of it.

2. **`ref` field name — Rust keyword**
   - What we know: `ref` is a Rust keyword (used in pattern matching). Similar to `where`.
   - What's unclear: Whether `r#ref` compiles correctly in all Rust editions or if rename is cleaner.
   - Recommendation: Use `#[serde(rename = "ref")] pub reference: Option<String>` — explicit rename is clearer than raw identifier syntax.

3. **Recursive ThingDef for `has:` sub-things**
   - What we know: The `has:` field contains nested thing definitions with the same schema.
   - What's unclear: Whether serde-saphyr handles recursive struct deserialization via `Box<ThingDef>` or `Option<IndexMap<String, ThingDef>>`.
   - Recommendation: Use `Option<IndexMap<String, ThingDef>>`. Recursive types in serde work when the recursion is behind a heap allocation (HashMap/IndexMap values are heap-allocated). No `Box` needed in this case.

---

## Sources

### Primary (HIGH confidence)
- [RustSec RUSTSEC-2025-0068](https://rustsec.org/advisories/RUSTSEC-2025-0068.html) — serde_yml unsound and unmaintained (2025)
- [serde_yaml crates.io — +deprecated](https://crates.io/crates/serde_yaml) — deprecation by dtolnay confirmed
- [SuperCollider Synth Definition File Format](https://doc.sccode.org/Reference/Synth-Definition-File-Format.html) — SCgf magic bytes, big-endian, format versions 1 and 2
- [serde.rs — Struct attributes deny_unknown_fields](https://serde.rs/field-attrs.html) — behavior confirmed
- [serde.rs — Enum representations (untagged)](https://serde.rs/enum-representations.html) — untagged enum pattern

### Secondary (MEDIUM confidence)
- [serde-saphyr on lib.rs](https://lib.rs/crates/serde-saphyr) — version 0.0.22, 1000+ tests, saphyr-based
- [GitHub bourumir-wyngs/serde-saphyr](https://github.com/bourumir-wyngs/serde-saphyr) — actively maintained, panic-free, direct-to-type
- [GitHub yaml/yaml-serde](https://github.com/yaml/yaml-serde) — official YAML org fork, v0.10, drop-in API
- [rust.code-maven.com — YAML deny unknown fields](https://rust.code-maven.com/yaml/yaml-deny-unknown-fields) — working example pattern

### Tertiary (LOW confidence)
- WebSearch cross-references on serde flatten + deny_unknown_fields incompatibility — consistent across multiple GitHub issues, treated as confirmed serde behavior

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — RustSec advisory verified, serde-saphyr version confirmed on crates.io/lib.rs
- Architecture: HIGH — serde patterns (untagged enum, deny_unknown_fields on inner struct) are well-established; .scsyndef loading confirmed from Phase 1
- Pitfalls: HIGH — `where`/`ref` keyword conflicts are compile-time facts; RustSec advisory is authoritative; flatten+deny_unknown_fields incompatibility documented in multiple serde issues

**Research date:** 2026-03-20
**Valid until:** 2026-06-20 (serde-saphyr is in 0.0.x, watch for API changes)

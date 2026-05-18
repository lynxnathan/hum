---
phase: 10-translation-sync
plan: 1
type: execute
wave: 1
depends_on: []
files_modified:
  - src/dict.rs
  - src/transport.rs
  - src/main.rs
autonomous: true
requirements:
  - SYNC-02
  - SYNC-04

must_haves:
  truths:
    - "Saving a manually edited synth: block writes a divergence comment into the .hum file"
    - "`hum dict add <thing> <term>` writes a new entry to hum.dict with the thing's current resolved synth block"
    - "The written dict entry includes learned-from: <thing> metadata"
  artifacts:
    - path: "src/dict.rs"
      provides: "DictStore::add_entry() serializes a SynthBlock + metadata to hum.dict"
    - path: "src/transport.rs"
      provides: "TransportCmd::DictAdd, TransportReply::DictAdded variants"
    - path: "src/main.rs"
      provides: "Divergence detection in handle_file_change + dict add CLI routing"
  key_links:
    - from: "src/main.rs handle_file_change"
      to: "piece.hum"
      via: "synth_hash comparison between old and new ThingDef"
      pattern: "synth.*hash|diverge"
    - from: "src/main.rs handle_dict_cli"
      to: "src/dict.rs DictStore::add_entry"
      via: "resolve current synth block from state, call add_entry"
      pattern: "DictAdd|add_entry"
---

<objective>
Implement divergence detection (SYNC-02) and `hum dict add` (SYNC-04).

Purpose: When a user hand-tunes synth: parameters that diverge from pipe: output, hum-rt marks the file so the LLM knows. When a user approves a sound, they capture it into the dictionary with one command.
Output: Divergence comment written to .hum file on manual synth: edit; `hum dict add <thing> <term>` appends to hum.dict.
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
    // ...all fields
}
pub type Piece = IndexMap<String, ThingDef>;
```

From src/dict.rs:
```rust
pub struct DictEntry {
    pub synth: SynthBlock,
    pub context: Option<String>,
    pub learned_from: Option<String>,   // serde rename = "learned-from"
}
pub struct DictStore {
    entries: HashMap<String, DictEntry>,
}
impl DictStore {
    pub fn load(path: &Path) -> anyhow::Result<Self>;
    pub fn get(&self, term: &str) -> Option<&DictEntry>;
    pub fn all_terms(&self) -> Vec<&str>;
    pub fn merge_with_global(global: DictStore, project: DictStore) -> DictStore;
}
```

From src/transport.rs:
```rust
pub enum TransportCmd {
    DictList,
    DictShow { term: String },
    // add: DictAdd { thing: String, term: String }
}
pub enum TransportReply {
    DictVocab { terms: Vec<String> },
    DictEntry { term: String, synth: String, context: Option<String> },
    // add: DictAdded { term: String }
}
```

From src/main.rs:
```rust
// CLI dispatch: "dict" branch calls handle_dict_cli(&args[1..])
// Daemon event loop: DaemonEvent::Transport(cmd, reply_tx) -> handle_transport(...)
// handle_file_change receives &mut dict_store, &mut state, etc.
// StateStore holds current Piece (parsed ThingDefs)
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Divergence detection in handle_file_change</name>
  <files>src/main.rs</files>
  <behavior>
    - When piece.hum reloads, for each thing: compare old ThingDef.synth hash vs new ThingDef.synth hash
    - If synth: changed AND pipe: is also present AND the new synth does NOT equal what expand_pipe would produce → it is a manual edit
    - Write comment line `# synth: manually tuned, pipe: may be stale` directly above the `synth:` key of that thing in the .hum file using std::fs::read_to_string + string manipulation + std::fs::write
    - If the comment already exists for that thing, do not duplicate it
    - Hash strategy: use a quick format!("{:?}", synth_block) string hash (no external crate needed); store as HashMap<String, String> (thing_name -> synth_repr) in the event loop local state
    - Test: given old_synth != new_synth and pipe: present and new_synth != pipe-expanded output, comment is inserted; if comment already present, no duplicate
  </behavior>
  <action>
    In the event loop's local state (near the top of the daemon setup in main()), add:
    `let mut synth_hashes: HashMap<String, String> = HashMap::new();`

    In handle_file_change (or inline in the DaemonEvent::FileChanged arm):
    1. After re-parsing the piece, iterate new_piece things
    2. For each thing with both pipe: and synth: present:
       a. Get new_synth_repr = format!("{:?}", thing.synth)
       b. Get old_repr = synth_hashes.get(thing_name).cloned().unwrap_or_default()
       c. If new_repr != old_repr AND old_repr is non-empty (meaning synth changed since last load):
          - Call expand_pipe on the pipe: string to get the pipe-expanded SynthBlock
          - Compare format!("{:?}", pipe_synth) vs new_repr
          - If they differ → manual divergence detected
          - Insert comment into the .hum file: read file, find the line matching `  synth:` under the correct thing key, prepend `  # synth: manually tuned, pipe: may be stale\n` if not already present
       d. Update synth_hashes.insert(thing_name.clone(), new_repr)

    Note: File comment insertion must be scoped to the correct thing block. Use a line-scan approach: track current thing key (YAML top-level key = line not starting with spaces and ending with `:`), insert comment only in the matching thing's block.

    Use `parse_pipe_block` + `expand_pipe` (already imported in main.rs) for pipe expansion comparison.
  </action>
  <verify>
    <automated>cargo test --lib -- diverge 2>&1 | tail -20</automated>
  </verify>
  <done>Unit test passes: given a piece where synth: was manually changed from pipe output, comment is inserted into the file string. No duplicate on second pass.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: DictStore::add_entry + hum dict add CLI</name>
  <files>src/dict.rs, src/transport.rs, src/main.rs</files>
  <behavior>
    - DictStore::add_entry(path, term, synth_block, learned_from) appends a new YAML entry to hum.dict
    - If term already exists, it is overwritten (user is explicitly re-capturing)
    - Serialization: use serde_saphyr or manual YAML string construction matching the existing hum.dict format
    - CLI: `hum dict add <thing> <term>` reads the current resolved synth block for <thing> from piece.hum (via DictStore + parser, client-side, no daemon), writes to hum.dict
    - `hum dict add` without enough args prints usage and exits 1
    - Test: add_entry writes valid YAML; re-adding same term overwrites; missing args returns error
  </behavior>
  <action>
    **src/dict.rs** — add method:
    ```rust
    pub fn add_entry(
        dict_path: &Path,
        term: &str,
        entry: DictEntry,
    ) -> anyhow::Result<()>
    ```
    Implementation:
    - Load existing DictFile from path (or empty HashMap if file missing)
    - Insert/overwrite term -> entry
    - Serialize back to YAML string using serde_saphyr::to_string (or manual formatting matching existing dict style)
    - Write to dict_path

    **src/transport.rs** — add to TransportCmd enum:
    `DictAdd { thing: String, term: String }`
    Add to TransportReply: `DictAdded { term: String }`
    (These are for potential future daemon-routed use; the CLI impl is client-side.)

    **src/main.rs** — extend handle_dict_cli match arm:
    ```
    Some("add") => {
        // args: ["add", <thing>, <term>]
        let thing = args.get(1) else { eprintln!("usage: hum dict add <thing> <term>"); exit(1) };
        let term  = args.get(2) else { same };
        // Parse piece.hum to find the thing
        let piece_path = PathBuf::from("piece.hum");
        let content = fs::read_to_string(&piece_path)?;
        let piece: Piece = serde_saphyr::from_str(&content)?;
        let thing_def = piece.get(thing) else { eprintln!("error: thing '{}' not found", thing); exit(1) };
        let synth = thing_def.synth.clone().unwrap_or_default();
        let entry = DictEntry {
            synth,
            context: None,
            learned_from: Some(format!("{} ({})", thing, chrono::Local::now().format("%Y-%m-%d"))),
        };
        // Use chrono or just use a hardcoded date via std::time if chrono not in Cargo.toml.
        // If chrono is not available, use learned_from: Some(thing.to_string()).
        DictStore::add_entry(&PathBuf::from("hum.dict"), term, entry)?;
        println!("dict: added '{}' (learned from '{}')", term, thing);
    }
    ```

    Also update the usage string in the `_` arm and in run_cli to include `dict add`.

    Check Cargo.toml for chrono before using it; if absent, omit the date from learned-from.
  </action>
  <verify>
    <automated>cargo test --lib -- dict 2>&1 | tail -30</automated>
  </verify>
  <done>
    `cargo test --lib -- dict` passes including add_entry tests.
    `cargo build` succeeds with no errors.
    Manual smoke test: `hum dict add glass laser` writes a new entry to hum.dict with the glass thing's synth block.
  </done>
</task>

</tasks>

<verification>
cargo test --lib 2>&1 | tail -20
cargo build 2>&1 | tail -10
</verification>

<success_criteria>
1. When synth: in piece.hum differs from what pipe: would expand to, hum-rt inserts `# synth: manually tuned, pipe: may be stale` above the synth: key on next file reload
2. `hum dict add <thing> <term>` writes to hum.dict with the thing's synth block and learned-from metadata
3. All existing tests still pass
</success_criteria>

<output>
After completion, create `.planning/phases/10-translation-sync/10-1-SUMMARY.md`
</output>

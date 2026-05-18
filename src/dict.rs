use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::ir::types::SynthBlock;

/// A single dictionary entry: synth params + optional human context.
#[derive(Debug, Clone, Deserialize)]
pub struct DictEntry {
    pub synth: SynthBlock,
    pub context: Option<String>,
    #[serde(rename = "learned-from")]
    pub learned_from: Option<String>,
}

/// The raw hum.dict file format: term -> DictEntry mapping.
type DictFile = HashMap<String, DictEntry>;

/// Stores vocabulary entries keyed by term name.
#[derive(Debug, Default, Clone)]
pub struct DictStore {
    entries: HashMap<String, DictEntry>,
}

impl DictStore {
    /// Load a hum.dict YAML file. Missing file = empty store (not error).
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            tracing::info!("dict: no file at {:?}, using empty store", path);
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let dict_file: DictFile = serde_saphyr::from_str(&content)
            .map_err(|e| anyhow::anyhow!("dict parse error in {:?}: {}", path, e))?;
        tracing::info!("dict: loaded {} entries from {:?}", dict_file.len(), path);
        Ok(Self { entries: dict_file })
    }

    /// Merge global dict (base) with project dict (project wins on conflict).
    pub fn merge_with_global(global: DictStore, project: DictStore) -> DictStore {
        let mut merged = global.entries;
        for (k, v) in project.entries {
            merged.insert(k, v); // project overrides global
        }
        DictStore { entries: merged }
    }

    /// Look up a term. Returns the DictEntry for that entry.
    pub fn get(&self, term: &str) -> Option<&DictEntry> {
        self.entries.get(term)
    }

    /// All terms in sorted order.
    pub fn all_terms(&self) -> Vec<&str> {
        let mut terms: Vec<&str> = self.entries.keys().map(|s| s.as_str()).collect();
        terms.sort();
        terms
    }

    /// Add (or overwrite) a dictionary entry and persist to disk.
    ///
    /// Loads existing dict from path (or starts empty if file missing),
    /// inserts/overwrites the entry, and writes back as YAML.
    pub fn add_entry(
        dict_path: &Path,
        term: &str,
        entry: DictEntry,
    ) -> anyhow::Result<()> {
        // Load existing entries (or empty)
        let mut store = Self::load(dict_path)?;
        store.entries.insert(term.to_string(), entry);

        // Serialize all entries back to YAML manually
        let yaml = entries_to_yaml(&store.entries);
        std::fs::write(dict_path, yaml)?;
        tracing::info!("dict: wrote entry '{}' to {:?}", term, dict_path);
        Ok(())
    }
}

/// Serialize a SynthBlock to YAML key-value lines (indented 4 spaces).
fn synth_block_to_yaml(synth: &SynthBlock) -> String {
    let mut lines = Vec::new();
    if let Some(ref osc) = synth.osc {
        lines.push(format!("    osc: \"{}\"", osc));
    }
    if let Some(ref filter) = synth.filter {
        lines.push(format!("    filter: \"{}\"", filter));
    }
    if let Some(ref env) = synth.env {
        lines.push(format!("    env: \"{}\"", env));
    }
    if let Some(ref distort) = synth.distort {
        lines.push(format!("    distort: \"{}\"", distort));
    }
    if let Some(ref fx) = synth.fx {
        lines.push(format!("    fx: \"{}\"", fx));
    }
    if let Some(ref pan) = synth.pan {
        lines.push(format!("    pan: \"{}\"", pan));
    }
    if let Some(ref amp) = synth.amp {
        lines.push(format!("    amp: {}", amp));
    }
    if let Some(ref tempo) = synth.tempo {
        lines.push(format!("    tempo: \"{}\"", tempo));
    }
    if let Some(ref notes) = synth.notes {
        let notes_str: Vec<String> = notes.iter().map(|n| n.to_string()).collect();
        lines.push(format!("    notes: [{}]", notes_str.join(", ")));
    }
    lines.join("\n")
}

/// Serialize all dict entries to a YAML string matching hum.dict format.
fn entries_to_yaml(entries: &HashMap<String, DictEntry>) -> String {
    let mut output = String::new();
    // Sort terms for deterministic output
    let mut terms: Vec<&String> = entries.keys().collect();
    terms.sort();

    for (i, term) in terms.iter().enumerate() {
        let entry = &entries[*term];
        if i > 0 {
            output.push('\n');
        }
        output.push_str(&format!("{}:\n", term));
        output.push_str("  synth:\n");
        output.push_str(&synth_block_to_yaml(&entry.synth));
        output.push('\n');
        if let Some(ref ctx) = entry.context {
            output.push_str(&format!("  context: {}\n", ctx));
        }
        if let Some(ref lf) = entry.learned_from {
            output.push_str(&format!("  learned-from: {}\n", lf));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const DICT_YAML: &str = r#"laser:
  synth:
    osc: sine
  context: bright, cutting
warm:
  synth:
    filter: "lpf(cutoff: 800)"
  context: soft, intimate
"#;

    #[test]
    fn load_valid_dict_returns_entries() {
        let dir = tempfile::tempdir().unwrap();
        let dict_path = dir.path().join("hum.dict");
        let mut f = std::fs::File::create(&dict_path).unwrap();
        f.write_all(DICT_YAML.as_bytes()).unwrap();

        let store = DictStore::load(&dict_path).unwrap();
        assert_eq!(store.all_terms().len(), 2);
        assert!(store.get("laser").is_some());
        assert!(store.get("warm").is_some());

        // Check SynthBlock fields
        let laser = store.get("laser").unwrap();
        assert!(laser.synth.osc.is_some());
    }

    #[test]
    fn load_missing_file_returns_empty_store() {
        let store = DictStore::load(Path::new("/tmp/nonexistent-hum-dict-file.dict")).unwrap();
        assert!(store.all_terms().is_empty());
    }

    #[test]
    fn merge_with_global_project_wins_on_conflict() {
        let dir = tempfile::tempdir().unwrap();

        // Global dict: has "laser" and "dark"
        let global_path = dir.path().join("global.dict");
        std::fs::write(
            &global_path,
            "laser:\n  synth:\n    osc: saw\ndark:\n  synth:\n    amp: 0.2\n",
        )
        .unwrap();

        // Project dict: has "laser" (override) and "warm"
        let project_path = dir.path().join("project.dict");
        std::fs::write(&project_path, DICT_YAML).unwrap();

        let global = DictStore::load(&global_path).unwrap();
        let project = DictStore::load(&project_path).unwrap();
        let merged = DictStore::merge_with_global(global, project);

        // Project's "laser" wins
        let laser = merged.get("laser").unwrap();
        assert!(
            laser.synth.osc.is_some(),
            "project laser (sine) should override global (saw)"
        );
        // Global's "dark" is preserved
        assert!(merged.get("dark").is_some());
        // Project's "warm" is present
        assert!(merged.get("warm").is_some());
        // Total: 3 terms
        assert_eq!(merged.all_terms().len(), 3);
    }

    #[test]
    fn get_existing_term_returns_some() {
        let dir = tempfile::tempdir().unwrap();
        let dict_path = dir.path().join("hum.dict");
        std::fs::write(&dict_path, DICT_YAML).unwrap();

        let store = DictStore::load(&dict_path).unwrap();
        assert!(store.get("laser").is_some());
    }

    #[test]
    fn get_nonexistent_term_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let dict_path = dir.path().join("hum.dict");
        std::fs::write(&dict_path, DICT_YAML).unwrap();

        let store = DictStore::load(&dict_path).unwrap();
        assert!(store.get("nonexistent").is_none());
    }

    // --- add_entry tests ---

    #[test]
    fn add_entry_writes_new_entry_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let dict_path = dir.path().join("hum.dict");

        let entry = DictEntry {
            synth: crate::ir::types::SynthBlock {
                osc: Some(crate::ir::types::OscLayer(vec![crate::ir::types::OscPrimitive::Sine { freq: None }])),
                amp: Some(crate::ir::types::Value::Fixed(0.5)),
                notes: None, filter: None, env: None,
                distort: None, fx: None, pan: None, tempo: None,
                sample: None, loop_mode: None,
            },
            context: None,
            learned_from: Some("glass".to_string()),
        };

        DictStore::add_entry(&dict_path, "bright", entry).unwrap();

        // Verify: reload and check
        let store = DictStore::load(&dict_path).unwrap();
        assert!(store.get("bright").is_some());
        let e = store.get("bright").unwrap();
        assert_eq!(e.learned_from.as_deref(), Some("glass"));
        assert!(e.synth.osc.is_some());
    }

    #[test]
    fn add_entry_overwrites_existing_term() {
        let dir = tempfile::tempdir().unwrap();
        let dict_path = dir.path().join("hum.dict");
        std::fs::write(&dict_path, DICT_YAML).unwrap();

        // Overwrite "laser" with a new entry
        let entry = DictEntry {
            synth: crate::ir::types::SynthBlock {
                osc: Some(crate::ir::types::OscLayer(vec![crate::ir::types::OscPrimitive::Saw { detune: None }])),
                amp: Some(crate::ir::types::Value::Fixed(0.9)),
                notes: None, filter: None, env: None,
                distort: None, fx: None, pan: None, tempo: None,
                sample: None, loop_mode: None,
            },
            context: Some("updated".to_string()),
            learned_from: Some("pulse".to_string()),
        };

        DictStore::add_entry(&dict_path, "laser", entry).unwrap();

        let store = DictStore::load(&dict_path).unwrap();
        let laser = store.get("laser").unwrap();
        assert_eq!(laser.context.as_deref(), Some("updated"));
        assert_eq!(laser.learned_from.as_deref(), Some("pulse"));
        // "warm" should still exist
        assert!(store.get("warm").is_some());
    }

    #[test]
    fn add_entry_to_nonexistent_file_creates_it() {
        let dir = tempfile::tempdir().unwrap();
        let dict_path = dir.path().join("new.dict");

        let entry = DictEntry {
            synth: crate::ir::types::SynthBlock {
                osc: Some(crate::ir::types::OscLayer(vec![crate::ir::types::OscPrimitive::Sine { freq: None }])),
                notes: None, filter: None, env: None,
                distort: None, fx: None, pan: None, amp: None, tempo: None,
                sample: None, loop_mode: None,
            },
            context: None,
            learned_from: None,
        };

        DictStore::add_entry(&dict_path, "test-term", entry).unwrap();
        assert!(dict_path.exists());
        let store = DictStore::load(&dict_path).unwrap();
        assert_eq!(store.all_terms().len(), 1);
        assert!(store.get("test-term").is_some());
    }

    #[test]
    fn dict_entry_has_context_field() {
        let dir = tempfile::tempdir().unwrap();
        let dict_path = dir.path().join("hum.dict");
        std::fs::write(&dict_path, DICT_YAML).unwrap();

        let store = DictStore::load(&dict_path).unwrap();
        let laser = store.get("laser").unwrap();
        assert_eq!(laser.context.as_deref(), Some("bright, cutting"));

        let warm = store.get("warm").unwrap();
        assert_eq!(warm.context.as_deref(), Some("soft, intimate"));
    }
}

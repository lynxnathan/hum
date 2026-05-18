use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::ir::types::SynthBlock;

/// A standalone instrument file: `type: instrument` + `synth:` block.
/// This is NOT a piece map — it's a single-definition file from instruments/.
#[derive(Debug, Deserialize)]
struct InstrumentFile {
    r#type: String,
    synth: SynthBlock,
}

/// Stores reusable instrument definitions keyed by name (file stem).
#[derive(Debug, Default)]
pub struct InstrumentStore {
    instruments: HashMap<String, SynthBlock>,
}

impl InstrumentStore {
    /// Scan a directory for `*.hum` files with `type: instrument`.
    /// Missing directory = warning + empty store (not error).
    /// Non-instrument files are skipped with a warning.
    pub fn load_dir(dir: &Path) -> anyhow::Result<Self> {
        let mut instruments = HashMap::new();

        if !dir.exists() {
            tracing::warn!("instruments directory not found: {:?}", dir);
            return Ok(Self { instruments });
        }

        let entries = std::fs::read_dir(dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Only process .hum files
            let ext = path.extension().and_then(|e| e.to_str());
            if ext != Some("hum") {
                continue;
            }

            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("could not read {:?}: {}", path, e);
                    continue;
                }
            };

            let inst_file: InstrumentFile = match serde_saphyr::from_str(&content) {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!("could not parse {:?}: {}", path, e);
                    continue;
                }
            };

            if inst_file.r#type != "instrument" {
                tracing::warn!("skipping {:?}: type is '{}', not 'instrument'", path, inst_file.r#type);
                continue;
            }

            tracing::info!("loaded instrument '{}' from {:?}", stem, path);
            instruments.insert(stem, inst_file.synth);
        }

        Ok(Self { instruments })
    }

    /// Look up an instrument by name.
    pub fn get(&self, name: &str) -> Option<&SynthBlock> {
        self.instruments.get(name)
    }

    /// Merge two SynthBlocks: `over` fields win when Some, `base` fills gaps.
    pub fn merge(base: &SynthBlock, over: &SynthBlock) -> SynthBlock {
        SynthBlock {
            notes: over.notes.clone().or_else(|| base.notes.clone()),
            osc: over.osc.clone().or_else(|| base.osc.clone()),
            filter: over.filter.clone().or_else(|| base.filter.clone()),
            env: over.env.clone().or_else(|| base.env.clone()),
            distort: over.distort.clone().or_else(|| base.distort.clone()),
            fx: over.fx.clone().or_else(|| base.fx.clone()),
            pan: over.pan.clone().or_else(|| base.pan.clone()),
            amp: over.amp.clone().or_else(|| base.amp.clone()),
            tempo: over.tempo.clone().or_else(|| base.tempo.clone()),
            sample: over.sample.clone().or_else(|| base.sample.clone()),
            loop_mode: over.loop_mode.or(base.loop_mode),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::types::{OscPrimitive, OscLayer, Value};

    fn empty_synth() -> SynthBlock {
        SynthBlock {
            notes: None, osc: None, filter: None, env: None,
            distort: None, fx: None, pan: None, amp: None, tempo: None,
            sample: None, loop_mode: None,
        }
    }

    #[test]
    fn merge_base_osc_override_notes() {
        let base = SynthBlock {
            osc: Some(OscLayer(vec![OscPrimitive::Saw { detune: None }])),
            ..empty_synth()
        };
        let over = SynthBlock {
            notes: Some(vec!["D4".into()]),
            ..empty_synth()
        };
        let merged = InstrumentStore::merge(&base, &over);
        assert_eq!(merged.osc, Some(OscLayer(vec![OscPrimitive::Saw { detune: None }])));
        assert_eq!(merged.notes, Some(vec!["D4".into()]));
    }

    #[test]
    fn merge_override_amp_wins() {
        let base = SynthBlock {
            amp: Some(Value::Fixed(0.8)),
            ..empty_synth()
        };
        let over = SynthBlock {
            amp: Some(Value::Fixed(0.3)),
            ..empty_synth()
        };
        let merged = InstrumentStore::merge(&base, &over);
        assert_eq!(merged.amp, Some(Value::Fixed(0.3)));
    }

    #[test]
    fn merge_all_none_returns_all_none() {
        let base = empty_synth();
        let over = empty_synth();
        let merged = InstrumentStore::merge(&base, &over);
        assert!(merged.osc.is_none());
        assert!(merged.amp.is_none());
    }

    #[test]
    fn load_dir_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstrumentStore::load_dir(dir.path()).unwrap();
        assert!(store.get("anything").is_none());
    }

    #[test]
    fn load_dir_missing_dir() {
        let store = InstrumentStore::load_dir(Path::new("/tmp/nonexistent-hum-instruments-dir")).unwrap();
        assert!(store.get("anything").is_none());
    }

    #[test]
    fn load_dir_parses_instrument_file() {
        let dir = tempfile::tempdir().unwrap();
        let inst_path = dir.path().join("test-inst.hum");
        std::fs::write(&inst_path, "type: instrument\nsynth:\n  osc: saw\n  amp: 0.5\n").unwrap();

        let store = InstrumentStore::load_dir(dir.path()).unwrap();
        let synth = store.get("test-inst").unwrap();
        assert_eq!(synth.osc, Some(OscLayer(vec![OscPrimitive::Saw { detune: None }])));
        assert_eq!(synth.amp, Some(Value::Fixed(0.5)));
    }

    #[test]
    fn load_dir_skips_non_instrument() {
        let dir = tempfile::tempdir().unwrap();
        let inst_path = dir.path().join("not-instrument.hum");
        std::fs::write(&inst_path, "type: stage\nsynth:\n  osc: saw\n").unwrap();

        let store = InstrumentStore::load_dir(dir.path()).unwrap();
        assert!(store.get("not-instrument").is_none());
    }
}

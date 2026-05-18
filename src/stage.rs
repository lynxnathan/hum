use std::collections::HashMap;

use anyhow::Result;

use crate::ir::compiler::compile_synth_block;
use crate::ir::types::{FxPrimitive, OscPrimitive, OscLayer, SynthBlock, Value};

/// Configuration for a single stage (group + effect chain).
#[derive(Debug, Clone)]
pub struct StageConfig {
    /// Which things this stage routes through its group.
    pub applies_to: Vec<String>,
    /// The fx chain applied at the tail of the group.
    pub fx: Option<FxPrimitive>,
    /// Allocated scsynth Group node ID.
    pub group_id: i32,
    /// Allocated scsynth node ID for the effect synth at group tail.
    pub effect_node_id: Option<i32>,
}

/// Maps stage names to their configuration and scsynth routing info.
pub struct StageStore {
    stages: HashMap<String, StageConfig>,
}

impl StageStore {
    pub fn new() -> Self {
        Self {
            stages: HashMap::new(),
        }
    }

    /// Returns which group_id a thing should be spawned into.
    /// Returns None if the thing is not in any stage (use default group 1).
    pub fn group_for_thing(&self, thing_name: &str) -> Option<i32> {
        for config in self.stages.values() {
            if config.applies_to.iter().any(|n| n == thing_name) {
                return Some(config.group_id);
            }
        }
        None
    }

    /// Insert a stage configuration.
    pub fn insert(&mut self, name: String, config: StageConfig) {
        self.stages.insert(name, config);
    }

    /// Iterate over all stages.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &StageConfig)> {
        self.stages.iter()
    }

    /// Number of stages.
    pub fn len(&self) -> usize {
        self.stages.len()
    }
}

/// Compile a stage effect SynthDef.
///
/// Builds a minimal SynthBlock with a sine oscillator carrier and the stage's fx chain,
/// then compiles it via the standard IR compiler. The effect node reads from the group bus
/// and applies the fx at the tail.
///
/// The SynthDef name will be "stage-{stage_name}".
pub fn compile_stage_effect(stage_name: &str, fx: &FxPrimitive) -> Result<Vec<u8>> {
    let synthdef_name = format!("stage-{}", stage_name);
    let block = SynthBlock {
        notes: None,
        osc: Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])),
        filter: None,
        env: None,
        distort: None,
        fx: Some(fx.clone()),
        pan: None,
        amp: Some(Value::Fixed(0.3)),
        tempo: None,
        sample: None,
        loop_mode: None,
    };
    compile_synth_block(&synthdef_name, &block)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::types::FxPrimitive;

    #[test]
    fn group_for_thing_returns_group_id() {
        let mut store = StageStore::new();
        store.insert(
            "reverb-stage".to_string(),
            StageConfig {
                applies_to: vec!["ghost".to_string(), "glass".to_string()],
                fx: Some(FxPrimitive::Reverb { mix: Value::Fixed(0.5), room: Value::Fixed(0.8) }),
                group_id: 2000,
                effect_node_id: Some(2001),
            },
        );

        assert_eq!(store.group_for_thing("ghost"), Some(2000));
        assert_eq!(store.group_for_thing("glass"), Some(2000));
        assert_eq!(store.group_for_thing("other"), None);
    }

    #[test]
    fn group_for_thing_none_when_empty() {
        let store = StageStore::new();
        assert_eq!(store.group_for_thing("anything"), None);
    }

    #[test]
    fn compile_stage_effect_produces_scgf() {
        let fx = FxPrimitive::Reverb { mix: Value::Fixed(0.7), room: Value::Fixed(0.95) };
        let bytes = compile_stage_effect("test-stage", &fx).unwrap();
        assert_eq!(&bytes[0..4], b"SCgf");
        assert!(bytes.len() > 50);
    }

    #[test]
    fn compile_stage_effect_delay() {
        let fx = FxPrimitive::Delay { time: Value::Fixed(0.3), feedback: Value::Fixed(0.5) };
        let bytes = compile_stage_effect("delay-stage", &fx).unwrap();
        assert_eq!(&bytes[0..4], b"SCgf");
    }

    #[test]
    fn multiple_stages_separate_groups() {
        let mut store = StageStore::new();
        store.insert(
            "stage-a".to_string(),
            StageConfig {
                applies_to: vec!["thing-1".to_string()],
                fx: None,
                group_id: 3000,
                effect_node_id: None,
            },
        );
        store.insert(
            "stage-b".to_string(),
            StageConfig {
                applies_to: vec!["thing-2".to_string()],
                fx: None,
                group_id: 3001,
                effect_node_id: None,
            },
        );

        assert_eq!(store.group_for_thing("thing-1"), Some(3000));
        assert_eq!(store.group_for_thing("thing-2"), Some(3001));
        assert_eq!(store.len(), 2);
    }
}

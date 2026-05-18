use indexmap::IndexMap;
use serde::Deserialize;

use crate::ir::types::{FxPrimitive, SynthBlock};

/// A parsed .hum file. Keys are thing names (e.g. "space-crackle").
/// Preserves insertion order via IndexMap.
pub type Piece = IndexMap<String, ThingDef>;

/// The type of a thing definition: instrument, stage, etc.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ThingType {
    Instrument,
    Stage,
}

/// One named thing in a piece. All fields optional -- absent means "not decided".
/// deny_unknown_fields enforces the schema: any unrecognized field is a parse error.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThingDef {
    // -- Runtime-actionable fields --

    /// When this thing enters (e.g. "0s", "10s")
    pub at: Option<String>,

    /// When this thing exits (e.g. "30s", absent = open-ended)
    pub until: Option<String>,

    /// Trajectories: what changes over time. Single string or list of strings.
    pub does: Option<DoesField>,

    /// Stereo placement (e.g. "wide", "center", "left")
    /// `where` is a Rust keyword, so we rename from YAML "where" to Rust "location"
    #[serde(rename = "where")]
    pub location: Option<String>,

    /// Sub-components with own behavior/placement (recursive)
    pub has: Option<IndexMap<String, ThingDef>>,

    /// Contextual relationship to another thing
    pub within: Option<String>,

    /// Rhythmic pattern (e.g. "every beat", "every 2s")
    pub every: Option<String>,

    // -- LLM-facing fields (parsed but not runtime-actionable in Phase 2) --

    /// What it sounds like. Free text. Primary input for LLM.
    pub like: Option<String>,

    /// Cultural reference. Informational, not spec.
    /// `ref` is a Rust keyword, so we rename from YAML "ref" to Rust "reference"
    #[serde(rename = "ref")]
    pub reference: Option<String>,

    /// Emotional context influencing LLM choices
    pub mood: Option<String>,

    // -- Synth IR (v2) --

    /// Inline synthesis parameters compiled directly to OSC.
    pub synth: Option<SynthBlock>,

    // -- Instrument + Stage (v2) --

    /// Thing type: instrument, stage, etc.
    #[serde(rename = "type")]
    pub thing_type: Option<ThingType>,

    /// Reference to a reusable instrument definition from instruments/ dir.
    pub instrument: Option<String>,

    /// Reference to a dict term: pulls synth params from hum.dict as base.
    /// Priority: .scd > instrument: > style: > bare synth:
    pub style: Option<String>,

    /// Which things this stage applies to (for type: stage).
    #[serde(rename = "applies-to")]
    pub applies_to: Option<Vec<String>>,

    /// Top-level fx for stage things (e.g. `fx: reverb(mix: 0.7, room: 0.95)`).
    /// None for normal things and instruments, Some for stages.
    pub fx: Option<FxPrimitive>,

    /// Pipe expression: functional composition over sound transforms.
    /// Multiline string parsed into PipeExpr at execution time.
    pub pipe: Option<String>,
}

/// The `does:` field can be a single string or a list of strings.
/// Untagged enum lets serde try Single first, then Multi.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum DoesField {
    Single(String),
    Multi(Vec<String>),
}

impl DoesField {
    /// Normalize to a vec of string slices regardless of variant.
    pub fn as_vec(&self) -> Vec<&str> {
        match self {
            DoesField::Single(s) => vec![s.as_str()],
            DoesField::Multi(v) => v.iter().map(|s| s.as_str()).collect(),
        }
    }
}

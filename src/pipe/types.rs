/// Pipe expression AST — parsed from `pipe:` blocks in .hum files.
///
/// A PipeExpr has a source (thing name or thing.field) and a chain of transforms.
/// This is purely a data model — execution/expansion happens in Plan 3.

#[derive(Debug, Clone, PartialEq)]
pub enum PipeSource {
    /// Bare thing name, e.g. "glass"
    Thing(String),
    /// Thing with field accessor, e.g. "glass.notes"
    Field(String, String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Transform {
    /// Clone into n parallel voices
    Replicate { n: usize },
    /// Apply per-voice with index: each(i => expr)
    Each { expr: String },
    /// Transform each note/event: map(n => expr)
    Map { expr: String },
    /// Pitch shift by semitones
    Shift { semitones: i32 },
    /// Distribute across stereo field: spread(pan: lo~hi)
    Spread { lo: f32, hi: f32 },
    /// Change playback speed: tempo(Xs/note)
    Tempo { seconds_per_note: f32 },
    /// First n notes
    Take { n: usize },
    /// Loop n times
    Repeat { n: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PipeExpr {
    pub source: PipeSource,
    pub transforms: Vec<Transform>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_expr_with_thing_source() {
        let expr = PipeExpr {
            source: PipeSource::Thing("glass".to_string()),
            transforms: vec![
                Transform::Replicate { n: 3 },
                Transform::Shift { semitones: 4 },
            ],
        };
        assert_eq!(expr.source, PipeSource::Thing("glass".to_string()));
        assert_eq!(expr.transforms.len(), 2);
    }

    #[test]
    fn pipe_expr_with_field_source() {
        let expr = PipeExpr {
            source: PipeSource::Field("glass".to_string(), "notes".to_string()),
            transforms: vec![Transform::Take { n: 4 }],
        };
        assert_eq!(
            expr.source,
            PipeSource::Field("glass".to_string(), "notes".to_string())
        );
    }

    #[test]
    fn all_transform_variants_constructible() {
        let transforms = vec![
            Transform::Replicate { n: 3 },
            Transform::Each { expr: "shift(semitones: i * 4)".to_string() },
            Transform::Map { expr: "n - 24".to_string() },
            Transform::Shift { semitones: -12 },
            Transform::Spread { lo: -0.8, hi: 0.8 },
            Transform::Tempo { seconds_per_note: 0.35 },
            Transform::Take { n: 4 },
            Transform::Repeat { n: 8 },
        ];
        assert_eq!(transforms.len(), 8);
    }
}

/// Pipe executor — expands a PipeExpr into N named SynthBlocks.
///
/// Given a thing name, a parsed PipeExpr, and the full piece, resolves the
/// source thing, applies transforms in order, and returns a Vec of
/// (synthetic_name, SynthBlock) pairs ready for compilation.

use anyhow::{bail, Result};

use crate::ir::notes::note_to_midi;
use crate::ir::types::{PanPrimitive, SynthBlock};
use crate::parser::Piece;

use super::types::{PipeExpr, PipeSource, Transform};

/// Expand a pipe expression into named SynthBlocks.
///
/// Returns Vec<(name, SynthBlock)> where name = "{thing_name}-pipe-{i}".
/// Each SynthBlock can be independently compiled and sent to scsynth.
pub fn expand_pipe(
    thing_name: &str,
    expr: &PipeExpr,
    piece: &Piece,
) -> Result<Vec<(String, SynthBlock)>> {
    // Step 1: Resolve source
    let base = resolve_source(&expr.source, piece)?;

    // Step 2: Start with 1 voice
    let mut voices: Vec<SynthBlock> = vec![base];

    // Step 3: Apply transforms in order
    for transform in &expr.transforms {
        voices = apply_transform(voices, transform)?;
    }

    // Step 4: Return named pairs
    let result = voices
        .into_iter()
        .enumerate()
        .map(|(i, block)| (format!("{}-pipe-{}", thing_name, i), block))
        .collect();

    Ok(result)
}

/// Resolve a pipe source to a base SynthBlock.
fn resolve_source(source: &PipeSource, piece: &Piece) -> Result<SynthBlock> {
    match source {
        PipeSource::Thing(name) => {
            let thing = piece
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("pipe source thing '{}' not found", name))?;
            let synth = thing
                .synth
                .as_ref()
                .ok_or_else(|| {
                    anyhow::anyhow!("pipe source thing '{}' has no synth: block", name)
                })?;
            Ok(synth.clone())
        }
        PipeSource::Field(thing_name, field) => {
            let thing = piece
                .get(thing_name)
                .ok_or_else(|| {
                    anyhow::anyhow!("pipe source thing '{}' not found", thing_name)
                })?;
            let synth = thing
                .synth
                .as_ref()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "pipe source thing '{}' has no synth: block",
                        thing_name
                    )
                })?;
            match field.as_str() {
                "notes" => {
                    let notes = synth.notes.clone().ok_or_else(|| {
                        anyhow::anyhow!(
                            "pipe source '{}.notes': thing has no notes",
                            thing_name
                        )
                    })?;
                    Ok(SynthBlock {
                        notes: Some(notes),
                        osc: None,
                        filter: None,
                        env: None,
                        distort: None,
                        fx: None,
                        pan: None,
                        amp: None,
                        tempo: None,
                        sample: None,
                        loop_mode: None,
                    })
                }
                other => bail!("pipe field accessor '.{}' not yet supported", other),
            }
        }
    }
}

/// Apply a single transform to the current set of voices.
fn apply_transform(mut voices: Vec<SynthBlock>, transform: &Transform) -> Result<Vec<SynthBlock>> {
    match transform {
        Transform::Replicate { n } => {
            if voices.len() != 1 {
                bail!("replicate() must be applied to a single voice (got {})", voices.len());
            }
            let base = voices.remove(0);
            Ok((0..*n).map(|_| base.clone()).collect())
        }
        Transform::Shift { semitones } => {
            for voice in &mut voices {
                if let Some(notes) = &mut voice.notes {
                    *notes = notes.iter().map(|n| shift_note(n, *semitones)).collect();
                }
            }
            Ok(voices)
        }
        Transform::Spread { lo, hi } => {
            let n = voices.len();
            for (i, voice) in voices.iter_mut().enumerate() {
                let pan_value = if n == 1 {
                    (lo + hi) / 2.0
                } else {
                    lo + (hi - lo) * i as f32 / (n - 1) as f32
                };
                voice.pan = Some(PanPrimitive::Fixed { value: pan_value });
            }
            Ok(voices)
        }
        Transform::Tempo { seconds_per_note } => {
            for voice in &mut voices {
                voice.tempo = Some(format!("{}s/note", seconds_per_note));
            }
            Ok(voices)
        }
        Transform::Take { n } => {
            for voice in &mut voices {
                if let Some(notes) = &mut voice.notes {
                    notes.truncate(*n);
                }
            }
            Ok(voices)
        }
        Transform::Repeat { n } => {
            for voice in &mut voices {
                if let Some(notes) = &mut voice.notes {
                    let original = notes.clone();
                    for _ in 1..*n {
                        notes.extend(original.iter().cloned());
                    }
                }
            }
            Ok(voices)
        }
        Transform::Each { expr } => {
            apply_each(&mut voices, expr)?;
            Ok(voices)
        }
        Transform::Map { expr } => {
            tracing::warn!("map({}) not yet implemented, skipping", expr);
            Ok(voices)
        }
    }
}

/// Apply an each(i => ...) expression to voices.
/// Currently only supports: `i => shift(semitones: i * N)`
fn apply_each(voices: &mut [SynthBlock], expr: &str) -> Result<()> {
    // Parse pattern: "i => shift(semitones: i * N)"
    let parts: Vec<&str> = expr.splitn(2, "=>").collect();
    if parts.len() != 2 {
        tracing::warn!("each() expression not recognized: '{}', skipping", expr);
        return Ok(());
    }

    let body = parts[1].trim();

    // Match: shift(semitones: i * N)
    if let Some(inner) = body.strip_prefix("shift(semitones:") {
        let inner = inner.trim().trim_end_matches(')').trim();
        // Parse "i * N"
        if let Some(mul_str) = inner.strip_prefix("i *") {
            let n: i32 = mul_str.trim().parse().map_err(|_| {
                anyhow::anyhow!("each() shift: could not parse multiplier from '{}'", inner)
            })?;
            for (i, voice) in voices.iter_mut().enumerate() {
                let semitones = i as i32 * n;
                if let Some(notes) = &mut voice.notes {
                    *notes = notes.iter().map(|note| shift_note(note, semitones)).collect();
                }
            }
            return Ok(());
        }
        // Also support "i*N" without spaces
        if let Some(mul_str) = inner.strip_prefix("i*") {
            let n: i32 = mul_str.trim().parse().map_err(|_| {
                anyhow::anyhow!("each() shift: could not parse multiplier from '{}'", inner)
            })?;
            for (i, voice) in voices.iter_mut().enumerate() {
                let semitones = i as i32 * n;
                if let Some(notes) = &mut voice.notes {
                    *notes = notes.iter().map(|note| shift_note(note, semitones)).collect();
                }
            }
            return Ok(());
        }
    }

    tracing::warn!("each() expression not yet supported: '{}', skipping", expr);
    Ok(())
}

// ---------------------------------------------------------------------------
// Note shifting helper
// ---------------------------------------------------------------------------

const PITCH_CLASSES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Shift a note name by the given number of semitones.
/// "-" (rest) passes through unchanged. Unrecognized formats pass through unchanged.
/// Uses sharps for output (e.g. Eb4 shifted +1 = E4, Eb4 shifted -1 = D4).
pub fn shift_note(note: &str, semitones: i32) -> String {
    if note == "-" || semitones == 0 {
        return note.to_string();
    }

    // Use note_to_midi for parsing
    match note_to_midi(note) {
        Some(midi) => {
            let new_midi = (midi as i32 + semitones).clamp(0, 127) as u8;
            midi_to_note_name(new_midi)
        }
        None => {
            // Unrecognized format, pass through
            tracing::warn!("shift_note: unrecognized note format '{}', passing through", note);
            note.to_string()
        }
    }
}

/// Convert a MIDI number back to a note name using sharps.
/// C4 = 60, so octave = (midi / 12) - 1, pitch_class = midi % 12.
fn midi_to_note_name(midi: u8) -> String {
    let pitch_class = (midi % 12) as usize;
    let octave = (midi / 12) as i8 - 1;
    format!("{}{}", PITCH_CLASSES[pitch_class], octave)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::types::{OscPrimitive, OscLayer, Value};
    use crate::parser::ThingDef;
    use indexmap::IndexMap;

    fn empty_synth() -> SynthBlock {
        SynthBlock {
            notes: None,
            osc: None,
            filter: None,
            env: None,
            distort: None,
            fx: None,
            pan: None,
            amp: None,
            tempo: None,
            sample: None,
            loop_mode: None,
        }
    }

    fn make_thing() -> ThingDef {
        ThingDef {
            at: None,
            until: None,
            does: None,
            location: None,
            has: None,
            within: None,
            every: None,
            like: None,
            reference: None,
            mood: None,
            synth: None,
            thing_type: None,
            instrument: None,
            style: None,
            applies_to: None,
            fx: None,
            pipe: None,
        }
    }

    fn glass_piece() -> Piece {
        let mut piece = IndexMap::new();
        let mut glass = make_thing();
        glass.synth = Some(SynthBlock {
            notes: Some(vec!["D4".into(), "Eb4".into()]),
            osc: Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])),
            amp: Some(Value::Fixed(0.3)),
            ..empty_synth()
        });
        piece.insert("glass".into(), glass);
        piece
    }

    // --- Source resolution ---

    #[test]
    fn source_thing_returns_full_synth() {
        let piece = glass_piece();
        let expr = PipeExpr {
            source: PipeSource::Thing("glass".to_string()),
            transforms: vec![],
        };
        let result = expand_pipe("glass-swarm", &expr, &piece).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "glass-swarm-pipe-0");
        assert_eq!(result[0].1.osc, Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])));
        assert_eq!(result[0].1.notes, Some(vec!["D4".into(), "Eb4".into()]));
    }

    #[test]
    fn source_field_notes_extracts_only_notes() {
        let piece = glass_piece();
        let expr = PipeExpr {
            source: PipeSource::Field("glass".to_string(), "notes".to_string()),
            transforms: vec![],
        };
        let result = expand_pipe("glass-drum", &expr, &piece).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].1.notes, Some(vec!["D4".into(), "Eb4".into()]));
        // osc should be None (only notes extracted)
        assert!(result[0].1.osc.is_none());
    }

    #[test]
    fn missing_source_thing_returns_err() {
        let piece: Piece = IndexMap::new();
        let expr = PipeExpr {
            source: PipeSource::Thing("nonexistent".to_string()),
            transforms: vec![],
        };
        assert!(expand_pipe("test", &expr, &piece).is_err());
    }

    #[test]
    fn source_thing_no_synth_returns_err() {
        let mut piece: Piece = IndexMap::new();
        piece.insert("empty".into(), make_thing());
        let expr = PipeExpr {
            source: PipeSource::Thing("empty".to_string()),
            transforms: vec![],
        };
        assert!(expand_pipe("test", &expr, &piece).is_err());
    }

    // --- Replicate ---

    #[test]
    fn replicate_creates_n_voices() {
        let piece = glass_piece();
        let expr = PipeExpr {
            source: PipeSource::Thing("glass".to_string()),
            transforms: vec![Transform::Replicate { n: 3 }],
        };
        let result = expand_pipe("glass-swarm", &expr, &piece).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, "glass-swarm-pipe-0");
        assert_eq!(result[1].0, "glass-swarm-pipe-1");
        assert_eq!(result[2].0, "glass-swarm-pipe-2");
        // All should have same synth
        for (_, block) in &result {
            assert_eq!(block.osc, Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])));
            assert_eq!(block.notes, Some(vec!["D4".into(), "Eb4".into()]));
        }
    }

    // --- Shift ---

    #[test]
    fn shift_notes_by_semitones() {
        // D4=62, Eb4=63; shift +4 -> F#4=66, G4=67
        assert_eq!(shift_note("D4", 4), "F#4");
        assert_eq!(shift_note("Eb4", 4), "G4");
    }

    #[test]
    fn shift_rest_passes_through() {
        assert_eq!(shift_note("-", 4), "-");
    }

    #[test]
    fn shift_negative_semitones() {
        // D4=62, shift -2 -> C4=60
        assert_eq!(shift_note("D4", -2), "C4");
    }

    #[test]
    fn shift_clamps_to_valid_range() {
        // C0=12, shift -20 -> clamps to 0
        let result = shift_note("C0", -20);
        assert_eq!(result, "C-1"); // MIDI 0 = C-1
    }

    // --- Spread ---

    #[test]
    fn spread_distributes_pan_across_3_voices() {
        let piece = glass_piece();
        let expr = PipeExpr {
            source: PipeSource::Thing("glass".to_string()),
            transforms: vec![
                Transform::Replicate { n: 3 },
                Transform::Spread { lo: -0.8, hi: 0.8 },
            ],
        };
        let result = expand_pipe("test", &expr, &piece).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].1.pan, Some(PanPrimitive::Fixed { value: -0.8 }));
        assert_eq!(result[1].1.pan, Some(PanPrimitive::Fixed { value: 0.0 }));
        assert_eq!(result[2].1.pan, Some(PanPrimitive::Fixed { value: 0.8 }));
    }

    #[test]
    fn spread_single_voice_centers() {
        let piece = glass_piece();
        let expr = PipeExpr {
            source: PipeSource::Thing("glass".to_string()),
            transforms: vec![Transform::Spread { lo: -0.8, hi: 0.8 }],
        };
        let result = expand_pipe("test", &expr, &piece).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].1.pan, Some(PanPrimitive::Fixed { value: 0.0 }));
    }

    // --- Take ---

    #[test]
    fn take_truncates_notes() {
        let mut piece = glass_piece();
        // Add more notes
        if let Some(synth) = piece.get_mut("glass").unwrap().synth.as_mut() {
            synth.notes = Some(vec!["A4".into(), "B4".into(), "C5".into(), "D5".into()]);
        }
        let expr = PipeExpr {
            source: PipeSource::Thing("glass".to_string()),
            transforms: vec![Transform::Take { n: 2 }],
        };
        let result = expand_pipe("test", &expr, &piece).unwrap();
        assert_eq!(result[0].1.notes, Some(vec!["A4".into(), "B4".into()]));
    }

    // --- Repeat ---

    #[test]
    fn repeat_extends_notes() {
        let mut piece: Piece = IndexMap::new();
        let mut thing = make_thing();
        thing.synth = Some(SynthBlock {
            notes: Some(vec!["A4".into(), "B4".into()]),
            osc: Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])),
            ..empty_synth()
        });
        piece.insert("src".into(), thing);

        let expr = PipeExpr {
            source: PipeSource::Thing("src".to_string()),
            transforms: vec![Transform::Repeat { n: 2 }],
        };
        let result = expand_pipe("test", &expr, &piece).unwrap();
        assert_eq!(
            result[0].1.notes,
            Some(vec!["A4".into(), "B4".into(), "A4".into(), "B4".into()])
        );
    }

    // --- Tempo ---

    #[test]
    fn tempo_sets_on_each_voice() {
        let piece = glass_piece();
        let expr = PipeExpr {
            source: PipeSource::Thing("glass".to_string()),
            transforms: vec![
                Transform::Replicate { n: 2 },
                Transform::Tempo { seconds_per_note: 0.35 },
            ],
        };
        let result = expand_pipe("test", &expr, &piece).unwrap();
        for (_, block) in &result {
            assert_eq!(block.tempo, Some("0.35s/note".to_string()));
        }
    }

    // --- Each + Shift combo ---

    #[test]
    fn each_shift_distributes_semitones_by_index() {
        let piece = glass_piece();
        let expr = PipeExpr {
            source: PipeSource::Thing("glass".to_string()),
            transforms: vec![
                Transform::Replicate { n: 3 },
                Transform::Each {
                    expr: "i => shift(semitones: i * 4)".to_string(),
                },
            ],
        };
        let result = expand_pipe("glass-swarm", &expr, &piece).unwrap();
        assert_eq!(result.len(), 3);
        // Voice 0: shift 0 -> D4, Eb4 (unchanged)
        assert_eq!(result[0].1.notes, Some(vec!["D4".into(), "Eb4".into()]));
        // Voice 1: shift 4 -> F#4, G4
        assert_eq!(result[1].1.notes, Some(vec!["F#4".into(), "G4".into()]));
        // Voice 2: shift 8 -> D4(62)+8=70=A#4, Eb4(63)+8=71=B4
        assert_eq!(result[2].1.notes, Some(vec!["A#4".into(), "B4".into()]));
    }

    // --- Full combo: replicate + each + spread ---

    #[test]
    fn full_combo_replicate_each_spread() {
        let piece = glass_piece();
        let expr = PipeExpr {
            source: PipeSource::Thing("glass".to_string()),
            transforms: vec![
                Transform::Replicate { n: 3 },
                Transform::Each {
                    expr: "i => shift(semitones: i * 4)".to_string(),
                },
                Transform::Spread { lo: -0.8, hi: 0.8 },
            ],
        };
        let result = expand_pipe("glass-swarm", &expr, &piece).unwrap();
        assert_eq!(result.len(), 3);

        // Check names
        assert_eq!(result[0].0, "glass-swarm-pipe-0");
        assert_eq!(result[1].0, "glass-swarm-pipe-1");
        assert_eq!(result[2].0, "glass-swarm-pipe-2");

        // Check shifted notes
        assert_eq!(result[0].1.notes, Some(vec!["D4".into(), "Eb4".into()]));
        assert_eq!(result[1].1.notes, Some(vec!["F#4".into(), "G4".into()]));
        assert_eq!(result[2].1.notes, Some(vec!["A#4".into(), "B4".into()]));

        // Check pan spread
        assert_eq!(result[0].1.pan, Some(PanPrimitive::Fixed { value: -0.8 }));
        assert_eq!(result[1].1.pan, Some(PanPrimitive::Fixed { value: 0.0 }));
        assert_eq!(result[2].1.pan, Some(PanPrimitive::Fixed { value: 0.8 }));
    }

    // --- Midi to note name ---

    #[test]
    fn midi_to_note_name_middle_c() {
        assert_eq!(midi_to_note_name(60), "C4");
    }

    #[test]
    fn midi_to_note_name_a4() {
        assert_eq!(midi_to_note_name(69), "A4");
    }

    #[test]
    fn midi_to_note_name_sharps() {
        assert_eq!(midi_to_note_name(61), "C#4");
        assert_eq!(midi_to_note_name(63), "D#4");
        assert_eq!(midi_to_note_name(66), "F#4");
    }

    // --- SYNC-03: pipe change produces synth output ---

    #[test]
    fn pipe_change_produces_synth_output() {
        // A pipe: block with a valid source should expand to at least one SynthBlock.
        // This verifies SYNC-03: pipe: changes propagate to synth: output.
        let piece = glass_piece(); // has glass with synth: osc: sine, amp: 0.3, notes: D4, Eb4
        let expr = PipeExpr {
            source: PipeSource::Thing("glass".to_string()),
            transforms: vec![Transform::Replicate { n: 2 }],
        };
        let expanded = expand_pipe("derived", &expr, &piece).unwrap();
        assert!(!expanded.is_empty(), "pipe expansion should produce synth blocks");
        assert_eq!(expanded.len(), 2, "replicate(2) should produce 2 blocks");
        // Each expanded block should carry the source's synth params
        for (_, block) in &expanded {
            assert_eq!(block.osc, Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])));
            assert!(block.amp.is_some());
        }
    }

    // --- Unsupported field accessor ---

    #[test]
    fn unsupported_field_accessor_fails() {
        let piece = glass_piece();
        let expr = PipeExpr {
            source: PipeSource::Field("glass".to_string(), "osc".to_string()),
            transforms: vec![],
        };
        let result = expand_pipe("test", &expr, &piece);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not yet supported"));
    }
}

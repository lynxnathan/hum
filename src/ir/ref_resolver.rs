//! Ref resolution: resolves `ref: thing-name` on ThingDefs and `ref(thing).field`
//! inside synth: notes fields.
//!
//! Two passes:
//! 1. ThingDef-level `ref:` — merges referenced thing's SynthBlock as base
//! 2. `ref(thing).field` inside synth notes — replaces notes with referenced values

use anyhow::{anyhow, Result};
use regex::Regex;

use crate::instruments::InstrumentStore;
use crate::parser::Piece;

/// Resolve all ref: and ref(thing).field references in the piece, mutating in-place.
///
/// Pass 1: For each ThingDef with `reference` set, merge the referenced thing's
/// SynthBlock as base with the local synth as override.
///
/// Pass 2: Scan notes fields for `ref(thing)` and `ref(thing).notes` patterns,
/// resolving them against the piece.
pub fn resolve_refs(piece: &mut Piece) -> Result<()> {
    // Pass 1: ThingDef-level ref resolution
    // Collect ref targets first to avoid borrow issues
    let ref_pairs: Vec<(String, String)> = piece
        .iter()
        .filter_map(|(name, thing)| {
            thing.reference.as_ref().map(|r| (name.clone(), r.clone()))
        })
        .collect();

    for (name, ref_target) in &ref_pairs {
        // Look up the referenced thing
        let ref_thing = piece
            .get(ref_target)
            .ok_or_else(|| anyhow!("ref '{}' not found in piece (referenced by '{}')", ref_target, name))?;

        // Warn if ref target itself has a ref (no chaining in this phase)
        if ref_thing.reference.is_some() {
            tracing::warn!(
                "'{}': ref target '{}' itself has a ref — ref chaining not supported yet",
                name, ref_target
            );
        }

        let ref_synth = ref_thing.synth.clone();

        // Now get the current thing mutably
        let thing = piece.get_mut(name).unwrap();

        match (&ref_synth, &thing.synth) {
            (Some(base), Some(local)) => {
                // Merge: ref's synth is base, local synth overrides
                let merged = InstrumentStore::merge(base, local);
                thing.synth = Some(merged);
            }
            (Some(base), None) => {
                // No local synth, inherit ref's synth entirely
                thing.synth = Some(base.clone());
            }
            (None, _) => {
                tracing::warn!(
                    "'{}': ref target '{}' has no synth block — nothing to inherit",
                    name, ref_target
                );
            }
        }
    }

    // Pass 2: ref(thing) and ref(thing).field inside notes
    let re = Regex::new(r"^ref\(([^)]+)\)(?:\.(\w+))?$").unwrap();

    // Collect thing names that have notes with ref() patterns
    let things_with_ref_notes: Vec<(String, Vec<String>)> = piece
        .iter()
        .filter_map(|(name, thing)| {
            let synth = thing.synth.as_ref()?;
            let notes = synth.notes.as_ref()?;
            // Check if any note matches ref() pattern
            if notes.iter().any(|n| re.is_match(n)) {
                Some((name.clone(), notes.clone()))
            } else {
                None
            }
        })
        .collect();

    for (name, notes) in things_with_ref_notes {
        for note in &notes {
            if let Some(caps) = re.captures(note) {
                let ref_target = caps.get(1).unwrap().as_str();
                let field = caps.get(2).map(|m| m.as_str());

                let ref_thing = piece
                    .get(ref_target)
                    .ok_or_else(|| {
                        anyhow!(
                            "ref('{}') not found in piece (referenced in notes of '{}')",
                            ref_target, name
                        )
                    })?;

                let ref_synth = ref_thing.synth.as_ref().ok_or_else(|| {
                    anyhow!(
                        "ref('{}') has no synth block (referenced in notes of '{}')",
                        ref_target, name
                    )
                })?;

                match field {
                    Some("notes") => {
                        // ref(thing).notes — replace entire notes vec
                        let ref_notes = ref_synth.notes.clone().ok_or_else(|| {
                            anyhow!(
                                "ref('{}').notes: referenced thing has no notes",
                                ref_target
                            )
                        })?;
                        let thing = piece.get_mut(&name).unwrap();
                        if let Some(synth) = thing.synth.as_mut() {
                            synth.notes = Some(ref_notes);
                        }
                    }
                    None => {
                        // ref(thing) — merge entire SynthBlock as base
                        let ref_synth_clone = ref_synth.clone();
                        let thing = piece.get_mut(&name).unwrap();
                        if let Some(local_synth) = &thing.synth {
                            let merged =
                                InstrumentStore::merge(&ref_synth_clone, local_synth);
                            thing.synth = Some(merged);
                        } else {
                            thing.synth = Some(ref_synth_clone);
                        }
                    }
                    Some(other) => {
                        return Err(anyhow!(
                            "ref('{}').{}: field accessor '{}' not supported — only .notes is supported in this version",
                            ref_target, other, other
                        ));
                    }
                }

                // Only process the first ref() pattern found in notes
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::types::{OscPrimitive, OscLayer, SynthBlock, EnvPrimitive, FilterPrimitive, NoiseType, Value};
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

    // --- Pass 1: ThingDef-level ref: ---

    #[test]
    fn ref_inherits_synth_fields() {
        let mut piece: Piece = IndexMap::new();

        let mut glass = make_thing();
        glass.synth = Some(SynthBlock {
            notes: Some(vec!["D4".into(), "Eb4".into()]),
            osc: Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])),
            tempo: Some("0.35".into()),
            ..empty_synth()
        });
        piece.insert("glass".into(), glass);

        let mut glass_drum = make_thing();
        glass_drum.reference = Some("glass".into());
        glass_drum.synth = Some(SynthBlock {
            osc: Some(OscLayer(vec![OscPrimitive::Noise {
                noise_type: NoiseType::White,
            }])),
            env: Some(EnvPrimitive::Perc {
                attack: 0.001,
                release: 0.05,
            }),
            ..empty_synth()
        });
        piece.insert("glass-drum".into(), glass_drum);

        resolve_refs(&mut piece).unwrap();

        let resolved = piece.get("glass-drum").unwrap().synth.as_ref().unwrap();
        // Inherited from glass
        assert_eq!(resolved.notes, Some(vec!["D4".into(), "Eb4".into()]));
        assert_eq!(resolved.tempo, Some("0.35".into()));
        // Local override wins
        assert_eq!(
            resolved.osc,
            Some(OscLayer(vec![OscPrimitive::Noise {
                noise_type: NoiseType::White,
            }]))
        );
        assert_eq!(
            resolved.env,
            Some(EnvPrimitive::Perc {
                attack: 0.001,
                release: 0.05,
            })
        );
    }

    #[test]
    fn ref_missing_target_returns_error() {
        let mut piece: Piece = IndexMap::new();

        let mut thing = make_thing();
        thing.reference = Some("nonexistent".into());
        piece.insert("broken".into(), thing);

        let result = resolve_refs(&mut piece);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("nonexistent"));
        assert!(err_msg.contains("not found"));
    }

    #[test]
    fn ref_local_field_overrides_inherited() {
        let mut piece: Piece = IndexMap::new();

        let mut base = make_thing();
        base.synth = Some(SynthBlock {
            amp: Some(Value::Fixed(0.8)),
            osc: Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])),
            ..empty_synth()
        });
        piece.insert("base-thing".into(), base);

        let mut child = make_thing();
        child.reference = Some("base-thing".into());
        child.synth = Some(SynthBlock {
            amp: Some(Value::Fixed(0.2)), // Local override
            ..empty_synth()
        });
        piece.insert("child-thing".into(), child);

        resolve_refs(&mut piece).unwrap();

        let resolved = piece.get("child-thing").unwrap().synth.as_ref().unwrap();
        assert_eq!(resolved.amp, Some(Value::Fixed(0.2))); // Local wins
        assert_eq!(resolved.osc, Some(OscLayer(vec![OscPrimitive::Sine { freq: None }]))); // Inherited
    }

    #[test]
    fn thing_without_ref_passes_through() {
        let mut piece: Piece = IndexMap::new();

        let mut thing = make_thing();
        thing.synth = Some(SynthBlock {
            osc: Some(OscLayer(vec![OscPrimitive::Saw { detune: None }])),
            ..empty_synth()
        });
        piece.insert("standalone".into(), thing);

        resolve_refs(&mut piece).unwrap();

        let resolved = piece.get("standalone").unwrap().synth.as_ref().unwrap();
        assert_eq!(resolved.osc, Some(OscLayer(vec![OscPrimitive::Saw { detune: None }])));
    }

    // --- Pass 2: ref(thing).field in notes ---

    #[test]
    fn ref_thing_notes_accessor_resolves() {
        let mut piece: Piece = IndexMap::new();

        let mut glass = make_thing();
        glass.synth = Some(SynthBlock {
            notes: Some(vec!["D4".into(), "Eb4".into(), "C#4".into()]),
            osc: Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])),
            ..empty_synth()
        });
        piece.insert("glass".into(), glass);

        let mut drum = make_thing();
        drum.synth = Some(SynthBlock {
            notes: Some(vec!["ref(glass).notes".into()]),
            osc: Some(OscLayer(vec![OscPrimitive::Noise {
                noise_type: NoiseType::White,
            }])),
            ..empty_synth()
        });
        piece.insert("drum".into(), drum);

        resolve_refs(&mut piece).unwrap();

        let resolved = piece.get("drum").unwrap().synth.as_ref().unwrap();
        assert_eq!(
            resolved.notes,
            Some(vec!["D4".into(), "Eb4".into(), "C#4".into()])
        );
        // osc stays local
        assert_eq!(
            resolved.osc,
            Some(OscLayer(vec![OscPrimitive::Noise {
                noise_type: NoiseType::White,
            }]))
        );
    }

    #[test]
    fn ref_thing_no_field_merges_full_synth() {
        let mut piece: Piece = IndexMap::new();

        let mut glass = make_thing();
        glass.synth = Some(SynthBlock {
            notes: Some(vec!["D4".into()]),
            osc: Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])),
            amp: Some(Value::Fixed(0.5)),
            ..empty_synth()
        });
        piece.insert("glass".into(), glass);

        let mut variant = make_thing();
        variant.synth = Some(SynthBlock {
            notes: Some(vec!["ref(glass)".into()]),
            filter: Some(FilterPrimitive::Lpf { cutoff: Value::Fixed(800.0) }),
            ..empty_synth()
        });
        piece.insert("variant".into(), variant);

        resolve_refs(&mut piece).unwrap();

        let resolved = piece.get("variant").unwrap().synth.as_ref().unwrap();
        // Inherited from glass via merge
        assert_eq!(resolved.osc, Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])));
        assert_eq!(resolved.amp, Some(Value::Fixed(0.5)));
        // Local override
        assert_eq!(resolved.filter, Some(FilterPrimitive::Lpf { cutoff: Value::Fixed(800.0) }));
    }

    #[test]
    fn ref_thing_notes_missing_target_errors() {
        let mut piece: Piece = IndexMap::new();

        let mut thing = make_thing();
        thing.synth = Some(SynthBlock {
            notes: Some(vec!["ref(missing).notes".into()]),
            ..empty_synth()
        });
        piece.insert("broken".into(), thing);

        let result = resolve_refs(&mut piece);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing"));
    }

    #[test]
    fn ref_unsupported_field_accessor_errors() {
        let mut piece: Piece = IndexMap::new();

        let mut glass = make_thing();
        glass.synth = Some(SynthBlock {
            osc: Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])),
            ..empty_synth()
        });
        piece.insert("glass".into(), glass);

        let mut thing = make_thing();
        thing.synth = Some(SynthBlock {
            notes: Some(vec!["ref(glass).osc".into()]),
            ..empty_synth()
        });
        piece.insert("broken".into(), thing);

        let result = resolve_refs(&mut piece);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not supported"));
    }
}

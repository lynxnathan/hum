use crate::dict::DictStore;
use crate::ir::types::{FilterPrimitive, FxPrimitive, OscLayer, OscPrimitive, SynthBlock, Value};
use crate::parser::{Piece, ThingDef};

// ---------------------------------------------------------------------------
// Suggest: structural composition hints
// ---------------------------------------------------------------------------

/// Produce structural suggestions for a piece, referencing dict vocabulary
/// when applicable.
pub fn suggest(piece: &Piece, dict: &DictStore) -> Vec<String> {
    let mut hints = Vec::new();

    suggest_shared_fx(piece, &mut hints);
    suggest_timing_gaps(piece, &mut hints);
    suggest_osc_variety(piece, &mut hints);
    suggest_dict_matches(piece, dict, &mut hints);

    if hints.is_empty() {
        hints.push("No suggestions -- your piece looks well-balanced.".to_string());
    }
    hints
}

/// Things that share similar fx profiles could use a shared stage effect.
fn suggest_shared_fx(piece: &Piece, hints: &mut Vec<String>) {
    let things_with_fx: Vec<(&str, &FxPrimitive)> = piece
        .iter()
        .filter_map(|(name, def)| {
            def.synth.as_ref()?.fx.as_ref().map(|fx| (name.as_str(), fx))
        })
        .collect();

    // Group by fx type name (reverb vs delay)
    let mut reverb_things: Vec<&str> = Vec::new();
    let mut delay_things: Vec<&str> = Vec::new();

    for (name, fx) in &things_with_fx {
        match fx {
            FxPrimitive::Reverb { .. } => reverb_things.push(name),
            FxPrimitive::Delay { .. } | FxPrimitive::Allpass { .. } => delay_things.push(name),
        }
    }

    if reverb_things.len() >= 2 {
        hints.push(format!(
            "{} share reverb -- consider a shared stage effect to save CPU and unify the space",
            reverb_things.join(" and ")
        ));
    }
    if delay_things.len() >= 2 {
        hints.push(format!(
            "{} share delay -- consider a shared stage effect for cohesion",
            delay_things.join(" and ")
        ));
    }
}

/// Detect timing gaps where nothing plays.
fn suggest_timing_gaps(piece: &Piece, hints: &mut Vec<String>) {
    // Collect (start, end) intervals for all things
    let mut intervals: Vec<(f64, f64)> = Vec::new();

    for (_name, def) in piece.iter() {
        let start = parse_time_opt(def.at.as_deref()).unwrap_or(0.0);
        let end = parse_time_opt(def.until.as_deref()).unwrap_or(f64::MAX);
        intervals.push((start, end));
    }

    if intervals.is_empty() {
        return;
    }

    intervals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    // Find the latest end time (ignoring infinite ones)
    let max_end = intervals
        .iter()
        .filter(|(_, e)| *e < f64::MAX)
        .map(|(_, e)| *e)
        .fold(0.0f64, f64::max);

    if max_end <= 0.0 {
        return; // all open-ended, no gaps to detect
    }

    // Sweep: find seconds where nothing plays (check every 5s up to max_end)
    let step = 5.0;
    let mut t = 0.0;
    let mut gap_start: Option<f64> = None;

    while t <= max_end {
        let anything_playing = intervals.iter().any(|(s, e)| *s <= t && t < *e);

        if !anything_playing {
            if gap_start.is_none() {
                gap_start = Some(t);
            }
        } else if let Some(gs) = gap_start.take() {
            if t - gs >= 5.0 {
                hints.push(format!(
                    "Nothing plays between {:.0}s and {:.0}s -- consider filling this gap",
                    gs, t
                ));
            }
        }
        t += step;
    }

    // Check trailing gap
    if let Some(gs) = gap_start {
        if max_end - gs >= 5.0 {
            hints.push(format!(
                "Nothing plays between {:.0}s and {:.0}s -- consider filling this gap",
                gs, max_end
            ));
        }
    }
}

/// Detect lack of oscillator variety.
fn suggest_osc_variety(piece: &Piece, hints: &mut Vec<String>) {
    let oscs: Vec<&OscLayer> = piece
        .iter()
        .filter_map(|(_, def)| def.synth.as_ref()?.osc.as_ref())
        .collect();

    if oscs.len() < 2 {
        return; // not enough things with explicit osc to judge
    }

    let all_same = oscs.windows(2).all(|w| osc_layer_type_name(w[0]) == osc_layer_type_name(w[1]));
    if all_same {
        let name = osc_layer_type_name(oscs[0]);
        let alternatives: Vec<&str> = ["sine", "saw", "pulse", "noise"]
            .iter()
            .filter(|&&a| a != name)
            .copied()
            .collect();
        hints.push(format!(
            "All things use {} oscillator -- try {} for variety",
            name,
            alternatives.join(" or ")
        ));
    }
}

/// Check if any thing's synth profile matches a dict entry and suggest naming.
fn suggest_dict_matches(piece: &Piece, dict: &DictStore, hints: &mut Vec<String>) {
    for (thing_name, def) in piece.iter() {
        let synth = match &def.synth {
            Some(s) => s,
            None => continue,
        };

        // Skip things that already have a style: set
        if def.style.is_some() {
            continue;
        }

        for term in dict.all_terms() {
            if let Some(entry) = dict.get(term) {
                if synth_profile_matches(synth, &entry.synth) {
                    hints.push(format!(
                        "'{}' matches '{}' in your dictionary -- consider using style: {}",
                        thing_name, term, term
                    ));
                }
            }
        }
    }
}

/// Rough match: do two synth blocks share the same osc type and/or fx type?
fn synth_profile_matches(a: &SynthBlock, b: &SynthBlock) -> bool {
    // At least one meaningful match required
    let osc_match = match (&a.osc, &b.osc) {
        (Some(ao), Some(bo)) => osc_layer_type_name(ao) == osc_layer_type_name(bo),
        _ => false,
    };
    let fx_match = match (&a.fx, &b.fx) {
        (Some(af), Some(bf)) => fx_type_name(af) == fx_type_name(bf),
        _ => false,
    };
    let filter_match = match (&a.filter, &b.filter) {
        (Some(afl), Some(bfl)) => filter_type_name(afl) == filter_type_name(bfl),
        _ => false,
    };

    osc_match || fx_match || filter_match
}

fn osc_type_name(osc: &OscPrimitive) -> &'static str {
    match osc {
        OscPrimitive::Sine { .. } => "sine",
        OscPrimitive::Saw { .. } => "saw",
        OscPrimitive::Pulse { .. } => "pulse",
        OscPrimitive::Noise { .. } => "noise",
    }
}

/// Get a type name for the primary oscillator in an OscLayer.
fn osc_layer_type_name(layer: &OscLayer) -> &'static str {
    layer.0.first().map(osc_type_name).unwrap_or("none")
}

fn fx_type_name(fx: &FxPrimitive) -> &'static str {
    match fx {
        FxPrimitive::Reverb { .. } => "reverb",
        FxPrimitive::Delay { .. } => "delay",
        FxPrimitive::Allpass { .. } => "allpass",
    }
}

fn filter_type_name(f: &FilterPrimitive) -> &'static str {
    match f {
        FilterPrimitive::Lpf { .. } => "lpf",
        FilterPrimitive::Hpf { .. } => "hpf",
        FilterPrimitive::Bpf { .. } => "bpf",
    }
}

// ---------------------------------------------------------------------------
// Analyze: frequency balance assessment
// ---------------------------------------------------------------------------

/// Frequency band categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Band {
    Sub,       // < 100 Hz
    Bass,      // 100-300 Hz
    LowMid,   // 300-1000 Hz
    Mid,       // 1000-4000 Hz
    Presence,  // 4000-8000 Hz
    Air,       // > 8000 Hz
}

impl Band {
    fn label(&self) -> &'static str {
        match self {
            Band::Sub => "sub (<100Hz)",
            Band::Bass => "bass (100-300Hz)",
            Band::LowMid => "low-mid (300Hz-1kHz)",
            Band::Mid => "mid (1-4kHz)",
            Band::Presence => "presence (4-8kHz)",
            Band::Air => "air (>8kHz)",
        }
    }

    fn all() -> &'static [Band] {
        &[Band::Sub, Band::Bass, Band::LowMid, Band::Mid, Band::Presence, Band::Air]
    }
}

fn freq_to_band(hz: f64) -> Band {
    if hz < 100.0 {
        Band::Sub
    } else if hz < 300.0 {
        Band::Bass
    } else if hz < 1000.0 {
        Band::LowMid
    } else if hz < 4000.0 {
        Band::Mid
    } else if hz < 8000.0 {
        Band::Presence
    } else {
        Band::Air
    }
}

/// Produce a frequency balance assessment for a piece.
pub fn analyze(piece: &Piece) -> Vec<String> {
    let mut band_counts: std::collections::HashMap<Band, Vec<String>> =
        std::collections::HashMap::new();

    for (name, def) in piece.iter() {
        let bands = estimate_thing_bands(name, def);
        for band in bands {
            band_counts
                .entry(band)
                .or_default()
                .push(name.to_string());
        }
    }

    let mut lines = Vec::new();

    // Report populated bands
    let mut populated = Vec::new();
    let mut empty = Vec::new();

    for &band in Band::all() {
        if let Some(things) = band_counts.get(&band) {
            populated.push((band, things.clone()));
        } else {
            empty.push(band);
        }
    }

    // Heavy bands (3+ things)
    for (band, things) in &populated {
        if things.len() >= 3 {
            lines.push(format!(
                "Heavy {} content ({} things: {}) -- consider thinning or EQ separation",
                band.label(),
                things.len(),
                things.join(", ")
            ));
        } else if things.len() >= 2 {
            lines.push(format!(
                "{}: {} ({})",
                band.label(),
                things.len(),
                things.join(", ")
            ));
        } else {
            lines.push(format!("{}: {}", band.label(), things[0]));
        }
    }

    // Missing bands
    if !empty.is_empty() {
        let empty_labels: Vec<&str> = empty.iter().map(|b| b.label()).collect();
        lines.push(format!(
            "No content in {} -- consider adding material in {} range",
            empty_labels.join(", "),
            if empty.len() == 1 { "this" } else { "these" }
        ));
    }

    if lines.is_empty() {
        lines.push("Unable to assess frequency balance -- no synth: blocks found.".to_string());
    }

    lines
}

/// Estimate which frequency bands a thing occupies based on its synth params.
fn estimate_thing_bands(name: &str, def: &ThingDef) -> Vec<Band> {
    let synth = match &def.synth {
        Some(s) => s,
        None => {
            // No synth block -- try to infer from name/like text
            return infer_bands_from_text(name, def);
        }
    };

    let mut bands = Vec::new();

    // Start with osc-implied bands (based on primary oscillator)
    let primary_osc = synth.osc.as_ref().and_then(|l| l.0.first());
    let base_bands = match primary_osc {
        Some(OscPrimitive::Sine { .. }) => vec![Band::LowMid, Band::Mid], // narrow, depends on notes
        Some(OscPrimitive::Saw { .. }) => vec![Band::Bass, Band::LowMid, Band::Mid, Band::Presence], // rich harmonics
        Some(OscPrimitive::Pulse { .. }) => vec![Band::LowMid, Band::Mid, Band::Presence], // odd harmonics
        Some(OscPrimitive::Noise { .. }) => vec![Band::Sub, Band::Bass, Band::LowMid, Band::Mid, Band::Presence, Band::Air], // full spectrum
        None => vec![Band::LowMid, Band::Mid], // default assumption
    };

    bands.extend_from_slice(&base_bands);

    // Filter narrows the range
    if let Some(filter) = &synth.filter {
        match filter {
            FilterPrimitive::Lpf { cutoff } => {
                let top = freq_to_band(cutoff.fixed_or_mid() as f64);
                bands.retain(|b| band_order(b) <= band_order(&top));
            }
            FilterPrimitive::Hpf { cutoff } => {
                let bottom = freq_to_band(cutoff.fixed_or_mid() as f64);
                bands.retain(|b| band_order(b) >= band_order(&bottom));
            }
            FilterPrimitive::Bpf { freq, .. } => {
                let center = freq_to_band(freq.fixed_or_mid() as f64);
                // BPF concentrates energy around center +/- 1 band
                let ord = band_order(&center);
                bands.retain(|b| {
                    let bo = band_order(b);
                    bo >= ord.saturating_sub(1) && bo <= ord + 1
                });
            }
        }
    }

    // Notes can shift fundamental frequency estimate
    if let Some(notes) = &synth.notes {
        if let Some(lowest) = estimate_lowest_note_freq(notes) {
            let note_band = freq_to_band(lowest);
            // Ensure the fundamental's band is included
            if !bands.contains(&note_band) {
                bands.push(note_band);
            }
            // If fundamental is sub/bass, add those
            if lowest < 100.0 && !bands.contains(&Band::Sub) {
                bands.push(Band::Sub);
            }
        }
    }

    bands.sort_by_key(|b| band_order(b));
    bands.dedup();
    bands
}

fn band_order(b: &Band) -> usize {
    match b {
        Band::Sub => 0,
        Band::Bass => 1,
        Band::LowMid => 2,
        Band::Mid => 3,
        Band::Presence => 4,
        Band::Air => 5,
    }
}

/// Try to infer frequency character from the thing's name and like: text.
fn infer_bands_from_text(name: &str, def: &ThingDef) -> Vec<Band> {
    let text = format!(
        "{} {}",
        name,
        def.like.as_deref().unwrap_or("")
    )
    .to_lowercase();

    let mut bands = Vec::new();

    if text.contains("bass") || text.contains("sub") || text.contains("rumble") || text.contains("deep") {
        bands.push(Band::Sub);
        bands.push(Band::Bass);
    }
    if text.contains("rain") || text.contains("noise") || text.contains("white") || text.contains("hiss") {
        bands.push(Band::Mid);
        bands.push(Band::Presence);
        bands.push(Band::Air);
    }
    if text.contains("bright") || text.contains("sharp") || text.contains("laser") || text.contains("glass") {
        bands.push(Band::Mid);
        bands.push(Band::Presence);
    }
    if text.contains("warm") || text.contains("pad") {
        bands.push(Band::LowMid);
        bands.push(Band::Mid);
    }
    if text.contains("buzz") || text.contains("saw") || text.contains("chain") {
        bands.push(Band::LowMid);
        bands.push(Band::Mid);
        bands.push(Band::Presence);
    }

    if bands.is_empty() {
        // Default: mid-range
        bands.push(Band::LowMid);
        bands.push(Band::Mid);
    }

    bands.sort_by_key(|b| band_order(b));
    bands.dedup();
    bands
}

/// Rough MIDI note name to frequency (A4 = 440Hz).
fn estimate_lowest_note_freq(notes: &[String]) -> Option<f64> {
    let mut lowest = f64::MAX;
    for note in notes {
        if note == "-" || note == "~" {
            continue; // rest or tie
        }
        if let Some(freq) = note_to_freq(note) {
            if freq < lowest {
                lowest = freq;
            }
        }
    }
    if lowest < f64::MAX {
        Some(lowest)
    } else {
        None
    }
}

/// Convert note name (e.g. "C4", "Eb3", "F#5") to approximate frequency.
fn note_to_freq(note: &str) -> Option<f64> {
    let note = note.trim();
    if note.is_empty() {
        return None;
    }

    // Parse note letter + optional accidental + octave
    let bytes = note.as_bytes();
    let letter = bytes[0] as char;
    let semitone_base = match letter.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };

    let mut idx = 1;
    let mut accidental = 0i32;
    if idx < bytes.len() {
        match bytes[idx] as char {
            '#' | 's' => {
                accidental = 1;
                idx += 1;
            }
            'b' => {
                accidental = -1;
                idx += 1;
            }
            _ => {}
        }
    }

    let octave: i32 = note[idx..].parse().ok()?;
    let midi = (octave + 1) * 12 + semitone_base + accidental;
    // A4 (MIDI 69) = 440 Hz
    Some(440.0 * 2.0f64.powf((midi as f64 - 69.0) / 12.0))
}

/// Parse optional time string like "10s", "1m30s" to seconds.
fn parse_time_opt(s: Option<&str>) -> Option<f64> {
    let s = s?;
    if let Some(m_pos) = s.find('m') {
        let minutes: f64 = s[..m_pos].parse().ok()?;
        let rest = &s[m_pos + 1..];
        let seconds: f64 = if rest.is_empty() {
            0.0
        } else {
            let stripped = rest.strip_suffix('s').unwrap_or(rest);
            stripped.parse().ok()?
        };
        Some(minutes * 60.0 + seconds)
    } else {
        let stripped = s.strip_suffix('s').unwrap_or(s);
        stripped.parse().ok()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::types::SynthBlock;
    use indexmap::IndexMap;

    fn make_synth(osc: Option<OscPrimitive>, filter: Option<FilterPrimitive>, fx: Option<FxPrimitive>) -> SynthBlock {
        SynthBlock {
            osc: osc.map(|o| OscLayer(vec![o])),
            filter,
            fx,
            notes: None,
            env: None,
            distort: None,
            pan: None,
            amp: None,
            tempo: None,
            sample: None,
            loop_mode: None,
        }
    }

    fn make_thing(synth: Option<SynthBlock>, at: Option<&str>, until: Option<&str>) -> ThingDef {
        ThingDef {
            at: at.map(|s| s.to_string()),
            until: until.map(|s| s.to_string()),
            synth,
            does: None,
            location: None,
            has: None,
            within: None,
            every: None,
            like: None,
            reference: None,
            mood: None,
            thing_type: None,
            instrument: None,
            style: None,
            applies_to: None,
            fx: None,
            pipe: None,
        }
    }

    // -- suggest tests --

    #[test]
    fn suggest_detects_shared_reverb() {
        let mut piece: Piece = IndexMap::new();
        piece.insert("a".into(), make_thing(
            Some(make_synth(Some(OscPrimitive::Sine { freq: None }), None, Some(FxPrimitive::Reverb { mix: Value::Fixed(0.5), room: Value::Fixed(0.8) }))),
            Some("0s"), None,
        ));
        piece.insert("b".into(), make_thing(
            Some(make_synth(Some(OscPrimitive::Saw { detune: None }), None, Some(FxPrimitive::Reverb { mix: Value::Fixed(0.3), room: Value::Fixed(0.5) }))),
            Some("0s"), None,
        ));

        let dict = DictStore::default();
        let hints = suggest(&piece, &dict);
        assert!(
            hints.iter().any(|h| h.contains("shared stage effect")),
            "Expected shared stage hint, got: {:?}",
            hints
        );
    }

    #[test]
    fn suggest_detects_osc_monotony() {
        let mut piece: Piece = IndexMap::new();
        piece.insert("a".into(), make_thing(
            Some(make_synth(Some(OscPrimitive::Sine { freq: None }), None, None)),
            Some("0s"), None,
        ));
        piece.insert("b".into(), make_thing(
            Some(make_synth(Some(OscPrimitive::Sine { freq: None }), None, None)),
            Some("5s"), None,
        ));
        piece.insert("c".into(), make_thing(
            Some(make_synth(Some(OscPrimitive::Sine { freq: None }), None, None)),
            Some("10s"), None,
        ));

        let dict = DictStore::default();
        let hints = suggest(&piece, &dict);
        assert!(
            hints.iter().any(|h| h.contains("All things use sine")),
            "Expected osc variety hint, got: {:?}",
            hints
        );
    }

    #[test]
    fn suggest_detects_dict_match() {
        let mut piece: Piece = IndexMap::new();
        piece.insert("lead".into(), make_thing(
            Some(make_synth(Some(OscPrimitive::Sine { freq: None }), None, None)),
            Some("0s"), None,
        ));

        let dict_yaml = "laser:\n  synth:\n    osc: sine\n  context: bright\n";
        let dir = tempfile::tempdir().unwrap();
        let dict_path = dir.path().join("test.dict");
        std::fs::write(&dict_path, dict_yaml).unwrap();
        let dict = DictStore::load(&dict_path).unwrap();

        let hints = suggest(&piece, &dict);
        assert!(
            hints.iter().any(|h| h.contains("laser") && h.contains("dictionary")),
            "Expected dict match hint, got: {:?}",
            hints
        );
    }

    #[test]
    fn suggest_detects_timing_gap() {
        let mut piece: Piece = IndexMap::new();
        piece.insert("a".into(), make_thing(
            Some(make_synth(Some(OscPrimitive::Sine { freq: None }), None, None)),
            Some("0s"), Some("10s"),
        ));
        // Gap from 10s to 30s
        piece.insert("b".into(), make_thing(
            Some(make_synth(Some(OscPrimitive::Saw { detune: None }), None, None)),
            Some("30s"), Some("60s"),
        ));

        let dict = DictStore::default();
        let hints = suggest(&piece, &dict);
        assert!(
            hints.iter().any(|h| h.contains("Nothing plays between")),
            "Expected timing gap hint, got: {:?}",
            hints
        );
    }

    // -- analyze tests --

    #[test]
    fn analyze_detects_heavy_sub() {
        let mut piece: Piece = IndexMap::new();
        // Three things with notes in sub range
        for name in &["bass1", "bass2", "bass3"] {
            let mut synth = make_synth(Some(OscPrimitive::Saw { detune: None }), None, None);
            synth.notes = Some(vec!["C1".into(), "D1".into()]);
            piece.insert(name.to_string(), make_thing(Some(synth), Some("0s"), None));
        }

        let lines = analyze(&piece);
        assert!(
            lines.iter().any(|l| l.to_lowercase().contains("heavy") || l.to_lowercase().contains("sub")),
            "Expected heavy sub content warning, got: {:?}",
            lines
        );
    }

    #[test]
    fn analyze_detects_missing_bands() {
        let mut piece: Piece = IndexMap::new();
        // Only things with LPF cutting above 500Hz
        piece.insert("pad".into(), make_thing(
            Some(make_synth(
                Some(OscPrimitive::Sine { freq: None }),
                Some(FilterPrimitive::Lpf { cutoff: Value::Fixed(500.0) }),
                None,
            )),
            Some("0s"), None,
        ));

        let lines = analyze(&piece);
        assert!(
            lines.iter().any(|l| l.contains("No content in")),
            "Expected missing band warning, got: {:?}",
            lines
        );
    }

    // -- unit tests for helpers --

    #[test]
    fn note_to_freq_a4_is_440() {
        let freq = note_to_freq("A4").unwrap();
        assert!((freq - 440.0).abs() < 1.0, "A4 should be ~440Hz, got {}", freq);
    }

    #[test]
    fn note_to_freq_c1_is_sub() {
        let freq = note_to_freq("C1").unwrap();
        assert!(freq < 100.0, "C1 should be sub-100Hz, got {}", freq);
    }

    #[test]
    fn parse_time_opt_various() {
        assert_eq!(parse_time_opt(Some("10s")), Some(10.0));
        assert_eq!(parse_time_opt(Some("1m30s")), Some(90.0));
        assert_eq!(parse_time_opt(Some("0s")), Some(0.0));
        assert_eq!(parse_time_opt(None), None);
    }
}

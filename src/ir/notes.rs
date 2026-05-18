/// Convert a note name (e.g. "D4", "Eb4", "C#3") to a MIDI number.
/// Returns None for rests ("-") or invalid note names.
/// Convention: C4 = 60 (middle C), A4 = 69.
pub fn note_to_midi(note: &str) -> Option<u8> {
    if note == "-" {
        return None; // rest
    }

    let bytes = note.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    let pitch_class: i8 = match bytes[0] {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return None,
    };

    let (accidental, oct_start): (i8, usize) = match bytes.get(1) {
        Some(b'#') => (1, 2),
        Some(b'b') => (-1, 2),
        _ => (0, 1),
    };

    let octave: i8 = note[oct_start..].parse().ok()?;
    // MIDI = (octave + 1) * 12 + pitch_class + accidental
    let midi = (octave + 1) as i16 * 12 + pitch_class as i16 + accidental as i16;
    if midi < 0 || midi > 127 {
        return None;
    }
    Some(midi as u8)
}

/// Convert a MIDI note number to frequency in Hz (equal temperament, A4=440).
pub fn midi_to_freq(midi: u8) -> f32 {
    440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0)
}

/// Convert a list of note name strings to MIDI numbers.
/// Rests ("-") become None.
pub fn parse_note_list(notes: &[String]) -> Vec<Option<u8>> {
    notes.iter().map(|n| note_to_midi(n)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_to_midi_c4_is_60() {
        assert_eq!(note_to_midi("C4"), Some(60));
    }

    #[test]
    fn note_to_midi_a4_is_69() {
        assert_eq!(note_to_midi("A4"), Some(69));
    }

    #[test]
    fn note_to_midi_d4_is_62() {
        assert_eq!(note_to_midi("D4"), Some(62));
    }

    #[test]
    fn note_to_midi_eb4_is_63() {
        assert_eq!(note_to_midi("Eb4"), Some(63));
    }

    #[test]
    fn note_to_midi_c_sharp_4_is_61() {
        assert_eq!(note_to_midi("C#4"), Some(61));
    }

    #[test]
    fn note_to_midi_rest_is_none() {
        assert_eq!(note_to_midi("-"), None);
    }

    #[test]
    fn note_to_midi_c0_is_12() {
        assert_eq!(note_to_midi("C0"), Some(12));
    }

    #[test]
    fn note_to_midi_invalid_returns_none() {
        assert_eq!(note_to_midi("X4"), None);
        assert_eq!(note_to_midi(""), None);
    }

    #[test]
    fn midi_to_freq_a4_is_440() {
        let freq = midi_to_freq(69);
        assert!((freq - 440.0).abs() < 0.01, "Expected ~440.0, got {}", freq);
    }

    #[test]
    fn midi_to_freq_c4() {
        let freq = midi_to_freq(60);
        // C4 = 261.63 Hz
        assert!(
            (freq - 261.63).abs() < 0.1,
            "Expected ~261.63, got {}",
            freq
        );
    }

    #[test]
    fn parse_note_list_mixed() {
        let notes: Vec<String> = vec!["D4".into(), "Eb4".into(), "-".into()];
        let result = parse_note_list(&notes);
        assert_eq!(result, vec![Some(62), Some(63), None]);
    }
}

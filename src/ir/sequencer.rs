/// Note sequencer — drives freq changes on running synth nodes at tempo intervals.
///
/// Each NoteSequencer spawns a tokio task that loops through a note list,
/// sending SequencerEvent::SetFreq messages at the configured tempo.
/// Rests ("-") are silent — no freq change is sent.

use tokio::sync::mpsc::Sender;

use super::notes::note_to_midi;

/// A note sequencer for a single thing.
pub struct NoteSequencer {
    pub thing_name: String,
    pub notes: Vec<String>,
    pub tempo: f64, // seconds per note
}

impl NoteSequencer {
    /// Parse tempo string like "0.35s/note" into seconds per note.
    /// Returns None if the format is unrecognized.
    pub fn parse_tempo(s: &str) -> Option<f64> {
        let s = s.trim();
        // Format: "0.35s/note" or "0.35"
        let num_str = s
            .strip_suffix("s/note")
            .or_else(|| s.strip_suffix('s'))
            .unwrap_or(s);
        num_str.parse::<f64>().ok()
    }

    /// Spawn a tokio task that sends SetFreq events at tempo intervals.
    /// Returns the JoinHandle so it can be aborted on stop/hot-swap.
    pub fn spawn(&self, tx: Sender<SequencerEvent>) -> tokio::task::JoinHandle<()> {
        let notes = self.notes.clone();
        let tempo = self.tempo;
        let thing_name = self.thing_name.clone();

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_secs_f64(tempo));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            let mut idx = 0;
            loop {
                interval.tick().await;
                let note = &notes[idx % notes.len()];
                if note != "-" {
                    if let Some(midi) = note_to_midi(note) {
                        let freq = 440.0 * 2.0_f64.powf((midi as f64 - 69.0) / 12.0);
                        let _ = tx
                            .send(SequencerEvent::SetFreq {
                                thing_name: thing_name.clone(),
                                freq: freq as f32,
                            })
                            .await;
                    }
                }
                idx += 1;
            }
        })
    }
}

/// Events emitted by note sequencers, consumed by the main event loop.
#[derive(Debug)]
pub enum SequencerEvent {
    SetFreq { thing_name: String, freq: f32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tempo_with_suffix() {
        let t = NoteSequencer::parse_tempo("0.35s/note").unwrap();
        assert!((t - 0.35).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_tempo_bare_seconds() {
        let t = NoteSequencer::parse_tempo("0.5s").unwrap();
        assert!((t - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_tempo_bare_number() {
        let t = NoteSequencer::parse_tempo("0.25").unwrap();
        assert!((t - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_tempo_invalid() {
        assert!(NoteSequencer::parse_tempo("fast").is_none());
    }
}

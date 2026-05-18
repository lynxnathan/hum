/// Spectral flux onset (beat) detection from FFT bins.
///
/// Call `update()` once per FFT frame (~30fps). It computes the positive
/// spectral flux, compares against a rolling mean, and outputs a `beat_energy`
/// value in 0..1 that spikes to 1.0 on transients and decays smoothly.

pub struct BeatDetector {
    prev_bins: [f32; 64],
    energy_history: [f32; 43], // ~1.4s at 30fps circular buffer
    history_idx: usize,
    pub beat_energy: f32,
}

impl BeatDetector {
    pub fn new() -> Self {
        BeatDetector {
            prev_bins: [0.0; 64],
            energy_history: [0.0001; 43],
            history_idx: 0,
            beat_energy: 0.0,
        }
    }

    /// Feed a new FFT frame. Returns current beat_energy (0..1).
    pub fn update(&mut self, bins: &[f32; 64]) -> f32 {
        // Spectral flux: sum of positive bin differences (onset = energy increase)
        let flux: f32 = bins
            .iter()
            .zip(self.prev_bins.iter())
            .map(|(cur, prev)| (cur - prev).max(0.0))
            .sum();

        // Rolling mean of recent fluxes
        let mean = self.energy_history.iter().sum::<f32>() / self.energy_history.len() as f32;

        // Onset if flux exceeds adaptive threshold
        if flux > mean * 1.5 && flux > 0.01 {
            self.beat_energy = 1.0_f32.max(self.beat_energy);
        } else {
            self.beat_energy *= 0.82; // decay: ~200ms at 30fps
        }

        // Clamp floor to zero (avoid denormals)
        if self.beat_energy < 0.001 {
            self.beat_energy = 0.0;
        }

        // Update circular buffer and store previous bins
        self.energy_history[self.history_idx] = flux.max(0.0001);
        self.history_idx = (self.history_idx + 1) % self.energy_history.len();
        self.prev_bins.copy_from_slice(bins);

        self.beat_energy
    }
}

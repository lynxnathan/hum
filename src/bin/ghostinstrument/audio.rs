//! Audio pipeline for ghostinstrument.
//! Provides AudioParams, build_graph, build_stream, and init_audio_async.
//! Phase 3: stereo pan + proximity blend + one-pole smoothing.

use crossbeam_channel::bounded;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use fundsp::prelude32::*;

/// Shared audio parameters — written by UI thread, read by audio callback.
/// Uses fundsp::Shared (AtomicF32 wrapper) — no mutex, RT-safe.
pub struct AudioParams {
    pub pan_a: Shared,
    pub pan_b: Shared,
    pub blend: Shared,
}

impl AudioParams {
    pub fn new() -> Self {
        Self {
            pan_a: Shared::new(0.3),   // initial X of node A
            pan_b: Shared::new(0.7),   // initial X of node B
            blend: Shared::new(0.0),   // initial: far apart, no blend
        }
    }
}

impl Default for AudioParams {
    fn default() -> Self {
        Self::new()
    }
}

/// Build two sine oscillator DSP graphs at the given sample rate.
pub fn build_graph(
    _params: &AudioParams,
    sample_rate: u32,
) -> (Box<dyn AudioUnit + Send>, Box<dyn AudioUnit + Send>) {
    let mut osc_a = Box::new(sine_hz(440.0_f32)) as Box<dyn AudioUnit + Send>;
    let mut osc_b = Box::new(sine_hz(660.0_f32)) as Box<dyn AudioUnit + Send>;
    osc_a.set_sample_rate(sample_rate as f64);
    osc_b.set_sample_rate(sample_rate as f64);
    osc_a.reset();
    osc_b.reset();
    (osc_a, osc_b)
}

/// One-pole smoothing coefficient (~7ms at 48kHz).
const SMOOTH_COEFF: f32 = 0.995;

/// Build a cpal output stream with stereo panning and proximity blending.
/// The callback is allocation-free: no Vec, String, Box::new, or Arc::clone.
pub fn build_stream(
    device: cpal::Device,
    config: cpal::SupportedStreamConfig,
    mut graph_a: Box<dyn AudioUnit + Send>,
    mut graph_b: Box<dyn AudioUnit + Send>,
    pan_a: Shared,
    pan_b: Shared,
    blend: Shared,
) -> cpal::Stream {
    let stream_config: cpal::StreamConfig = config.into();

    // Smoothed state — lives in the callback closure, no heap allocation
    let mut smooth_pan_a: f32 = 0.3;
    let mut smooth_pan_b: f32 = 0.7;
    let mut smooth_blend: f32 = 0.0;

    device
        .build_output_stream(
            &stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // Read target values from atomics (lock-free)
                let target_pan_a = pan_a.value();
                let target_pan_b = pan_b.value();
                let target_blend = blend.value();

                let frames = data.len() / 2;
                for i in 0..frames {
                    // One-pole smoothing per sample
                    smooth_pan_a = smooth_pan_a * SMOOTH_COEFF + target_pan_a * (1.0 - SMOOTH_COEFF);
                    smooth_pan_b = smooth_pan_b * SMOOTH_COEFF + target_pan_b * (1.0 - SMOOTH_COEFF);
                    smooth_blend = smooth_blend * SMOOTH_COEFF + target_blend * (1.0 - SMOOTH_COEFF);

                    // Get mono samples from oscillators
                    let sa = graph_a.get_mono() * 0.4;
                    let sb = graph_b.get_mono() * 0.4;

                    // Equal-power pan for each node
                    let angle_a = smooth_pan_a * std::f32::consts::FRAC_PI_2;
                    let (la, ra) = (angle_a.cos(), angle_a.sin());
                    let angle_b = smooth_pan_b * std::f32::consts::FRAC_PI_2;
                    let (lb, rb) = (angle_b.cos(), angle_b.sin());

                    // Proximity blend: 0 = isolated, 1 = fully blended
                    // When blended, both signals go to both channels equally
                    let iso = 1.0 - smooth_blend;

                    // Left channel: each node's panned contribution + blended contribution
                    let left = sa * (la * iso + 0.707 * smooth_blend)
                             + sb * (lb * iso + 0.707 * smooth_blend);

                    // Right channel
                    let right = sa * (ra * iso + 0.707 * smooth_blend)
                              + sb * (rb * iso + 0.707 * smooth_blend);

                    data[i * 2] = left;
                    data[i * 2 + 1] = right;
                }
            },
            |err| eprintln!("audio stream error: {err}"),
            None,
        )
        .expect("failed to build output stream")
}

/// Initialize audio asynchronously: discover device on a spawned thread,
/// build stream on the caller thread.
pub fn init_audio_async() -> (cpal::Stream, AudioParams) {
    let (tx, rx) = bounded(1);

    std::thread::spawn(move || {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("no output device found");
        let config = device
            .default_output_config()
            .expect("could not get default output config");
        tx.send((device, config)).unwrap();
    });

    let (device, config) = rx.recv().unwrap();
    let sample_rate = config.sample_rate();

    let params = AudioParams::new();
    let (graph_a, graph_b) = build_graph(&params, sample_rate);

    // Clone Shared handles for the callback (cheap — atomic reference)
    let pan_a = params.pan_a.clone();
    let pan_b = params.pan_b.clone();
    let blend = params.blend.clone();

    let stream = build_stream(device, config, graph_a, graph_b, pan_a, pan_b, blend);
    stream.play().expect("failed to start audio stream");
    (stream, params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_accepts_sample_rate() {
        let params = AudioParams::new();
        let (mut osc_a, mut osc_b) = build_graph(&params, 48000);
        let _ = osc_a.get_mono();
        let _ = osc_b.get_mono();
    }

    #[test]
    fn test_graph_accepts_44100() {
        let params = AudioParams::new();
        let (mut osc_a, mut osc_b) = build_graph(&params, 44100);
        let _ = osc_a.get_mono();
        let _ = osc_b.get_mono();
    }

    #[test]
    fn test_audio_params_default_pan() {
        let params = AudioParams::new();
        assert_eq!(params.pan_a.value(), 0.3f32);
    }

    #[test]
    fn test_audio_params_default_pan_b() {
        let params = AudioParams::new();
        assert_eq!(params.pan_b.value(), 0.7f32);
    }

    #[test]
    fn test_audio_params_has_blend() {
        let params = AudioParams::new();
        assert_eq!(params.blend.value(), 0.0f32);
    }
}

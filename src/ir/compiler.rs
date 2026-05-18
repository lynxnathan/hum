/// IR Compiler — converts a SynthBlock into SCgf v2 binary bytes.
///
/// Builds a UGen graph in topological order following the fixed signal chain:
/// Control -> osc -> [filter] -> [env*signal] -> [distort] -> [fx] -> amp*signal -> [Pan2] -> Out
///
/// Then calls encode_synthdef() to produce the binary.

use anyhow::{bail, Result};

use super::encoder::{encode_synthdef, InputSpec, UgenGraph, UgenSpec};
use super::types::*;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Compile a SynthBlock into SCgf v2 binary bytes.
///
/// The `name` parameter becomes the SynthDef name in the binary header.
/// When `sample:` is present, compiles a PlayBuf-based SynthDef (buffer playback).
/// Otherwise requires `osc:` for oscillator-based synthesis.
pub fn compile_synth_block(name: &str, block: &SynthBlock) -> Result<Vec<u8>> {
    // Sample mode: PlayBuf-based SynthDef
    if block.sample.is_some() {
        return compile_sample_block(name, block);
    }

    let osc_layer = block
        .osc
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("synth: block requires osc: or sample:"))?;

    let mut gb = GraphBuilder::new();

    // ------------------------------------------------------------------
    // 1. Determine parameters
    // ------------------------------------------------------------------
    let needs_gate = matches!(block.env, Some(EnvPrimitive::Adsr { .. }));

    let amp_default = match &block.amp {
        Some(v) => v.fixed_or_mid(),
        None => 0.3,
    };

    gb.add_param("freq", 440.0);
    gb.add_param("amp", amp_default);
    if needs_gate {
        gb.add_param("gate", 1.0);
    }

    // Control UGen — one output per parameter, rate = ir (0)
    let num_params = gb.graph.params.len();
    let control_idx = gb.add_ugen(UgenSpec {
        class_name: "Control".to_string(),
        calc_rate: 0, // ir
        special_index: 0,
        inputs: vec![],
        outputs: vec![0; num_params], // ir rate for each output
    });

    let freq_output = InputSpec::UgenOutput {
        ugen: control_idx,
        output: 0,
    };
    let amp_output = InputSpec::UgenOutput {
        ugen: control_idx,
        output: 1,
    };

    // ------------------------------------------------------------------
    // 2. Oscillator(s)
    // ------------------------------------------------------------------
    let osc_idx = compile_osc_layer(&mut gb, osc_layer, &freq_output)?;

    let mut current_signal = InputSpec::UgenOutput {
        ugen: osc_idx,
        output: 0,
    };

    // ------------------------------------------------------------------
    // 3. Filter (optional)
    // ------------------------------------------------------------------
    if let Some(filter) = &block.filter {
        let filter_idx = compile_filter(&mut gb, filter, &current_signal);
        current_signal = InputSpec::UgenOutput {
            ugen: filter_idx,
            output: 0,
        };
    }

    // ------------------------------------------------------------------
    // 4. Envelope (optional) — EnvGen * signal
    // ------------------------------------------------------------------
    if let Some(env) = &block.env {
        let envgen_idx = match env {
            EnvPrimitive::Perc { attack, release } => {
                let env_consts: Vec<f32> = vec![
                    0.0, 2.0, -1.0, -1.0,
                    1.0, *attack, 5.0, -4.0,
                    0.0, *release, 5.0, -4.0,
                ];
                let env_const_indices: Vec<InputSpec> = env_consts
                    .iter()
                    .map(|&v| InputSpec::Constant(gb.add_constant(v)))
                    .collect();

                let gate_ci = gb.add_constant(1.0);
                let level_scale_ci = gb.add_constant(1.0);
                let level_bias_ci = gb.add_constant(0.0);
                let time_scale_ci = gb.add_constant(1.0);
                let done_action_ci = gb.add_constant(2.0);

                let mut inputs = vec![InputSpec::Constant(gate_ci)];
                inputs.push(InputSpec::Constant(level_scale_ci));
                inputs.push(InputSpec::Constant(level_bias_ci));
                inputs.push(InputSpec::Constant(time_scale_ci));
                inputs.push(InputSpec::Constant(done_action_ci));
                inputs.extend(env_const_indices);

                gb.add_ugen(UgenSpec {
                    class_name: "EnvGen".to_string(),
                    calc_rate: 2,
                    special_index: 0,
                    inputs,
                    outputs: vec![2],
                })
            }
            EnvPrimitive::Adsr {
                attack,
                decay,
                sustain,
                release,
            } => {
                let env_consts: Vec<f32> = vec![
                    0.0, 3.0, 2.0, -1.0,
                    1.0, *attack, 5.0, -4.0,
                    *sustain, *decay, 5.0, -4.0,
                    0.0, *release, 5.0, -4.0,
                ];
                let env_const_indices: Vec<InputSpec> = env_consts
                    .iter()
                    .map(|&v| InputSpec::Constant(gb.add_constant(v)))
                    .collect();

                let gate_param_idx = gb.param_index("gate").expect("gate param must exist");
                let gate_input = InputSpec::UgenOutput {
                    ugen: control_idx,
                    output: gate_param_idx,
                };
                let level_scale_ci = gb.add_constant(1.0);
                let level_bias_ci = gb.add_constant(0.0);
                let time_scale_ci = gb.add_constant(1.0);
                let done_action_ci = gb.add_constant(2.0);

                let mut inputs = vec![gate_input];
                inputs.push(InputSpec::Constant(level_scale_ci));
                inputs.push(InputSpec::Constant(level_bias_ci));
                inputs.push(InputSpec::Constant(time_scale_ci));
                inputs.push(InputSpec::Constant(done_action_ci));
                inputs.extend(env_const_indices);

                gb.add_ugen(UgenSpec {
                    class_name: "EnvGen".to_string(),
                    calc_rate: 2,
                    special_index: 0,
                    inputs,
                    outputs: vec![2],
                })
            }
        };

        // Multiply signal by envelope
        let env_output = InputSpec::UgenOutput {
            ugen: envgen_idx,
            output: 0,
        };
        let mul_idx = gb.add_ugen(UgenSpec {
            class_name: "BinaryOpUGen".to_string(),
            calc_rate: 2,
            special_index: 2, // multiply
            inputs: vec![current_signal.clone(), env_output],
            outputs: vec![2],
        });
        current_signal = InputSpec::UgenOutput {
            ugen: mul_idx,
            output: 0,
        };
    }

    // ------------------------------------------------------------------
    // 5. Distortion (optional)
    // ------------------------------------------------------------------
    if let Some(distort) = &block.distort {
        match distort {
            DistortPrimitive::Tanh { drive } => {
                // Multiply by drive first
                let drive_input = compile_value(&mut gb, drive, 1);
                let drive_mul_idx = gb.add_ugen(UgenSpec {
                    class_name: "BinaryOpUGen".to_string(),
                    calc_rate: 2,
                    special_index: 2, // multiply
                    inputs: vec![current_signal.clone(), drive_input],
                    outputs: vec![2],
                });
                // Then tanh (UnaryOpUGen special_index=17)
                let tanh_idx = gb.add_ugen(UgenSpec {
                    class_name: "UnaryOpUGen".to_string(),
                    calc_rate: 2,
                    special_index: 17, // tanh
                    inputs: vec![InputSpec::UgenOutput {
                        ugen: drive_mul_idx,
                        output: 0,
                    }],
                    outputs: vec![2],
                });
                current_signal = InputSpec::UgenOutput {
                    ugen: tanh_idx,
                    output: 0,
                };
            }
            DistortPrimitive::Bitcrush { bits } => {
                let step = 2f32.powi(1 - *bits as i32);
                let step_ci = gb.add_constant(step);
                let round_idx = gb.add_ugen(UgenSpec {
                    class_name: "BinaryOpUGen".to_string(),
                    calc_rate: 2,
                    special_index: 12, // round
                    inputs: vec![current_signal.clone(), InputSpec::Constant(step_ci)],
                    outputs: vec![2],
                });
                current_signal = InputSpec::UgenOutput {
                    ugen: round_idx,
                    output: 0,
                };
            }
        }
    }

    // ------------------------------------------------------------------
    // 6. FX (optional)
    // ------------------------------------------------------------------
    if let Some(fx) = &block.fx {
        let fx_idx = compile_fx(&mut gb, fx, &current_signal);
        current_signal = InputSpec::UgenOutput {
            ugen: fx_idx,
            output: 0,
        };
    }

    // ------------------------------------------------------------------
    // 7. Amplitude multiply
    // ------------------------------------------------------------------
    let amp_mul_idx = gb.add_ugen(UgenSpec {
        class_name: "BinaryOpUGen".to_string(),
        calc_rate: 2,
        special_index: 2, // multiply
        inputs: vec![current_signal.clone(), amp_output],
        outputs: vec![2],
    });
    current_signal = InputSpec::UgenOutput {
        ugen: amp_mul_idx,
        output: 0,
    };

    // ------------------------------------------------------------------
    // 8. Pan (optional) + Out
    // ------------------------------------------------------------------
    let bus_ci = gb.add_constant(0.0);

    if let Some(pan) = &block.pan {
        let pan_pos = compile_pan_pos(&mut gb, pan);

        // Pan2.ar: mono -> stereo
        let pan2_idx = gb.add_ugen(UgenSpec {
            class_name: "Pan2".to_string(),
            calc_rate: 2,
            special_index: 0,
            inputs: vec![current_signal, pan_pos],
            outputs: vec![2, 2], // two ar outputs (L, R)
        });

        // Out.ar with 2 channels
        gb.add_ugen(UgenSpec {
            class_name: "Out".to_string(),
            calc_rate: 2,
            special_index: 0,
            inputs: vec![
                InputSpec::Constant(bus_ci),
                InputSpec::UgenOutput {
                    ugen: pan2_idx,
                    output: 0,
                },
                InputSpec::UgenOutput {
                    ugen: pan2_idx,
                    output: 1,
                },
            ],
            outputs: vec![],
        });
    } else {
        // Mono output (no pan)
        gb.add_ugen(UgenSpec {
            class_name: "Out".to_string(),
            calc_rate: 2,
            special_index: 0,
            inputs: vec![InputSpec::Constant(bus_ci), current_signal],
            outputs: vec![],
        });
    }

    Ok(encode_synthdef(name, &gb.graph))
}

// ---------------------------------------------------------------------------
// Value compiler — produces either a constant or an LFNoise1-modulated range
// ---------------------------------------------------------------------------

/// Compile a Value to an InputSpec.
///
/// - `Fixed(x)` → constant
/// - `Range(lo, hi)` → LFNoise1.kr(0.1) scaled to [lo, hi]
///
/// `rate`: 1 = kr, 2 = ar (for the resulting signal)
fn compile_value(gb: &mut GraphBuilder, value: &Value, rate: u8) -> InputSpec {
    match value {
        Value::Fixed(v) => {
            let ci = gb.add_constant(*v);
            InputSpec::Constant(ci)
        }
        Value::Range(lo, hi) => {
            // LFNoise1 at control rate with slow modulation
            let noise_rate_ci = gb.add_constant(0.1);
            let lfnoise_idx = gb.add_ugen(UgenSpec {
                class_name: "LFNoise1".to_string(),
                calc_rate: 1, // kr
                special_index: 0,
                inputs: vec![InputSpec::Constant(noise_rate_ci)],
                outputs: vec![1],
            });
            // Scale from [-1, 1] to [lo, hi]
            let scale = (hi - lo) / 2.0;
            let offset = (hi + lo) / 2.0;
            let scale_ci = gb.add_constant(scale);
            let offset_ci = gb.add_constant(offset);
            // multiply by scale
            let mul_idx = gb.add_ugen(UgenSpec {
                class_name: "BinaryOpUGen".to_string(),
                calc_rate: rate,
                special_index: 2, // multiply
                inputs: vec![
                    InputSpec::UgenOutput {
                        ugen: lfnoise_idx,
                        output: 0,
                    },
                    InputSpec::Constant(scale_ci),
                ],
                outputs: vec![rate],
            });
            // add offset
            let add_idx = gb.add_ugen(UgenSpec {
                class_name: "BinaryOpUGen".to_string(),
                calc_rate: rate,
                special_index: 0, // add
                inputs: vec![
                    InputSpec::UgenOutput {
                        ugen: mul_idx,
                        output: 0,
                    },
                    InputSpec::Constant(offset_ci),
                ],
                outputs: vec![rate],
            });
            InputSpec::UgenOutput {
                ugen: add_idx,
                output: 0,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Oscillator compiler
// ---------------------------------------------------------------------------

/// Compile an OscLayer (one or more oscillators) and return the index of the
/// final summed signal UGen.
fn compile_osc_layer(
    gb: &mut GraphBuilder,
    layer: &OscLayer,
    freq_output: &InputSpec,
) -> Result<usize> {
    if layer.0.is_empty() {
        bail!("empty oscillator layer");
    }

    let mut osc_indices: Vec<usize> = Vec::new();

    for osc in &layer.0 {
        let idx = compile_single_osc(gb, osc, freq_output)?;
        osc_indices.push(idx);
    }

    if osc_indices.len() == 1 {
        return Ok(osc_indices[0]);
    }

    // Sum all oscillators using BinaryOpUGen(add)
    let mut sum_idx = osc_indices[0];
    for &next_idx in &osc_indices[1..] {
        sum_idx = gb.add_ugen(UgenSpec {
            class_name: "BinaryOpUGen".to_string(),
            calc_rate: 2,
            special_index: 0, // add
            inputs: vec![
                InputSpec::UgenOutput {
                    ugen: sum_idx,
                    output: 0,
                },
                InputSpec::UgenOutput {
                    ugen: next_idx,
                    output: 0,
                },
            ],
            outputs: vec![2],
        });
    }

    Ok(sum_idx)
}

/// Compile a single oscillator primitive, handling detune by creating a second
/// oscillator and summing.
fn compile_single_osc(
    gb: &mut GraphBuilder,
    osc: &OscPrimitive,
    freq_output: &InputSpec,
) -> Result<usize> {
    match osc {
        OscPrimitive::Sine { freq } => {
            let phase_ci = gb.add_constant(0.0);
            let osc_freq = match freq {
                Some(v) => compile_value(gb, v, 2),
                None => freq_output.clone(),
            };
            Ok(gb.add_ugen(UgenSpec {
                class_name: "SinOsc".to_string(),
                calc_rate: 2,
                special_index: 0,
                inputs: vec![osc_freq, InputSpec::Constant(phase_ci)],
                outputs: vec![2],
            }))
        }
        OscPrimitive::Saw { detune } => {
            let main_idx = gb.add_ugen(UgenSpec {
                class_name: "Saw".to_string(),
                calc_rate: 2,
                special_index: 0,
                inputs: vec![freq_output.clone()],
                outputs: vec![2],
            });
            match detune {
                Some(d) => Ok(compile_detuned_pair(gb, "Saw", main_idx, freq_output, d)?),
                None => Ok(main_idx),
            }
        }
        OscPrimitive::Pulse { width, detune } => {
            let width_input = compile_value(gb, width, 2);
            let main_idx = gb.add_ugen(UgenSpec {
                class_name: "Pulse".to_string(),
                calc_rate: 2,
                special_index: 0,
                inputs: vec![freq_output.clone(), width_input.clone()],
                outputs: vec![2],
            });
            match detune {
                Some(d) => Ok(compile_detuned_pair_pulse(gb, main_idx, freq_output, &width_input, d)?),
                None => Ok(main_idx),
            }
        }
        OscPrimitive::Noise { noise_type } => {
            let class = match noise_type {
                NoiseType::White => "WhiteNoise",
                NoiseType::Pink => "PinkNoise",
                NoiseType::Brown => "BrownNoise",
            };
            Ok(gb.add_ugen(UgenSpec {
                class_name: class.to_string(),
                calc_rate: 2,
                special_index: 0,
                inputs: vec![],
                outputs: vec![2],
            }))
        }
    }
}

/// Create a detuned pair: main osc + second osc at freq*(1+detune), summed.
fn compile_detuned_pair(
    gb: &mut GraphBuilder,
    class: &str,
    main_idx: usize,
    freq_output: &InputSpec,
    detune: &Value,
) -> Result<usize> {
    // freq2 = freq * (1 + detune)
    let one_ci = gb.add_constant(1.0);
    let detune_input = compile_value(gb, detune, 1);
    let one_plus_detune = gb.add_ugen(UgenSpec {
        class_name: "BinaryOpUGen".to_string(),
        calc_rate: 1,
        special_index: 0, // add
        inputs: vec![InputSpec::Constant(one_ci), detune_input],
        outputs: vec![1],
    });
    let freq2 = gb.add_ugen(UgenSpec {
        class_name: "BinaryOpUGen".to_string(),
        calc_rate: 2,
        special_index: 2, // multiply
        inputs: vec![
            freq_output.clone(),
            InputSpec::UgenOutput { ugen: one_plus_detune, output: 0 },
        ],
        outputs: vec![2],
    });
    let second_idx = gb.add_ugen(UgenSpec {
        class_name: class.to_string(),
        calc_rate: 2,
        special_index: 0,
        inputs: vec![InputSpec::UgenOutput { ugen: freq2, output: 0 }],
        outputs: vec![2],
    });
    // Sum main + detuned
    Ok(gb.add_ugen(UgenSpec {
        class_name: "BinaryOpUGen".to_string(),
        calc_rate: 2,
        special_index: 0, // add
        inputs: vec![
            InputSpec::UgenOutput { ugen: main_idx, output: 0 },
            InputSpec::UgenOutput { ugen: second_idx, output: 0 },
        ],
        outputs: vec![2],
    }))
}

/// Create a detuned Pulse pair with width.
fn compile_detuned_pair_pulse(
    gb: &mut GraphBuilder,
    main_idx: usize,
    freq_output: &InputSpec,
    width_input: &InputSpec,
    detune: &Value,
) -> Result<usize> {
    let one_ci = gb.add_constant(1.0);
    let detune_input = compile_value(gb, detune, 1);
    let one_plus_detune = gb.add_ugen(UgenSpec {
        class_name: "BinaryOpUGen".to_string(),
        calc_rate: 1,
        special_index: 0,
        inputs: vec![InputSpec::Constant(one_ci), detune_input],
        outputs: vec![1],
    });
    let freq2 = gb.add_ugen(UgenSpec {
        class_name: "BinaryOpUGen".to_string(),
        calc_rate: 2,
        special_index: 2,
        inputs: vec![
            freq_output.clone(),
            InputSpec::UgenOutput { ugen: one_plus_detune, output: 0 },
        ],
        outputs: vec![2],
    });
    let second_idx = gb.add_ugen(UgenSpec {
        class_name: "Pulse".to_string(),
        calc_rate: 2,
        special_index: 0,
        inputs: vec![
            InputSpec::UgenOutput { ugen: freq2, output: 0 },
            width_input.clone(),
        ],
        outputs: vec![2],
    });
    Ok(gb.add_ugen(UgenSpec {
        class_name: "BinaryOpUGen".to_string(),
        calc_rate: 2,
        special_index: 0,
        inputs: vec![
            InputSpec::UgenOutput { ugen: main_idx, output: 0 },
            InputSpec::UgenOutput { ugen: second_idx, output: 0 },
        ],
        outputs: vec![2],
    }))
}

// ---------------------------------------------------------------------------
// Filter compiler
// ---------------------------------------------------------------------------

fn compile_filter(gb: &mut GraphBuilder, filter: &FilterPrimitive, signal: &InputSpec) -> usize {
    match filter {
        FilterPrimitive::Lpf { cutoff } => {
            let cutoff_input = compile_value(gb, cutoff, 1);
            gb.add_ugen(UgenSpec {
                class_name: "LPF".to_string(),
                calc_rate: 2,
                special_index: 0,
                inputs: vec![signal.clone(), cutoff_input],
                outputs: vec![2],
            })
        }
        FilterPrimitive::Hpf { cutoff } => {
            let cutoff_input = compile_value(gb, cutoff, 1);
            gb.add_ugen(UgenSpec {
                class_name: "HPF".to_string(),
                calc_rate: 2,
                special_index: 0,
                inputs: vec![signal.clone(), cutoff_input],
                outputs: vec![2],
            })
        }
        FilterPrimitive::Bpf { freq, q } => {
            let freq_input = compile_value(gb, freq, 1);
            // BPF takes reciprocal of Q
            let rq_val = 1.0 / q.fixed_or_mid();
            let rq_input = match q {
                Value::Fixed(_) => {
                    let ci = gb.add_constant(rq_val);
                    InputSpec::Constant(ci)
                }
                Value::Range(lo, hi) => {
                    // Invert the range: rq = 1/q, so range(1/hi, 1/lo)
                    compile_value(gb, &Value::Range(1.0 / hi, 1.0 / lo), 1)
                }
            };
            gb.add_ugen(UgenSpec {
                class_name: "BPF".to_string(),
                calc_rate: 2,
                special_index: 0,
                inputs: vec![signal.clone(), freq_input, rq_input],
                outputs: vec![2],
            })
        }
    }
}

// ---------------------------------------------------------------------------
// FX compiler
// ---------------------------------------------------------------------------

fn compile_fx(gb: &mut GraphBuilder, fx: &FxPrimitive, signal: &InputSpec) -> usize {
    match fx {
        FxPrimitive::Reverb { mix, room } => {
            let mix_input = compile_value(gb, mix, 1);
            let room_input = compile_value(gb, room, 1);
            let damp_ci = gb.add_constant(0.1);
            gb.add_ugen(UgenSpec {
                class_name: "FreeVerb".to_string(),
                calc_rate: 2,
                special_index: 0,
                inputs: vec![
                    signal.clone(),
                    mix_input,
                    room_input,
                    InputSpec::Constant(damp_ci),
                ],
                outputs: vec![2],
            })
        }
        FxPrimitive::Delay { time, feedback } => {
            let time_input = compile_value(gb, time, 1);
            let maxdelay = match time {
                Value::Fixed(t) => *t,
                Value::Range(_, hi) => *hi,
            };
            let maxdelay_ci = gb.add_constant(maxdelay);
            // decaytime heuristic: higher feedback = longer decay
            let fb_mid = feedback.fixed_or_mid();
            let time_mid = time.fixed_or_mid();
            let decaytime = time_mid * fb_mid * 10.0;
            let decaytime_ci = gb.add_constant(decaytime);
            gb.add_ugen(UgenSpec {
                class_name: "CombN".to_string(),
                calc_rate: 2,
                special_index: 0,
                inputs: vec![
                    signal.clone(),
                    InputSpec::Constant(maxdelay_ci),
                    time_input,
                    InputSpec::Constant(decaytime_ci),
                ],
                outputs: vec![2],
            })
        }
        FxPrimitive::Allpass { time, feedback } => {
            let time_input = compile_value(gb, time, 1);
            let maxdelay = match time {
                Value::Fixed(t) => *t,
                Value::Range(_, hi) => *hi,
            };
            let maxdelay_ci = gb.add_constant(maxdelay);
            // decaytime from feedback
            let fb_mid = feedback.fixed_or_mid();
            let time_mid = time.fixed_or_mid();
            let decaytime = time_mid * fb_mid * 10.0;
            let decaytime_ci = gb.add_constant(decaytime);
            gb.add_ugen(UgenSpec {
                class_name: "AllpassC".to_string(),
                calc_rate: 2,
                special_index: 0,
                inputs: vec![
                    signal.clone(),
                    InputSpec::Constant(maxdelay_ci),
                    time_input,
                    InputSpec::Constant(decaytime_ci),
                ],
                outputs: vec![2],
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Pan position compiler
// ---------------------------------------------------------------------------

fn compile_pan_pos(gb: &mut GraphBuilder, pan: &PanPrimitive) -> InputSpec {
    match pan {
        PanPrimitive::Center => {
            let pos_ci = gb.add_constant(0.0);
            InputSpec::Constant(pos_ci)
        }
        PanPrimitive::Fixed { value } => {
            let pos_ci = gb.add_constant(*value);
            InputSpec::Constant(pos_ci)
        }
        PanPrimitive::Lfo { rate } => {
            let rate_input = compile_value(gb, rate, 1);
            let phase_ci = gb.add_constant(0.0);
            let lfo_idx = gb.add_ugen(UgenSpec {
                class_name: "SinOsc".to_string(),
                calc_rate: 1, // kr
                special_index: 0,
                inputs: vec![rate_input, InputSpec::Constant(phase_ci)],
                outputs: vec![1],
            });
            InputSpec::UgenOutput {
                ugen: lfo_idx,
                output: 0,
            }
        }
        PanPrimitive::Noise { rate, range } => {
            let rate_input = compile_value(gb, rate, 1);
            let lfnoise_idx = gb.add_ugen(UgenSpec {
                class_name: "LFNoise1".to_string(),
                calc_rate: 1, // kr
                special_index: 0,
                inputs: vec![rate_input],
                outputs: vec![1],
            });
            // Scale LFNoise1 output [-1,1] to the pan range
            let (lo, hi) = match range {
                Value::Range(lo, hi) => (*lo, *hi),
                Value::Fixed(v) => (-v.abs(), v.abs()),
            };
            let scale = (hi - lo) / 2.0;
            let offset = (hi + lo) / 2.0;
            let scale_ci = gb.add_constant(scale);
            let offset_ci = gb.add_constant(offset);
            let mul_idx = gb.add_ugen(UgenSpec {
                class_name: "BinaryOpUGen".to_string(),
                calc_rate: 1,
                special_index: 2,
                inputs: vec![
                    InputSpec::UgenOutput {
                        ugen: lfnoise_idx,
                        output: 0,
                    },
                    InputSpec::Constant(scale_ci),
                ],
                outputs: vec![1],
            });
            let add_idx = gb.add_ugen(UgenSpec {
                class_name: "BinaryOpUGen".to_string(),
                calc_rate: 1,
                special_index: 0, // add
                inputs: vec![
                    InputSpec::UgenOutput {
                        ugen: mul_idx,
                        output: 0,
                    },
                    InputSpec::Constant(offset_ci),
                ],
                outputs: vec![1],
            });
            InputSpec::UgenOutput {
                ugen: add_idx,
                output: 0,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Sample (PlayBuf) compiler
// ---------------------------------------------------------------------------

/// Compile a sample-based SynthDef using PlayBuf.ar.
fn compile_sample_block(name: &str, block: &SynthBlock) -> Result<Vec<u8>> {
    let mut gb = GraphBuilder::new();

    let loop_flag = if block.loop_mode.unwrap_or(false) { 1.0 } else { 0.0 };

    let amp_default = match &block.amp {
        Some(v) => v.fixed_or_mid(),
        None => 0.3,
    };

    // Parameters
    gb.add_param("bufnum", 0.0);
    gb.add_param("amp", amp_default);

    let num_params = gb.graph.params.len();
    let control_idx = gb.add_ugen(UgenSpec {
        class_name: "Control".to_string(),
        calc_rate: 1, // kr for bufnum
        special_index: 0,
        inputs: vec![],
        outputs: vec![1; num_params],
    });

    let bufnum_output = InputSpec::UgenOutput {
        ugen: control_idx,
        output: 0,
    };
    let amp_output = InputSpec::UgenOutput {
        ugen: control_idx,
        output: 1,
    };

    // BufRateScale.kr(bufnum)
    let buf_rate_idx = gb.add_ugen(UgenSpec {
        class_name: "BufRateScale".to_string(),
        calc_rate: 1,
        special_index: 0,
        inputs: vec![bufnum_output.clone()],
        outputs: vec![1],
    });
    let rate_output = InputSpec::UgenOutput {
        ugen: buf_rate_idx,
        output: 0,
    };

    // PlayBuf.ar
    let num_ch_ci = gb.add_constant(2.0);
    let trigger_ci = gb.add_constant(1.0);
    let start_pos_ci = gb.add_constant(0.0);
    let loop_ci = gb.add_constant(loop_flag);
    let done_action = if loop_flag == 0.0 { 2.0 } else { 0.0 };
    let done_ci = gb.add_constant(done_action);

    let playbuf_idx = gb.add_ugen(UgenSpec {
        class_name: "PlayBuf".to_string(),
        calc_rate: 2,
        special_index: 0,
        inputs: vec![
            InputSpec::Constant(num_ch_ci),
            bufnum_output.clone(),
            rate_output,
            InputSpec::Constant(trigger_ci),
            InputSpec::Constant(start_pos_ci),
            InputSpec::Constant(loop_ci),
            InputSpec::Constant(done_ci),
        ],
        outputs: vec![2, 2],
    });

    // Mix stereo to mono
    let half_ci = gb.add_constant(0.5);
    let sum_idx = gb.add_ugen(UgenSpec {
        class_name: "BinaryOpUGen".to_string(),
        calc_rate: 2,
        special_index: 0,
        inputs: vec![
            InputSpec::UgenOutput { ugen: playbuf_idx, output: 0 },
            InputSpec::UgenOutput { ugen: playbuf_idx, output: 1 },
        ],
        outputs: vec![2],
    });
    let mono_idx = gb.add_ugen(UgenSpec {
        class_name: "BinaryOpUGen".to_string(),
        calc_rate: 2,
        special_index: 2,
        inputs: vec![
            InputSpec::UgenOutput { ugen: sum_idx, output: 0 },
            InputSpec::Constant(half_ci),
        ],
        outputs: vec![2],
    });

    let mut current_signal = InputSpec::UgenOutput {
        ugen: mono_idx,
        output: 0,
    };

    // Optional filter
    if let Some(filter) = &block.filter {
        let filter_idx = compile_filter(&mut gb, filter, &current_signal);
        current_signal = InputSpec::UgenOutput {
            ugen: filter_idx,
            output: 0,
        };
    }

    // Optional FX
    if let Some(fx) = &block.fx {
        let fx_idx = compile_fx(&mut gb, fx, &current_signal);
        current_signal = InputSpec::UgenOutput {
            ugen: fx_idx,
            output: 0,
        };
    }

    // Amplitude multiply
    let amp_mul_idx = gb.add_ugen(UgenSpec {
        class_name: "BinaryOpUGen".to_string(),
        calc_rate: 2,
        special_index: 2,
        inputs: vec![current_signal, amp_output],
        outputs: vec![2],
    });

    // Pan2 for stereo output
    let bus_ci = gb.add_constant(0.0);
    let pan_pos_ci = gb.add_constant(0.0);
    let pan2_idx = gb.add_ugen(UgenSpec {
        class_name: "Pan2".to_string(),
        calc_rate: 2,
        special_index: 0,
        inputs: vec![
            InputSpec::UgenOutput { ugen: amp_mul_idx, output: 0 },
            InputSpec::Constant(pan_pos_ci),
        ],
        outputs: vec![2, 2],
    });

    // Out.ar stereo
    gb.add_ugen(UgenSpec {
        class_name: "Out".to_string(),
        calc_rate: 2,
        special_index: 0,
        inputs: vec![
            InputSpec::Constant(bus_ci),
            InputSpec::UgenOutput { ugen: pan2_idx, output: 0 },
            InputSpec::UgenOutput { ugen: pan2_idx, output: 1 },
        ],
        outputs: vec![],
    });

    Ok(encode_synthdef(name, &gb.graph))
}

// ---------------------------------------------------------------------------
// Graph builder helper
// ---------------------------------------------------------------------------

struct GraphBuilder {
    graph: UgenGraph,
}

impl GraphBuilder {
    fn new() -> Self {
        Self {
            graph: UgenGraph {
                constants: Vec::new(),
                params: Vec::new(),
                ugens: Vec::new(),
            },
        }
    }

    /// Add a constant to the graph, returning its index.
    /// Reuses existing constant if value matches exactly.
    fn add_constant(&mut self, value: f32) -> usize {
        for (i, &c) in self.graph.constants.iter().enumerate() {
            if c.to_bits() == value.to_bits() {
                return i;
            }
        }
        let idx = self.graph.constants.len();
        self.graph.constants.push(value);
        idx
    }

    /// Add a named parameter with initial value.
    fn add_param(&mut self, name: &str, initial: f32) {
        self.graph.params.push((name.to_string(), initial));
    }

    /// Find a parameter index by name.
    fn param_index(&self, name: &str) -> Option<usize> {
        self.graph
            .params
            .iter()
            .position(|(n, _)| n == name)
    }

    /// Add a UGen to the graph, returning its index.
    fn add_ugen(&mut self, spec: UgenSpec) -> usize {
        let idx = self.graph.ugens.len();
        self.graph.ugens.push(spec);
        idx
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_block() -> SynthBlock {
        SynthBlock {
            notes: None,
            osc: Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])),
            filter: None,
            env: None,
            distort: None,
            fx: None,
            pan: None,
            amp: Some(Value::Fixed(0.1)),
            tempo: None,
            sample: None,
            loop_mode: None,
        }
    }

    // -- Basic compilation --

    #[test]
    fn sine_produces_scgf_header() {
        let block = default_block();
        let bytes = compile_synth_block("test", &block).unwrap();
        assert_eq!(&bytes[0..4], b"SCgf");
    }

    #[test]
    fn sine_has_expected_ugen_chain() {
        let block = default_block();
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(bytes.len() > 50);
        assert_eq!(&bytes[0..4], b"SCgf");
        assert_eq!(&bytes[4..8], &2i32.to_be_bytes());
    }

    #[test]
    fn err_on_missing_osc_and_sample() {
        let block = SynthBlock {
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
        };
        let result = compile_synth_block("test", &block);
        assert!(result.is_err());
    }

    #[test]
    fn amp_defaults_to_0_3() {
        let block = SynthBlock {
            amp: None,
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(bytes.len() > 50);
        let amp_bytes = 0.3f32.to_be_bytes();
        assert!(bytes.windows(4).any(|w| w == amp_bytes));
    }

    // -- Oscillator variants --

    #[test]
    fn saw_uses_saw_ugen() {
        let block = SynthBlock {
            osc: Some(OscLayer(vec![OscPrimitive::Saw { detune: None }])),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "Saw"));
        assert!(!contains_pstring(&bytes, "SinOsc"));
    }

    #[test]
    fn pulse_uses_pulse_ugen() {
        let block = SynthBlock {
            osc: Some(OscLayer(vec![OscPrimitive::Pulse { width: Value::Fixed(0.3), detune: None }])),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "Pulse"));
    }

    #[test]
    fn noise_white_uses_whitenoise() {
        let block = SynthBlock {
            osc: Some(OscLayer(vec![OscPrimitive::Noise {
                noise_type: NoiseType::White,
            }])),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "WhiteNoise"));
    }

    #[test]
    fn noise_pink_uses_pinknoise() {
        let block = SynthBlock {
            osc: Some(OscLayer(vec![OscPrimitive::Noise {
                noise_type: NoiseType::Pink,
            }])),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "PinkNoise"));
    }

    // -- Filter variants --

    #[test]
    fn lpf_inserts_lpf_ugen() {
        let block = SynthBlock {
            filter: Some(FilterPrimitive::Lpf { cutoff: Value::Fixed(800.0) }),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "LPF"));
    }

    #[test]
    fn hpf_inserts_hpf_ugen() {
        let block = SynthBlock {
            filter: Some(FilterPrimitive::Hpf { cutoff: Value::Fixed(200.0) }),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "HPF"));
    }

    #[test]
    fn bpf_inserts_bpf_ugen() {
        let block = SynthBlock {
            filter: Some(FilterPrimitive::Bpf {
                freq: Value::Fixed(2000.0),
                q: Value::Fixed(0.3),
            }),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "BPF"));
    }

    // -- Envelope --

    #[test]
    fn perc_env_inserts_envgen() {
        let block = SynthBlock {
            env: Some(EnvPrimitive::Perc {
                attack: 0.01,
                release: 0.5,
            }),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "EnvGen"));
    }

    #[test]
    fn adsr_env_adds_gate_param() {
        let block = SynthBlock {
            env: Some(EnvPrimitive::Adsr {
                attack: 0.01,
                decay: 0.1,
                sustain: 0.8,
                release: 0.3,
            }),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "EnvGen"));
        assert!(contains_pstring(&bytes, "gate"));
    }

    // -- Distortion --

    #[test]
    fn tanh_distortion_inserts_unaryop() {
        let block = SynthBlock {
            distort: Some(DistortPrimitive::Tanh { drive: Value::Fixed(2.0) }),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "UnaryOpUGen"));
    }

    #[test]
    fn bitcrush_inserts_round() {
        let block = SynthBlock {
            distort: Some(DistortPrimitive::Bitcrush { bits: 8 }),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert_eq!(&bytes[0..4], b"SCgf");
    }

    // -- FX --

    #[test]
    fn reverb_inserts_freeverb() {
        let block = SynthBlock {
            fx: Some(FxPrimitive::Reverb {
                mix: Value::Fixed(0.7),
                room: Value::Fixed(0.95),
            }),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "FreeVerb"));
    }

    #[test]
    fn delay_inserts_combn() {
        let block = SynthBlock {
            fx: Some(FxPrimitive::Delay {
                time: Value::Fixed(0.3),
                feedback: Value::Fixed(0.5),
            }),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "CombN"));
    }

    #[test]
    fn allpass_inserts_allpassc() {
        let block = SynthBlock {
            fx: Some(FxPrimitive::Allpass {
                time: Value::Fixed(0.3),
                feedback: Value::Fixed(0.6),
            }),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "AllpassC"));
    }

    // -- Pan --

    #[test]
    fn center_pan_inserts_pan2() {
        let block = SynthBlock {
            pan: Some(PanPrimitive::Center),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "Pan2"));
    }

    #[test]
    fn lfo_pan_inserts_sinosc_kr_and_pan2() {
        let block = SynthBlock {
            pan: Some(PanPrimitive::Lfo { rate: Value::Fixed(0.5) }),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "Pan2"));
        let count = count_pstring(&bytes, "SinOsc");
        assert_eq!(count, 2, "Expected 2 SinOsc UGens (osc + pan LFO)");
    }

    #[test]
    fn noise_pan_inserts_lfnoise1_and_pan2() {
        let block = SynthBlock {
            pan: Some(PanPrimitive::Noise {
                rate: Value::Fixed(0.1),
                range: Value::Range(-0.5, 0.5),
            }),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "LFNoise1"));
        assert!(contains_pstring(&bytes, "Pan2"));
    }

    #[test]
    fn pan_out_has_two_channels() {
        let block = SynthBlock {
            pan: Some(PanPrimitive::Center),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "Pan2"));
        assert!(contains_pstring(&bytes, "Out"));
    }

    #[test]
    fn no_pan_out_has_one_channel() {
        let block = default_block();
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "Out"));
        assert!(!contains_pstring(&bytes, "Pan2"));
    }

    // -- Range values in compilation --

    #[test]
    fn range_filter_cutoff_produces_lfnoise1() {
        let block = SynthBlock {
            filter: Some(FilterPrimitive::Lpf { cutoff: Value::Range(180.0, 900.0) }),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "LPF"));
        assert!(contains_pstring(&bytes, "LFNoise1"));
    }

    #[test]
    fn range_pulse_width_produces_lfnoise1() {
        let block = SynthBlock {
            osc: Some(OscLayer(vec![OscPrimitive::Pulse {
                width: Value::Range(0.03, 0.08),
                detune: None,
            }])),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "Pulse"));
        assert!(contains_pstring(&bytes, "LFNoise1"));
    }

    // -- Multi-osc --

    #[test]
    fn multi_osc_sums_two() {
        let block = SynthBlock {
            osc: Some(OscLayer(vec![
                OscPrimitive::Saw { detune: None },
                OscPrimitive::Sine { freq: Some(Value::Fixed(30.0)) },
            ])),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        assert!(contains_pstring(&bytes, "Saw"));
        assert!(contains_pstring(&bytes, "SinOsc"));
    }

    // -- Detune --

    #[test]
    fn saw_detune_produces_two_saws() {
        let block = SynthBlock {
            osc: Some(OscLayer(vec![OscPrimitive::Saw {
                detune: Some(Value::Fixed(0.03)),
            }])),
            ..default_block()
        };
        let bytes = compile_synth_block("test", &block).unwrap();
        let saw_count = count_pstring(&bytes, "Saw");
        assert_eq!(saw_count, 2, "Expected 2 Saw UGens (main + detuned)");
    }

    // -- Full combo --

    #[test]
    fn full_combo_compiles() {
        let block = SynthBlock {
            notes: None,
            osc: Some(OscLayer(vec![OscPrimitive::Pulse { width: Value::Fixed(0.5), detune: None }])),
            filter: Some(FilterPrimitive::Lpf { cutoff: Value::Fixed(800.0) }),
            env: Some(EnvPrimitive::Perc {
                attack: 0.01,
                release: 0.5,
            }),
            distort: Some(DistortPrimitive::Tanh { drive: Value::Fixed(2.0) }),
            fx: Some(FxPrimitive::Reverb {
                mix: Value::Fixed(0.7),
                room: Value::Fixed(0.95),
            }),
            pan: Some(PanPrimitive::Center),
            amp: Some(Value::Fixed(0.1)),
            tempo: None,
            sample: None,
            loop_mode: None,
        };
        let bytes = compile_synth_block("full_test", &block).unwrap();
        assert_eq!(&bytes[0..4], b"SCgf");
        assert!(bytes.len() > 100);
    }

    #[test]
    fn all_primitive_combos_produce_nonempty() {
        let oscs = vec![
            OscLayer(vec![OscPrimitive::Sine { freq: None }]),
            OscLayer(vec![OscPrimitive::Saw { detune: None }]),
            OscLayer(vec![OscPrimitive::Pulse { width: Value::Fixed(0.5), detune: None }]),
            OscLayer(vec![OscPrimitive::Noise {
                noise_type: NoiseType::White,
            }]),
        ];
        for osc in oscs {
            let block = SynthBlock {
                osc: Some(osc.clone()),
                ..default_block()
            };
            let bytes = compile_synth_block("test", &block).unwrap();
            assert!(
                !bytes.is_empty(),
                "Empty output for osc: {:?}",
                block.osc
            );
            assert_eq!(&bytes[0..4], b"SCgf");
        }
    }

    // -- Horla test piece (the jam session piece that broke) --

    #[test]
    fn horla_piece_compiles() {
        let block = SynthBlock {
            notes: None,
            osc: Some(OscLayer(vec![OscPrimitive::Pulse {
                width: Value::Range(0.03, 0.08),
                detune: None,
            }])),
            filter: Some(FilterPrimitive::Lpf { cutoff: Value::Range(180.0, 900.0) }),
            env: None,
            distort: None,
            fx: Some(FxPrimitive::Allpass {
                time: Value::Range(0.3, 0.7),
                feedback: Value::Fixed(0.6),
            }),
            pan: Some(PanPrimitive::Noise {
                rate: Value::Fixed(0.04),
                range: Value::Range(-1.0, 1.0),
            }),
            amp: Some(Value::Fixed(0.04)),
            tempo: None,
            sample: None,
            loop_mode: None,
        };
        let bytes = compile_synth_block("horla", &block).unwrap();
        assert_eq!(&bytes[0..4], b"SCgf");
        assert!(bytes.len() > 100);
        assert!(contains_pstring(&bytes, "Pulse"));
        assert!(contains_pstring(&bytes, "LPF"));
        assert!(contains_pstring(&bytes, "AllpassC"));
        assert!(contains_pstring(&bytes, "Pan2"));
        assert!(contains_pstring(&bytes, "LFNoise1"));
    }

    // -- Test helpers --

    /// Check if a pstring (1-byte length + bytes) for the given name appears in the binary.
    fn contains_pstring(bytes: &[u8], name: &str) -> bool {
        let name_bytes = name.as_bytes();
        let len = name_bytes.len() as u8;
        for i in 0..bytes.len().saturating_sub(name_bytes.len()) {
            if bytes[i] == len && i + 1 + name_bytes.len() <= bytes.len() {
                if &bytes[i + 1..i + 1 + name_bytes.len()] == name_bytes {
                    return true;
                }
            }
        }
        false
    }

    /// Count how many times a pstring appears in the binary.
    fn count_pstring(bytes: &[u8], name: &str) -> usize {
        let name_bytes = name.as_bytes();
        let len = name_bytes.len() as u8;
        let mut count = 0;
        for i in 0..bytes.len().saturating_sub(name_bytes.len()) {
            if bytes[i] == len && i + 1 + name_bytes.len() <= bytes.len() {
                if &bytes[i + 1..i + 1 + name_bytes.len()] == name_bytes {
                    count += 1;
                }
            }
        }
        count
    }

    // -- Sample (PlayBuf) tests --

    fn sample_block(loop_mode: bool) -> SynthBlock {
        SynthBlock {
            sample: Some("samples/kick.wav".to_string()),
            loop_mode: Some(loop_mode),
            amp: Some(Value::Fixed(0.8)),
            notes: None,
            osc: None,
            filter: None,
            env: None,
            distort: None,
            fx: None,
            pan: None,
            tempo: None,
        }
    }

    #[test]
    fn sample_block_produces_scgf() {
        let block = sample_block(false);
        let bytes = compile_synth_block("kick", &block).unwrap();
        assert_eq!(&bytes[0..4], b"SCgf");
        assert!(bytes.len() > 50);
    }

    #[test]
    fn sample_block_has_playbuf() {
        let block = sample_block(false);
        let bytes = compile_synth_block("kick", &block).unwrap();
        assert!(contains_pstring(&bytes, "PlayBuf"));
    }

    #[test]
    fn sample_block_has_bufratescale() {
        let block = sample_block(false);
        let bytes = compile_synth_block("kick", &block).unwrap();
        assert!(contains_pstring(&bytes, "BufRateScale"));
    }

    #[test]
    fn sample_block_has_bufnum_param() {
        let block = sample_block(false);
        let bytes = compile_synth_block("kick", &block).unwrap();
        assert!(contains_pstring(&bytes, "bufnum"));
    }

    #[test]
    fn sample_block_has_pan2_and_out() {
        let block = sample_block(false);
        let bytes = compile_synth_block("kick", &block).unwrap();
        assert!(contains_pstring(&bytes, "Pan2"));
        assert!(contains_pstring(&bytes, "Out"));
    }

    #[test]
    fn sample_block_no_oscillator() {
        let block = sample_block(false);
        let bytes = compile_synth_block("kick", &block).unwrap();
        assert!(!contains_pstring(&bytes, "SinOsc"));
        assert!(!contains_pstring(&bytes, "Saw"));
    }

    #[test]
    fn sample_loop_compiles() {
        let block = sample_block(true);
        let bytes = compile_synth_block("loop_pad", &block).unwrap();
        assert_eq!(&bytes[0..4], b"SCgf");
        assert!(contains_pstring(&bytes, "PlayBuf"));
    }

    #[test]
    fn sample_with_filter_compiles() {
        let mut block = sample_block(false);
        block.filter = Some(FilterPrimitive::Lpf { cutoff: Value::Fixed(2000.0) });
        let bytes = compile_synth_block("filtered_sample", &block).unwrap();
        assert!(contains_pstring(&bytes, "PlayBuf"));
        assert!(contains_pstring(&bytes, "LPF"));
    }

    #[test]
    fn sample_with_fx_compiles() {
        let mut block = sample_block(true);
        block.fx = Some(FxPrimitive::Reverb { mix: Value::Fixed(0.5), room: Value::Fixed(0.8) });
        let bytes = compile_synth_block("reverb_sample", &block).unwrap();
        assert!(contains_pstring(&bytes, "PlayBuf"));
        assert!(contains_pstring(&bytes, "FreeVerb"));
    }

    #[test]
    fn sample_overrides_osc() {
        let block = SynthBlock {
            sample: Some("samples/hit.wav".to_string()),
            osc: Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])),
            loop_mode: None,
            amp: Some(Value::Fixed(0.5)),
            notes: None,
            filter: None,
            env: None,
            distort: None,
            fx: None,
            pan: None,
            tempo: None,
        };
        let bytes = compile_synth_block("hit", &block).unwrap();
        assert!(contains_pstring(&bytes, "PlayBuf"));
        assert!(!contains_pstring(&bytes, "SinOsc"));
    }
}

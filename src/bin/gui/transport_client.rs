use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::UdpSocket;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use rosc::{OscMessage, OscPacket, OscType, encoder, decoder};

use crate::beat_detector::BeatDetector;

const SOCKET_PATH: &str = "/tmp/hum.sock";

/// Global reference to GuiState for the FFT polling thread to read per-thing amplitudes.
/// Set once during `start_polling()`.
static GUI_STATE_REF: OnceLock<Arc<Mutex<GuiState>>> = OnceLock::new();

#[derive(Default, Clone)]
pub struct GuiState {
    pub playing: bool,
    pub pos: f64,
    pub active: Vec<String>,
    pub solo: Vec<String>,
    pub mute: Vec<String>,
    pub amplitudes: HashMap<String, f32>,
    pub connected: bool,
}

/// Spawn a background thread that polls the daemon every 100ms.
/// Updates the shared GuiState with latest transport status.
pub fn start_polling(state: Arc<Mutex<GuiState>>) {
    // Store a clone for the FFT polling thread to read per-thing amplitudes
    GUI_STATE_REF.set(Arc::clone(&state)).ok();
    std::thread::spawn(move || loop {
        match poll_once(&state) {
            Ok(()) => {}
            Err(_) => {
                if let Ok(mut s) = state.lock() {
                    s.connected = false;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    });
}

/// Connect to daemon, send status query, parse reply into GuiState.
fn poll_once(state: &Arc<Mutex<GuiState>>) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect(SOCKET_PATH)?;
    stream.set_read_timeout(Some(Duration::from_millis(200)))?;
    stream.write_all(b"{\"cmd\":\"status\"}\n")?;
    stream.flush()?;

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    let reply: serde_json::Value = serde_json::from_str(line.trim())?;

    if let Some(ok) = reply.get("ok").and_then(|v| v.as_str()) {
        if ok == "status" {
            let mut s = state.lock().map_err(|e| format!("lock: {e}"))?;
            s.connected = true;
            s.playing = reply.get("playing").and_then(|v| v.as_bool()).unwrap_or(false);
            s.pos = reply.get("pos").and_then(|v| v.as_f64()).unwrap_or(0.0);
            s.active = parse_string_array(&reply, "active");
            s.solo = parse_string_array(&reply, "solo");
            s.mute = parse_string_array(&reply, "mute");
            s.amplitudes = parse_amplitude_map(&reply);
        }
    }

    Ok(())
}

fn parse_string_array(val: &serde_json::Value, key: &str) -> Vec<String> {
    val.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_amplitude_map(val: &serde_json::Value) -> HashMap<String, f32> {
    val.get("amplitudes")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f as f32)))
                .collect()
        })
        .unwrap_or_default()
}

/// Send a fire-and-forget command to the daemon (play/stop/seek).
/// Ignores errors silently -- the GUI stays responsive regardless.
pub fn send_cmd(cmd: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect(SOCKET_PATH)?;
    stream.set_write_timeout(Some(Duration::from_millis(200)))?;
    stream.write_all(cmd.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// FFT spectral analysis polling (scsynth OSC /b_getn)
// ---------------------------------------------------------------------------

/// Shared FFT bin magnitudes + beat detection + per-thing amplitudes.
/// Written by the FFT polling thread, read by VisualizerView on each frame.
#[derive(Clone)]
pub struct FftState {
    pub bins: [f32; 64],
    /// Beat energy from spectral flux onset detection (0.0..1.0, decays ~200ms).
    pub beat_energy: f32,
    /// Per-thing amplitude slots: (amplitude, name_bytes). Up to 8 things.
    /// `name_bytes` is a 32-byte zero-padded UTF-8 name. Amplitude 0.0 if unused.
    pub per_thing: [(f32, [u8; 32]); 8],
}

impl Default for FftState {
    fn default() -> Self {
        Self {
            bins: [0.0f32; 64],
            beat_energy: 0.0,
            per_thing: [(0.0, [0u8; 32]); 8],
        }
    }
}

/// Spawn a background thread that polls scsynth's FFT analysis buffer at ~30fps.
///
/// Sends OSC `/b_getn` to read 64 samples from buffer 0 (the analysis bus).
/// Expects the daemon's SCD files to include an FFT analysis SynthDef writing
/// to buffer 0 — without one, /b_getn returns zeros (silent = no bars, not a crash).
///
/// The scsynth address is read from `HUM_SCSYNTH_HOST` env var (default "127.0.0.1:57110").
pub fn start_fft_polling(fft_state: Arc<Mutex<FftState>>) {
    std::thread::spawn(move || {
        let scsynth_addr = std::env::var("HUM_SCSYNTH_HOST")
            .unwrap_or_else(|_| "127.0.0.1:57110".to_string());

        let socket = match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[hum-gui] fft socket bind failed: {e}");
                return;
            }
        };
        if let Err(e) = socket.connect(&scsynth_addr) {
            eprintln!("[hum-gui] fft socket connect to {scsynth_addr} failed: {e}");
            return;
        }
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .ok();

        // Beat detector lives in this thread -- no lock contention
        let mut beat_detector = BeatDetector::new();

        loop {
            // Send /b_getn buf=0 start=0 count=64
            if let Ok(buf) = encode_b_getn(0, 0, 64) {
                let _ = socket.send(&buf);
            }

            // Receive /b_setn reply
            let mut recv = [0u8; 2048];
            if let Ok(n) = socket.recv(&mut recv) {
                if let Ok(bins) = decode_b_setn(&recv[..n]) {
                    // Run beat detection on the new FFT frame
                    let beat = beat_detector.update(&bins);

                    // Read per-thing amplitudes from GuiState (populated by daemon polling)
                    let per_thing = read_per_thing_amps();

                    if let Ok(mut state) = fft_state.lock() {
                        state.bins = bins;
                        state.beat_energy = beat;
                        state.per_thing = per_thing;
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(33)); // ~30fps
        }
    });
}

/// Read per-thing amplitudes from the global GuiState reference.
/// Returns a fixed-size array of (amplitude, name_bytes) for up to 8 things.
fn read_per_thing_amps() -> [(f32, [u8; 32]); 8] {
    let mut result = [(0.0f32, [0u8; 32]); 8];
    if let Some(gui_arc) = GUI_STATE_REF.get() {
        if let Ok(gui) = gui_arc.lock() {
            for (i, (name, &amp)) in gui.amplitudes.iter().enumerate() {
                if i >= 8 {
                    break;
                }
                result[i].0 = amp;
                // Copy name bytes (truncated to 32 bytes)
                let name_bytes = name.as_bytes();
                let len = name_bytes.len().min(32);
                result[i].1[..len].copy_from_slice(&name_bytes[..len]);
            }
        }
    }
    result
}

/// Encode an OSC `/b_getn` message: read `count` samples from `buf_index` starting at `start`.
fn encode_b_getn(buf_index: i32, start: i32, count: i32) -> Result<Vec<u8>, String> {
    let msg = OscPacket::Message(OscMessage {
        addr: "/b_getn".to_string(),
        args: vec![
            OscType::Int(buf_index),
            OscType::Int(start),
            OscType::Int(count),
        ],
    });
    encoder::encode(&msg).map_err(|e| format!("osc encode: {e}"))
}

/// Decode an OSC `/b_setn` reply into 64 magnitude bins.
/// Expected args: [buf_index: Int, start: Int, num_samples: Int, val0: Float, val1: Float, ...].
/// If fewer than 64 float values are present, the remainder is zero-padded.
fn decode_b_setn(data: &[u8]) -> Result<[f32; 64], String> {
    let (_, packet) = decoder::decode_udp(data).map_err(|e| format!("osc decode: {e}"))?;

    match packet {
        OscPacket::Message(msg) if msg.addr == "/b_setn" => {
            let mut bins = [0.0f32; 64];
            // Skip first 3 args (buf_index, start, num_samples), collect floats
            let float_args = msg.args.iter().skip(3);
            for (i, arg) in float_args.enumerate() {
                if i >= 64 {
                    break;
                }
                match arg {
                    OscType::Float(f) => bins[i] = f.clamp(0.0, 1.0),
                    OscType::Double(d) => bins[i] = (*d as f32).clamp(0.0, 1.0),
                    OscType::Int(n) => bins[i] = (*n as f32).clamp(0.0, 1.0),
                    _ => {}
                }
            }
            Ok(bins)
        }
        _ => Err("not a /b_setn message".to_string()),
    }
}

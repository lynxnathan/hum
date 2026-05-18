---
phase: 11-makepad-gui
plan: 3
type: execute
wave: 2
depends_on: [11-01]
files_modified:
  - src/bin/gui/spectral_view.rs
  - src/bin/gui/app.rs
  - src/bin/gui/transport_client.rs
autonomous: true
requirements: [MKPD-02]

must_haves:
  truths:
    - "Spectral analyzer zone shows real-time FFT frequency bars reacting to audio"
    - "Bars update at ~30fps without frame drops"
    - "When nothing is playing, bars rest at zero/near-zero"
  artifacts:
    - path: "src/bin/gui/spectral_view.rs"
      provides: "SpectralView widget with Makepad shader DSL rendering FFT bins as bars"
      exports: ["SpectralView"]
  key_links:
    - from: "src/bin/gui/spectral_view.rs"
      to: "src/bin/gui/transport_client.rs"
      via: "Arc<Mutex<FftState>> written by background thread reading scsynth /b_getn"
      pattern: "fft_state\\.lock.*bins"
    - from: "transport_client.rs FFT thread"
      to: "scsynth OSC"
      via: "rosc UdpSocket sending /b_getn on analysis bus index 0"
      pattern: "b_getn"
---

<objective>
Build the spectral analyzer: a background thread polls scsynth's FFT analysis bus via OSC /b_getn, and a Makepad shader renders the frequency bins as animated vertical bars.

Purpose: Delivers the real-time audio visualization that makes hum-gui feel alive as a performance tool.
Output: Spectral analyzer zone shows 64 frequency bars reacting to live scsynth audio.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/phases/11-makepad-gui/11-1-SUMMARY.md

<interfaces>
<!-- scsynth FFT analysis via OSC /b_getn -->
<!-- Requires: a running FFT analysis SynthDef writing to a buffer. -->
<!-- OSC message: /b_getn [buf_index: 0, start_sample: 0, num_samples: 64] -->
<!-- Reply: /b_setn [buf_index, start_sample, num_samples, val0, val1, ...val63] -->
<!-- rosc is already in Cargo.toml -->

<!-- rosc encoding example: -->
```rust
use rosc::{OscMessage, OscPacket, OscType, encoder};
let msg = OscPacket::Message(OscMessage {
    addr: "/b_getn".to_string(),
    args: vec![OscType::Int(0), OscType::Int(0), OscType::Int(64)],
});
let buf = encoder::encode(&msg).unwrap();
```

<!-- GuiState (from Plan 1) — FftState is SEPARATE to avoid lock contention -->
```rust
pub struct FftState {
    pub bins: [f32; 64],   // magnitude per frequency bin, 0.0..1.0
}
```

<!-- Makepad shader DSL pattern for custom drawing (from research): -->
<!-- Use DrawQuad with fn pixel() -> vec4 { ... } in live_design! -->
<!-- Pass uniform array via Makepad's shader uniform system -->
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: FFT polling thread in transport_client</name>
  <files>src/bin/gui/transport_client.rs</files>
  <action>
Add FftState struct and a second background thread that polls scsynth's analysis buffer.

```rust
#[derive(Clone)]
pub struct FftState {
    pub bins: [f32; 64],
}
impl Default for FftState {
    fn default() -> Self { Self { bins: [0.0f32; 64] } }
}

pub fn start_fft_polling(fft_state: Arc<Mutex<FftState>>) {
    std::thread::spawn(move || {
        // Connect to scsynth OSC — read host from env HUM_SCSYNTH_HOST (default "127.0.0.1:57110")
        let scsynth_addr = std::env::var("HUM_SCSYNTH_HOST")
            .unwrap_or_else(|_| "127.0.0.1:57110".to_string());
        let socket = std::net::UdpSocket::bind("0.0.0.0:0").expect("fft socket bind");
        socket.connect(&scsynth_addr).expect("fft socket connect");
        socket.set_read_timeout(Some(std::time::Duration::from_millis(50))).ok();

        loop {
            // Send /b_getn 0 0 64
            if let Ok(buf) = encode_b_getn(0, 0, 64) {
                let _ = socket.send(&buf);
            }
            // Receive /b_setn reply
            let mut recv = [0u8; 1024];
            if let Ok(n) = socket.recv(&mut recv) {
                if let Ok(bins) = decode_b_setn(&recv[..n]) {
                    *fft_state.lock().unwrap() = FftState { bins };
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(33)); // ~30fps
        }
    });
}
```

Implement encode_b_getn() using rosc encoder (already in Cargo.toml). Implement decode_b_setn() using rosc decoder: parse OscPacket::Message with addr "/b_setn", extract Float args starting at index 3 (skip buf_idx, start, num), collect into [f32; 64]. If fewer than 64 samples returned, zero-pad.

NOTE: The FFT analysis bus requires a running FFT SynthDef in scsynth. If the daemon hasn't loaded one, /b_getn returns zeros — this is correct behavior (silent = no bars). Document this requirement in a comment: the daemon's SCD files must include an FFT analysis synth writing to buffer 0.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build --bin hum-gui 2>&1 | tail -5</automated>
  </verify>
  <done>transport_client compiles with FftState and start_fft_polling exported.</done>
</task>

<task type="auto">
  <name>Task 2: SpectralView shader widget + wire into app</name>
  <files>src/bin/gui/spectral_view.rs, src/bin/gui/app.rs</files>
  <action>
Create SpectralView widget using Makepad's DrawQuad with a custom pixel shader that renders 64 frequency bars.

Approach: Use a DrawQuad instance per bar (64 quads), each positioned and sized proportionally. This avoids needing a uniform array (which requires deeper Makepad shader DSL knowledge) and is simpler to implement correctly.

In SpectralView.draw_walk():
- Iterate bins[0..64] from FftState
- For each bin i: compute rect x = (i / 64.0) * total_width, width = total_width / 64.0 - 1px gap
- height = bins[i] * max_height (clamped 0.0..1.0)
- y = bottom - height (bars grow upward from bottom)
- Color: lerp from #313244 (zero) through #89b4fa (blue, mid) to #cba6f7 (mauve, high) based on bins[i]
- Draw using draw_quad.draw_abs(cx, Rect { ... })

SpectralView struct:
```rust
pub struct SpectralView {
    #[live] draw_bg: DrawColor,
    #[live] draw_bar: DrawColor,
    #[walk] walk: Walk,
    #[rust] fft_state: Arc<Mutex<FftState>>,
}
```

In app.rs:
1. Add `fft_state: Arc<Mutex<FftState>>` to App struct
2. In after_new_from_doc: call start_fft_polling(Arc::clone(&self.fft_state))
3. Replace SpectralZone placeholder View with SpectralView widget, passing fft_state arc
4. Ensure cx.start_interval(0.033) triggers redraws at ~30fps (can share with existing 0.1s interval — use the faster rate)

The spectral view height stays at 120px as established in Plan 2 layout.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build --bin hum-gui 2>&1 | tail -5</automated>
  </verify>
  <done>SpectralView compiles and renders in the top zone. With scsynth running audio and an FFT analysis SynthDef loaded, bars animate. Without FFT SynthDef, bars remain at zero (not a crash).</done>
</task>

</tasks>

<verification>
1. `cargo build --bin hum-gui` passes
2. With scsynth running: bars show non-zero values when audio plays
3. No crash when scsynth is unreachable (UDP send/recv errors swallowed gracefully)
4. 30fps redraw does not block the UI or cause jank in transport bar updates
</verification>

<success_criteria>
- SpectralView renders 64 frequency bars in the top zone
- Bars react to live audio when FFT analysis bus is active
- Graceful degradation: all-zero bars when no audio or no FFT SynthDef loaded
</success_criteria>

<output>
After completion, create `.planning/phases/11-makepad-gui/11-3-SUMMARY.md`
</output>

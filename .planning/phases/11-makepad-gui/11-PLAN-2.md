---
phase: 11-makepad-gui
plan: 2
type: execute
wave: 2
depends_on: [11-01]
files_modified:
  - src/bin/gui/app.rs
  - src/bin/gui/arrangement_view.rs
  - src/bin/gui/vu_meters.rs
autonomous: true
requirements: [MKPD-03, MKPD-04, MKPD-06, MKPD-07, MKPD-08]

must_haves:
  truths:
    - "Arrangement view shows one horizontal lane per thing with a colored block from at: to until:"
    - "A playhead line moves across lanes tracking current playback position"
    - "VU meters show per-thing amplitude bars updated from daemon Status polling"
    - "Clicking a thing lane toggles solo/mute and sends Solo/Mute cmd to daemon"
    - "Piece overview shows all things color-coded with at:/until: timing ranges"
  artifacts:
    - path: "src/bin/gui/arrangement_view.rs"
      provides: "ArrangementView widget — lanes, blocks, playhead, click handling"
      exports: ["ArrangementView"]
    - path: "src/bin/gui/vu_meters.rs"
      provides: "VuMeters widget — per-thing amplitude bars from GuiState.amplitudes"
      exports: ["VuMeters"]
  key_links:
    - from: "src/bin/gui/arrangement_view.rs"
      to: "src/bin/gui/transport_client.rs"
      via: "GuiState.active / .solo / .mute / .pos read per frame"
      pattern: "gui_state\\.lock.*active"
    - from: "src/bin/gui/arrangement_view.rs"
      to: "daemon"
      via: "transport_client::send_cmd on click (Solo/Mute JSON)"
      pattern: "send_cmd.*solo|mute"
    - from: "src/bin/gui/vu_meters.rs"
      to: "src/bin/gui/transport_client.rs"
      via: "GuiState.amplitudes HashMap<String, f32> read per frame"
      pattern: "amplitudes\\.get"
---

<objective>
Build the arrangement view (Ableton-style thing lanes with colored at:/until: blocks and scrolling playhead), per-thing VU meters, and click-to-solo/mute interaction.

Purpose: Delivers the visual composition feedback core — user sees their piece as a timeline and controls playback per-thing.
Output: Arrangement zone shows live timing blocks, VU meters pulse with amplitude, clicks trigger solo/mute.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/phases/11-makepad-gui/11-1-SUMMARY.md

<interfaces>
<!-- GuiState (from Plan 1 transport_client.rs) -->
```rust
pub struct GuiState {
    pub playing: bool,
    pub pos: f64,              // current playback position in seconds
    pub active: Vec<String>,   // things currently playing
    pub solo: Vec<String>,
    pub mute: Vec<String>,
    pub amplitudes: HashMap<String, f32>,  // thing_name -> 0.0..1.0
    pub connected: bool,
}
```

<!-- Transport commands for solo/mute -->
<!-- send_cmd(r#"{"cmd":"solo","thing":"bass"}"#) -->
<!-- send_cmd(r#"{"cmd":"mute","thing":"bass"}"#) -->

<!-- .hum timing: at:/until: are beat positions (f64). -->
<!-- To display as timeline blocks, normalize by total piece duration. -->
<!-- Parse piece.hum at startup to get thing names + at/until values. -->
<!-- Fallback: derive from GuiState.active list with estimated ranges. -->
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: ArrangementView — lanes, blocks, playhead</name>
  <files>src/bin/gui/arrangement_view.rs</files>
  <action>
Create ArrangementView as a Makepad custom widget using DrawQuad (solid colored rectangles) for thing blocks and the playhead line.

Data model stored in ArrangementView:
```rust
pub struct ThingLane {
    pub name: String,
    pub at: f64,     // start beat
    pub until: f64,  // end beat
    pub color: Vec4, // assigned color per thing (cycle through palette)
}
```

ArrangementView reads ThingLane data from a Vec<ThingLane> passed in, plus GuiState for live state.

Layout:
- Left column (120px): thing name labels
- Right area: scrollable timeline canvas
- Each lane: 32px tall, full width, with a colored block from (at/total_duration * width) to (until/total_duration * width)
- Playhead: 2px wide vertical line at (pos/total_duration * width), color #f38ba8 (red)
- Active things: block color at full opacity; inactive: 40% opacity
- Solo'd things: block border #f9e2af (yellow); Muted: block color greyed out

On click (MouseDown event on a lane row):
- Determine which thing was clicked by y position
- If thing is already solo'd → send `{"cmd":"solo","thing":"..."}` again to toggle off (daemon handles toggle logic)
- Otherwise → send `{"cmd":"solo","thing":"..."}`

Parse piece.hum at startup to populate ThingLane list. Read the file at the path from env var HUM_PIECE (default: piece.hum in cwd). Parse YAML `things:` block extracting name, at, until. Use serde-saphyr (already in Cargo.toml) for YAML parsing. If parsing fails, fall back to empty lane list.

Color palette (Catppuccin Mocha, cycling):
- #89b4fa (blue), #a6e3a1 (green), #fab387 (peach), #cba6f7 (mauve), #f38ba8 (red), #89dceb (sky)

ArrangementView stores Arc<Mutex<GuiState>> passed from App.

total_duration: computed as max(until) across all things, minimum 60.0 seconds.
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build --bin hum-gui 2>&1 | tail -5</automated>
  </verify>
  <done>ArrangementView compiles. When integrated into app.rs, arrangement zone shows colored lanes with blocks.</done>
</task>

<task type="auto">
  <name>Task 2: VuMeters widget + wire everything into app layout</name>
  <files>src/bin/gui/vu_meters.rs, src/bin/gui/app.rs</files>
  <action>
Create VuMeters widget that renders per-thing amplitude bars vertically stacked, matching the arrangement lanes.

VuMeters layout:
- Same 32px lane height as ArrangementView
- Each bar: full width of the VU panel (80px), height = amplitude * 28px
- Bar color: gradient from #a6e3a1 (green, low) to #f38ba8 (red, high) — implement as: if amp < 0.7 use green, else red
- Thing name label left-aligned
- Bars animate by reading GuiState.amplitudes on each redraw

In VuMeters.draw_walk(), iterate GuiState.amplitudes keys (sorted), draw a filled rect per thing.

Update app.rs to:
1. Replace the arrangement placeholder with ArrangementView (passing Arc<Mutex<GuiState>>)
2. Replace the VU placeholder with VuMeters (passing Arc<Mutex<GuiState>>)
3. Add piece path loading: read HUM_PIECE env var, pass ThingLane list to ArrangementView

Final layout in live_design!:
```
Window
  └── View (flow: Down)
        ├── SpectralZone    (height: 120px, placeholder grey box — Plan 3 fills this)
        ├── View (flow: Right, height: Fill)
        │     ├── ArrangementView (width: Fill)
        │     └── VuMeters (width: 80px)
        └── TransportBar (height: 48px)
```

The SpectralZone is a View with draw_bg color #313244 and label "Spectral Analyzer — Phase 3".
  </action>
  <verify>
    <automated>cd ~/code/hum && cargo build --bin hum-gui 2>&1 | tail -5</automated>
  </verify>
  <done>Full layout compiles and renders. Arrangement shows thing lanes from piece.hum. VU bars pulse with amplitude data from daemon. Clicking a lane sends solo/mute to daemon. Transport bar from Plan 1 remains functional.</done>
</task>

</tasks>

<verification>
1. `cargo build --bin hum-gui` passes
2. With `HUM_PIECE=piece.hum cargo run --bin hum-gui` and daemon running: arrangement shows lanes with colored blocks matching the things in piece.hum
3. Playhead moves during playback
4. VU meters show amplitude activity for active things
5. Clicking a lane solos that thing (audible in scsynth, and daemon Status.solo updates)
</verification>

<success_criteria>
- Arrangement view renders correct at:/until: blocks per thing
- Playhead tracks playback position
- VU meters respond to live amplitude data
- Click-to-solo/mute confirmed working end-to-end
</success_criteria>

<output>
After completion, create `.planning/phases/11-makepad-gui/11-2-SUMMARY.md`
</output>

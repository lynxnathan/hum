# Requirements: ghostinstrument v5.0

**Defined:** 2026-03-27
**Core Value:** Launch. Drag nodes. Hear sound change. 20ms latency ceiling.

## v1 Requirements

Requirements for v5.0 "Two Nodes Make Sound." Each maps to roadmap phases.

### Audio

- [ ] **AUD-01**: Two fundsp oscillators produce sound at different pitches through Windows speakers via cpal WASAPI
- [ ] **AUD-02**: Audio callback is allocation-free with pre-built fundsp graph
- [ ] **AUD-03**: Sample rate is negotiated from device config, not hardcoded
- [ ] **AUD-04**: Audio stream initializes on a dedicated thread without blocking Makepad UI

### Canvas

- [ ] **CAN-01**: Dark canvas background fills the Makepad window
- [ ] **CAN-02**: Two circles rendered at distinct positions with different colors indicating different nodes
- [ ] **CAN-03**: User can click and drag a node to move it anywhere on the canvas
- [ ] **CAN-04**: Node positions persist across frames (state tracked per node)

### Spatial

- [ ] **SPA-01**: Stereo panning derived from node X position relative to canvas center (equal-power law)
- [ ] **SPA-02**: Dragging a node left/right audibly shifts its sound between left and right speakers
- [ ] **SPA-03**: Proximity blending: dragging two nodes close together blends their sounds
- [ ] **SPA-04**: Proximity blending: dragging two nodes apart isolates their sounds
- [ ] **SPA-05**: All audio parameters (pan, blend) use one-pole smoothing to prevent zipper noise

### Build

- [ ] **BLD-01**: ghostinstrument binary cross-compiles from WSL2 to x86_64-pc-windows-msvc via cargo-xwin
- [ ] **BLD-02**: Compiled .exe launches on Windows and shows a Makepad window with audio output

## v2 Requirements

Deferred to future milestone. Tracked but not in v5.0 roadmap.

### Visual Polish

- **VIZ-01**: Amplitude pulse rings around nodes indicating audio activity
- **VIZ-02**: Connection lines between nearby nodes showing blend strength
- **VIZ-03**: Node color shifts based on proximity state

### Additional Nodes

- **NOD-01**: User can add/remove nodes dynamically
- **NOD-02**: Node type selector (sine, saw, noise, sample)
- **NOD-03**: Node parameter inspector on click

### Input Devices

- **INP-01**: MIDI controller binding to node parameters
- **INP-02**: Phone sensor input (accelerometer → node position)
- **INP-03**: Gamepad support for node navigation

## Out of Scope

| Feature | Reason |
|---------|--------|
| scsynth integration | fundsp only for v5.0 — zero external deps |
| RAVE / DDSP neural nodes | Future milestone — needs model infrastructure |
| Timeline / recording | Live-only for v5.0 |
| HRTF / binaural | Stereo panning sufficient for proof |
| Node trait abstraction | Two hardcoded nodes is correct — premature to generalize |
| Persistence / presets | No save/load needed yet |
| VST hosting | Never — Nodes Not Plugins stance |
| Amplitude-based node sizing | Anti-feature — makes drag hit detection unstable |
| Grid snap | Anti-feature — freeform spatial positioning is the point |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| AUD-01 | Phase 1 — Audio Core | Pending |
| AUD-02 | Phase 1 — Audio Core | Pending |
| AUD-03 | Phase 1 — Audio Core | Pending |
| AUD-04 | Phase 1 — Audio Core | Pending |
| CAN-01 | Phase 2 — Canvas + Cross-Compile | Pending |
| CAN-02 | Phase 2 — Canvas + Cross-Compile | Pending |
| CAN-03 | Phase 2 — Canvas + Cross-Compile | Pending |
| CAN-04 | Phase 2 — Canvas + Cross-Compile | Pending |
| BLD-01 | Phase 2 — Canvas + Cross-Compile | Pending |
| SPA-01 | Phase 3 — Spatial Wiring | Pending |
| SPA-02 | Phase 3 — Spatial Wiring | Pending |
| SPA-03 | Phase 3 — Spatial Wiring | Pending |
| SPA-04 | Phase 3 — Spatial Wiring | Pending |
| SPA-05 | Phase 3 — Spatial Wiring | Pending |
| BLD-02 | Phase 3 — Spatial Wiring | Pending |

**Coverage:**
- v1 requirements: 15 total
- Mapped to phases: 15
- Unmapped: 0 ✓

---
*Requirements defined: 2026-03-27*
*Last updated: 2026-03-27 — traceability populated by roadmapper*

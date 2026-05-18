# Roadmap: HUM / ghostinstrument

## Overview

Build hum-rt bottom-up along its dependency chain: establish the OSC connection to scsynth, define the data model via strict YAML parsing, wire the reconciler and file watcher to get the first save-to-sound feedback loop, then cap with the transport CLI. Each phase delivers a verifiable capability before the next begins. v5.0 begins a new product — ghostinstrument — a spatial audio canvas built from audio core upward, ending with two draggable oscillator nodes producing stereo-panned, proximity-blended sound on Windows.

## Milestones

- ✅ **v1.0 MVP** - Phases 1-4 (shipped 2026-03-20)
- ✅ **v2.0 Inline Synth IR + Pipe Language** - Phases 5-8 (shipped 2026-03-22)
- ✅ **v3.0 Translation Pipeline + Makepad GUI** - Phases 9-12 (shipped 2026-03-22)
- ✅ **v4.0 The Sound IDE** - Phases 13-15 (shipped 2026-03-22)
- 🚧 **v5.0 Two Nodes Make Sound** - Phases 1-3 (in progress, new milestone counter)

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1-4) - SHIPPED 2026-03-20</summary>

### Phase 1: OSC Bridge
**Goal**: hum-rt can connect to a configurable scsynth host and communicate via OSC
**Depends on**: Nothing (first phase)
**Requirements**: OSC-01, OSC-02, OSC-03, OSC-04, OSC-05, OSC-06
**Success Criteria** (what must be TRUE):
  1. Running `hum-rt` with SCSYNTH_HOST set connects to scsynth without crashing
  2. hum-rt can send a hardcoded SynthDef to scsynth via /d_recv and wait for /sync confirmation before proceeding
  3. hum-rt can create, update parameters on, and free a synth node — scsynth has no orphaned nodes after free
  4. Changing SCSYNTH_HOST env var or config file changes which scsynth instance hum-rt targets
**Plans**: 2 plans

Plans:
- [x] 01-PLAN-1.md — Cargo project bootstrap + layered config (OSC-01, OSC-02)
- [x] 01-PLAN-2.md — ScsynthClient: full OSC lifecycle + smoke test (OSC-03, OSC-04, OSC-05, OSC-06)

### Phase 2: Parser + SCD Reader
**Goal**: hum-rt can parse any valid .hum file with strict validation and associate .scd files with thing names
**Depends on**: Phase 1
**Requirements**: PARSE-01, PARSE-02, PARSE-03, PARSE-04, SCD-01, SCD-02, SCD-03
**Success Criteria** (what must be TRUE):
  1. A valid .hum file with all supported fields (at, until, does, where, has, within, every, mood, like, ref) parses without error
  2. A .hum file with an unknown field is rejected with a message showing line number, field name, and a suggestion
  3. .scd files in out/sc/ are read and their SynthDefs are loaded into scsynth on startup
  4. Each .scd file is associated with its thing name from piece.hum (e.g., space-crackle.scd → thing "space-crackle")
**Plans**: 2 plans

Plans:
- [x] 02-PLAN-1.md — .hum parser: types, deny_unknown_fields, DoesField enum, HumParseError (PARSE-01, PARSE-02, PARSE-03, PARSE-04)
- [x] 02-PLAN-2.md — SCD reader: ScdStore + startup SynthDef loading into scsynth (SCD-01, SCD-02, SCD-03)

### Phase 3: State, Reconciler + File Watcher
**Goal**: Editing piece.hum while the daemon runs changes what scsynth plays — the core feedback loop works
**Depends on**: Phase 2
**Requirements**: WATCH-01, WATCH-02, WATCH-03, WATCH-04, TIME-01, TIME-02, TIME-03
**Success Criteria** (what must be TRUE):
  1. Saving piece.hum triggers hum-rt to diff desired vs current state and send only the minimal OSC delta within ~1 second
  2. Saving a .scd file while a thing is playing causes hum-rt to reload the SynthDef and crossfade to the new sound
  3. Things activate at their `at:` time and deactivate at their `until:` time as playback advances
  4. Things with no `until:` remain active until explicitly removed or stopped
  5. Files under /mnt/ paths (NTFS-mounted via WSL2) are watched correctly via PollWatcher fallback
**Plans**: 3 plans

Plans:
- [x] 03-PLAN-1.md — events + state + reconciler: pure logic core with unit tests (WATCH-02, TIME-01, TIME-02, TIME-03)
- [x] 03-PLAN-2.md — watcher + timeline: file watching with /mnt/ PollWatcher fallback and ticker (WATCH-01, WATCH-04)
- [x] 03-PLAN-3.md — main.rs event loop wiring: full daemon integration + human verify (WATCH-01, WATCH-02, WATCH-03, WATCH-04, TIME-01, TIME-02, TIME-03)

### Phase 4: Transport + E2E
**Goal**: Full CLI control surface works and all three end-to-end scenarios pass on a real piece.hum
**Depends on**: Phase 3
**Requirements**: XPORT-01, XPORT-02, XPORT-03, XPORT-04, XPORT-05, XPORT-06, XPORT-07, E2E-01, E2E-02, E2E-03
**Success Criteria** (what must be TRUE):
  1. `hum play` starts playback; `hum stop` stops it and frees all synth nodes; `hum status` shows current time and active things
  2. `hum solo <thing>` mutes all other things and survives a file reload; `hum mute <thing>` mutes that thing and also survives reload
  3. `hum play from <time>` seeks to the given position and starts playing from there
  4. `hum loop <start> <end>` loops playback between two time points continuously
  5. A piece.hum with multiple things at different `at:` times plays each thing at the correct moment
  6. Editing piece.hum while playing changes what is heard within ~1 second
  7. Editing a .scd file while playing changes the sound character of the running synth via crossfade
**Plans**: 3 plans

Plans:
- [x] 04-PLAN-1.md — Unix socket transport layer: protocol types, socket server, hum CLI subcommands (XPORT-01, XPORT-02, XPORT-03, XPORT-06, XPORT-07)
- [x] 04-PLAN-2.md — Solo/mute state + active_things_filtered + reconciler integration (XPORT-04, XPORT-05)
- [x] 04-PLAN-3.md — Test piece.hum + E2E human verification of all three scenarios (E2E-01, E2E-02, E2E-03)

</details>

<details>
<summary>✅ v2.0 Inline Synth IR + Pipe Language (Phases 5-8) - SHIPPED 2026-03-22</summary>

### Phase 5: Synth IR
**Goal**: hum-rt compiles `synth:` blocks directly to scsynth OSC with no sclang involved
**Depends on**: Phase 4
**Requirements**: IR-01, IR-02, IR-03, IR-04, IR-05, IR-06, IR-07, IR-08, IR-09, IR-10, IR-11
**Success Criteria** (what must be TRUE):
  1. A .hum thing with a `synth:` block (osc + filter + env + fx + pan + amp + notes) plays sound without any .scd file present
  2. All osc primitives (sine, saw, pulse, noise), filter types (lpf, hpf, bpf), env shapes (perc, adsr), distort variants, fx types (reverb, delay), and pan modes compile without error
  3. Editing `synth:` fields while playing causes the sound to update within ~1 second (hot-swap)
  4. When out/sc/<thing>.scd exists, it overrides the `synth:` block — .scd escape hatch takes precedence
**Plans**: 3 plans

Plans:
- [x] 05-PLAN-1.md — SynthIR types + parser: `synth:` YAML block deserialization (IR-01, IR-02, IR-03, IR-04, IR-05, IR-06, IR-07, IR-08, IR-09)
- [x] 05-PLAN-2.md — IR compiler: SynthIR → SynthDef binary + OSC dispatch (IR-10)
- [x] 05-PLAN-3.md — Escape hatch integration + hot-swap wiring + note sequencer (IR-08, IR-10, IR-11)

### Phase 6: Ref Resolution + Pipe Language
**Goal**: Things can reference other things' synth fields, and pipe expressions compose sounds functionally
**Depends on**: Phase 5
**Requirements**: REF-01, REF-02, REF-03, REF-04, PIPE-01, PIPE-02, PIPE-03, PIPE-04, PIPE-05, PIPE-06, PIPE-07, PIPE-08, PIPE-09
**Success Criteria** (what must be TRUE):
  1. A thing with `ref: other-thing` inherits all synth fields from the referenced thing, with local fields overriding
  2. `ref(thing).notes` in a synth block pulls only the notes sequence from the referenced thing
  3. A `pipe:` block with `|>` chain (replicate, each, shift, spread, take, repeat) produces the correct multi-voice output in scsynth
  4. Pipe source `thing.notes` correctly resolves the field accessor before applying transforms
**Plans**: 3 plans

Plans:
- [x] 06-PLAN-1.md — Ref resolver: ref: field + ref(thing).field accessor in synth: (REF-01, REF-02, REF-03, REF-04)
- [x] 06-PLAN-2.md — Pipe parser: `|>` syntax + all transform types (PIPE-01, PIPE-02, PIPE-03, PIPE-04, PIPE-05, PIPE-06, PIPE-07, PIPE-08)
- [x] 06-PLAN-3.md — Pipe execution: thing name + thing.field sources, output → OSC (PIPE-09)

### Phase 7: Instruments + Stage Effects
**Goal**: Reusable instrument definitions and group-level stage effects are fully supported
**Depends on**: Phase 5
**Requirements**: INST-01, INST-02, INST-03, STAGE-01, STAGE-02, STAGE-03
**Success Criteria** (what must be TRUE):
  1. An `instruments/` directory with `type: instrument` .hum files loads on startup; a thing using `instrument:` field inherits its synth definition
  2. A thing's `synth:` fields override the base instrument's fields
  3. A `type: stage` thing with `applies-to:` routes matching things through a shared group bus and applies its `fx:` chain
**Plans**: 2 plans

Plans:
- [x] 07-PLAN-1.md — Instrument loader: instruments/ directory scan + InstrumentStore (INST-01, INST-02, INST-03)
- [x] 07-PLAN-2.md — Stage effects: type:stage parser + group bus routing + fx chain OSC (STAGE-01, STAGE-02, STAGE-03)

### Phase 8: GPUI Dashboard + Transport Fix
**Goal**: `hum watch` opens a GPU-accelerated dashboard via GPUI; `hum play from <time>` works as a single command
**Depends on**: Phase 5
**Requirements**: TUI-01, TUI-02, TUI-03, XFIX-01
**Success Criteria** (what must be TRUE):
  1. `hum watch` opens a GPUI window showing a scrolling timeline with currently-active things highlighted
  2. Per-thing VU meters update in real time by polling scsynth node amplitudes (120fps capable)
  3. Current playback position and transport state (playing/stopped/looping) are visible at all times
  4. `hum play from 1m30s` seeks to that position and starts playing in a single command (no separate seek + play)
**Plans**: 2 plans

Plans:
- [x] 08-PLAN-1.md — Transport protocol: PlayFrom cmd + amplitude data in Status reply + parse_time_arg XmYs (XFIX-01, TUI-02)
- [x] 08-PLAN-2.md — GPUI watch window: timeline + VU meters + transport bar polling daemon (TUI-01, TUI-02, TUI-03)

</details>

<details>
<summary>✅ v3.0 Translation Pipeline + Makepad GUI (Phases 9-12) - SHIPPED 2026-03-22</summary>

### Phase 9: Dictionary
**Goal**: hum-rt loads and exposes a shared vocabulary that maps human vibes to synthesis parameters
**Depends on**: Phase 8
**Requirements**: DICT-01, DICT-02, DICT-03, DICT-04, DICT-05, DICT-06, DICT-07
**Success Criteria** (what must be TRUE):
  1. hum-rt starts up and loads `hum.dict` from the project root; if `~/.config/hum/global.dict` exists, entries are merged (project overrides global)
  2. A `synth:` block with `style: laser` resolves to the synth parameter set defined under "laser" in hum.dict, and the thing plays sound
  3. `hum dict list` prints all vocabulary entries; `hum dict show laser` prints the synth mapping and context for "laser"
  4. Saving hum.dict while the daemon is running causes hum-rt to reload it without restart — affected things update within ~1 second
**Plans**: 2 plans

Plans:
- [x] 09-PLAN-1.md — DictStore + style: resolution + watcher hot-reload (DICT-01, DICT-02, DICT-03, DICT-04, DICT-07)
- [x] 09-PLAN-2.md — CLI dict list/show transport commands + sample hum.dict (DICT-05, DICT-06)

### Phase 10: Translation Sync
**Goal**: All three layers of a .hum thing (like:, pipe:, synth:) stay semantically in sync, and new vocabulary is captured from approved sounds
**Depends on**: Phase 9
**Requirements**: SYNC-01, SYNC-02, SYNC-03, SYNC-04, SYNC-05
**Success Criteria** (what must be TRUE):
  1. When `synth:` is manually edited diverging from `pipe:`, hum-rt (or the LLM via dict) inserts a divergence comment in the file marking the stale layer
  2. When `pipe:` changes, the expanded synth output reflects the new pipe state without manual `synth:` edits
  3. `hum dict add <term>` captures the current thing's synth params as a new dictionary entry for the given term
  4. `hum dict suggest` analyzes piece.hum and outputs recurring synth patterns with suggested term names
**Plans**: 2 plans

Plans:
- [x] 10-PLAN-1.md — Divergence detection (SYNC-02) + hum dict add (SYNC-04)
- [x] 10-PLAN-2.md — pipe propagation verify (SYNC-03) + like: detection (SYNC-01) + hum dict suggest (SYNC-05)

### Phase 11: Makepad GUI
**Goal**: `hum gui` opens a Makepad window with real-time spectral visualization, arrangement view, and transport controls — replacing the GPUI dashboard
**Depends on**: Phase 8
**Requirements**: MKPD-01, MKPD-02, MKPD-03, MKPD-04, MKPD-05, MKPD-06, MKPD-07, MKPD-08
**Success Criteria** (what must be TRUE):
  1. `hum gui` opens a Makepad window on WSL2 via OpenGL; the window renders at a stable framerate with no crash
  2. The spectral analyzer shows live FFT data from scsynth as a shader-rendered visualization that reacts to audio in real time
  3. The arrangement view shows all things as colored blocks on a horizontal timeline; blocks appear and disappear as at:/until: times are crossed during playback
  4. Clicking play/stop/seek in the GUI transport bar has the same effect as the equivalent `hum` CLI command
  5. Per-thing VU meters and waveform shapes update live; clicking a thing's lane solos or mutes it
**Plans**: 3 plans

Plans:
- [x] 11-PLAN-1.md — hum-gui binary, Makepad window, transport bar (MKPD-01, MKPD-05)
- [x] 11-PLAN-2.md — Arrangement view, VU meters, click-to-solo/mute (MKPD-03, MKPD-04, MKPD-06, MKPD-07, MKPD-08)
- [x] 11-PLAN-3.md — Spectral analyzer shader + FFT polling (MKPD-02)

### Phase 12: Creative Assistant
**Goal**: `hum suggest` and `hum analyze` give actionable compositional hints grounded in the piece's actual sounds and dictionary vocabulary
**Depends on**: Phase 9
**Requirements**: ASST-01, ASST-02, ASST-03
**Success Criteria** (what must be TRUE):
  1. `hum suggest` outputs at least one concrete structural hint (e.g. "ghost-machine and glass share reverb character — consider a shared stage effect")
  2. When dict vocabulary matches a thing's synth profile, suggestions name the dict term (e.g. "this matches 'haunted-echo' in your dictionary")
  3. `hum analyze` outputs a frequency balance assessment: which frequency bands are dominant, which are absent, and a concrete recommendation
**Plans**: 1 plan

Plans:
- [x] 12-PLAN-1.md — assistant module: suggest + analyze + CLI wiring + tests (ASST-01, ASST-02, ASST-03)

</details>

<details>
<summary>✅ v4.0 The Sound IDE (Phases 13-15) - SHIPPED 2026-03-22</summary>

### Phase 13: Sample Playback
**Goal**: Users can reference audio files in synth IR and hear them play back via scsynth buffers
**Depends on**: Phase 11
**Requirements**: SAMP-01, SAMP-02, SAMP-03, SAMP-04
**Success Criteria** (what must be TRUE):
  1. A thing with `sample: samples/kick.wav` in its `synth:` block plays the audio file on startup without error
  2. Editing the `sample:` field while the daemon is running causes the new file to load and play within ~1 second
  3. Looped and one-shot playback modes both work — the sample either repeats continuously or fires once per trigger
  4. Buffer reload fires when the .wav file on disk changes (hot-swap for sample content)
**Plans**: 2 plans

Plans:
- [x] 13-PLAN-1.md — SAMP parser + IR: `sample:` field, PlayBuf UGen, relative path resolution (SAMP-01, SAMP-02, SAMP-03)
- [x] 13-PLAN-2.md — Buffer management: load on startup, hot-reload on file change, buffer ID tracking (SAMP-04)

### Phase 14: MilkDrop Visualizer
**Goal**: The Makepad GUI displays a reactive audio visualizer driven by scsynth FFT data, with live shader editing
**Depends on**: Phase 13
**Requirements**: VIZ-01, VIZ-02, VIZ-03, VIZ-04, VIZ-05
**Success Criteria** (what must be TRUE):
  1. The visualizer pane renders animated patterns that visibly react to audio — silence produces a still frame, loud transients cause visible change
  2. At least four presets (waveform, spectrum bars, plasma, tunnel) are selectable from the GUI while audio plays
  3. Editing shader DSL code in the shader editor pane causes the visualization to update without restarting hum-gui
  4. Beat transients cause a perceptible visual "hit" — the reaction is distinct from steady-state FFT response
  5. Each active thing's amplitude drives a visually distinct element — louder things produce more prominent visual output
**Plans**: 3 plans

Plans:
- [x] 14-PLAN-1.md — Shader visualizer: FFT → uniform pipeline, waveform + spectrum presets (VIZ-01, VIZ-02)
- [x] 14-PLAN-2.md — Shader editor pane: live DSL edit → hot-reload visualization (VIZ-03)
- [x] 14-PLAN-3.md — Beat detection + per-thing amplitude routing (VIZ-04, VIZ-05)

### Phase 15: Sound IDE
**Goal**: HUM is a self-contained creative environment with embedded terminal, DAW keyboard shortcuts, and a configurable split-pane IDE layout
**Depends on**: Phase 14
**Requirements**: TERM-01, TERM-02, TERM-03, TERM-04, KEYS-01, KEYS-02, KEYS-03, KEYS-04, IDE-01, IDE-02, IDE-03, IDE-04
**Success Criteria** (what must be TRUE):
  1. A terminal pane opens inside the Makepad window running the user's default shell — Claude Code can be launched and used from it
  2. Pressing Escape toggles keyboard focus between the terminal and the GUI panes; spacebar play/stop and number-key solo/mute work when GUI has focus
  3. Panes (visualizer, arrangement, terminal) are resizable by dragging dividers; layout persists across restarts
  4. The project browser sidebar lists pieces, instruments, and dictionary files; clicking a file opens it in the external editor or terminal
  5. The status bar shows daemon connection state, playback state, and active thing count at all times; the Catppuccin Mocha theme is applied by default
**Plans**: 3 plans

Plans:
- [x] 15-PLAN-1.md — PTY terminal pane: embed in Makepad, default shell, copy/paste (TERM-01, TERM-02, TERM-03, TERM-04)
- [x] 15-PLAN-2.md — Keyboard management: DAW shortcuts, keybinding config, focus toggle, number-key solo (KEYS-01, KEYS-02, KEYS-03, KEYS-04)
- [x] 15-PLAN-3.md — IDE layout: split panes, project browser, status bar, theme system (IDE-01, IDE-02, IDE-03, IDE-04)

</details>

### 🚧 v5.0 Two Nodes Make Sound (In Progress)

**Milestone Goal:** Two fundsp oscillator nodes on a dark spatial canvas, draggable with mouse, stereo panning + proximity blending, running on Windows via cross-compilation from WSL2. Phase numbering resets to 1 for this milestone.

**Already proven:** Phase 01 (cross-compile): bare Makepad window cross-compiles and launches on Windows (2026-03-27)

## Phase Details

### Phase 1: Audio Core
**Goal**: Two oscillators producing distinct pitches through Windows speakers via a correct, allocation-free, sample-rate-negotiated cpal + fundsp pipeline
**Depends on**: Nothing (cross-compile toolchain proven in milestone pre-work)
**Requirements**: AUD-01, AUD-02, AUD-03, AUD-04
**Success Criteria** (what must be TRUE):
  1. Two sine tones at different pitches are audible through Windows speakers when the binary runs
  2. Sample rate is read from the WASAPI device config — no hardcoded rate anywhere in the audio path
  3. The cpal callback contains zero heap allocations (no Vec, String, Box, or Arc clone/drop)
  4. Makepad's UI thread is not blocked during audio initialization — the window opens before audio is ready
**Plans**: 2 plans

Plans:
- [x] 01-01-PLAN.md — Cargo deps, stub modules (nodes.rs, spatial.rs), full audio.rs implementation (AUD-02, AUD-03)
- [x] 01-02-PLAN.md — Wire audio into App struct via LiveHook + human verify two tones (AUD-01, AUD-04)

### Phase 2: Canvas + Cross-Compile
**Goal**: A dark Makepad canvas with two colored draggable circles, cross-compiled to a Windows .exe that opens and displays the canvas
**Depends on**: Phase 1
**Requirements**: CAN-01, CAN-02, CAN-03, CAN-04, BLD-01
**Success Criteria** (what must be TRUE):
  1. `cargo xwin build --target x86_64-pc-windows-msvc` completes without linker errors
  2. The .exe launches on Windows and shows a dark canvas filling the window
  3. Two circles appear at distinct starting positions with visually different colors
  4. Clicking and dragging a circle moves it freely anywhere on the canvas
  5. Releasing the mouse keeps the node at its dropped position across subsequent frames
**Plans**: 2 plans

Plans:
- [ ] 02-01-PLAN.md — CanvasWidget implementation: DrawNode shader, draw_walk, drag state machine (CAN-01, CAN-02, CAN-03, CAN-04)
- [ ] 02-02-PLAN.md — App wiring + cross-compile + human verify on Windows (BLD-01)

### Phase 3: Spatial Wiring
**Goal**: Dragging nodes changes the sound in real-time — left/right position controls stereo pan, node proximity controls blend, all transitions are click-free
**Depends on**: Phase 2
**Requirements**: SPA-01, SPA-02, SPA-03, SPA-04, SPA-05, BLD-02
**Success Criteria** (what must be TRUE):
  1. Dragging a node to the left edge pans its tone fully left; dragging to the right edge pans fully right
  2. A slow left-to-right drag produces a smooth, audibly continuous pan with no zipper noise
  3. Moving two nodes close together produces an audible blend between their tones
  4. Separating two nodes to opposite canvas edges isolates each tone to its own speaker with no bleed
  5. The .exe runs on Windows with both canvas and live audio output simultaneously — v5.0 is complete
**Plans**: TBD

## Progress

**Execution Order (v5.0):**
Phases execute in numeric order: 1 → 2 → 3

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. OSC Bridge | v1.0 | 2/2 | Complete | 2026-03-20 |
| 2. Parser + SCD Reader | v1.0 | 2/2 | Complete | 2026-03-20 |
| 3. State, Reconciler + File Watcher | v1.0 | 3/3 | Complete | 2026-03-20 |
| 4. Transport + E2E | v1.0 | 3/3 | Complete | 2026-03-20 |
| 5. Synth IR | v2.0 | 3/3 | Complete | 2026-03-22 |
| 6. Ref Resolution + Pipe Language | v2.0 | 3/3 | Complete | 2026-03-22 |
| 7. Instruments + Stage Effects | v2.0 | 2/2 | Complete | 2026-03-22 |
| 8. GPUI Dashboard + Transport Fix | v2.0 | 2/2 | Complete | 2026-03-22 |
| 9. Dictionary | v3.0 | 2/2 | Complete | 2026-03-22 |
| 10. Translation Sync | v3.0 | 2/2 | Complete | 2026-03-22 |
| 11. Makepad GUI | v3.0 | 3/3 | Complete | 2026-03-22 |
| 12. Creative Assistant | v3.0 | 1/1 | Complete | 2026-03-22 |
| 13. Sample Playback | v4.0 | 2/2 | Complete | 2026-03-22 |
| 14. MilkDrop Visualizer | v4.0 | 3/3 | Complete | 2026-03-22 |
| 15. Sound IDE | v4.0 | 3/3 | Complete | 2026-03-22 |
| 1. Audio Core | v5.0 | 0/2 | In progress | - |
| 2. Canvas + Cross-Compile | v5.0 | 0/? | Not started | - |
| 3. Spatial Wiring | v5.0 | 0/? | Not started | - |

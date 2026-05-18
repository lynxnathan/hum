# HUM Vision: v3 + v4

**Date:** 2026-03-22
**Status:** Vision capture

## v3: The Translation Pipeline + Dictionary

**Core idea:** The LLM and human learn a shared vocabulary. The dictionary maps vibes to synthesis. The translation is bidirectional — editing any layer updates the others.

### Features
- **hum.dict** — project + user-level vocabulary (laser → sine+fm, haunted → delay+reverb)
- **Bidirectional sync** — like: ←→ pipe: ←→ synth: stay in sync via LLM
- **Creative assistant mode** — LLM reads the piece + dict and suggests: "you need more nodes", "try a group effect here", "this instrument would work for that vibe"
- **Dictionary learning** — when human approves a sound, the mapping gets added to dict automatically
- **GPUI dashboard polish** — real VU meters (scsynth amplitude polling), waveform rendering, transport controls in GUI

### The Pipeline
```
Human: "add something that feels like underwater cathedral bells"
  ↓ LLM reads dict: "underwater" → lpf(200) + reverb(0.98), "cathedral" → reverb(room:0.99, mix:0.8), "bells" → instrument:glass-bell
  ↓ LLM writes to piece.hum:
    cathedral-bells:
      at: 45s
      like: underwater cathedral bells, deep reverb, sparse and holy
      instrument: glass-bell
      synth:
        filter: "lpf(cutoff: 200)"
        fx: "reverb(mix: 0.8, room: 0.99)"
      pipe: |
        cathedral-bells |> replicate(4) |> spread(-0.9~0.9) |> shift(i * 7)
  ↓ hum-rt compiles, plays
  ↓ Human: "yes but darker, like it's sinking"
  ↓ LLM updates dict: "sinking" → line(cutoff: 200→50, duration: 30s)
```

## v4: The Sound IDE

**Core idea:** HUM becomes a standalone creative environment — its own IDE for sound.

### Features
- **Zed terminal emulator integration** — run Claude Code inside the HUM window
- **Keyboard management** — DAW-style shortcuts (spacebar = play/stop, etc.)
- **Split view** — editor pane + visualizer pane + terminal pane
- **MilkDrop-style visualizer** — real-time custom shaders reacting to audio FFT
- **Shader editor** — edit GLSL/WGSL live, see visualization change in real-time
- **Project browser** — navigate pieces, instruments, dictionary

### MilkDrop Architecture
```
scsynth audio bus → FFT analysis (in Rust or SC) → uniforms
  ↓
Custom fragment shader (GLSL/WGSL):
  - uniform float bass, mids, highs, volume
  - uniform float time
  - uniform sampler2D waveform (1D texture of audio data)
  - uniform sampler2D spectrum (1D texture of FFT data)
  ↓
GPU renders to texture → composited in UI framework
```

### UI Framework Decision
- **Makepad** — purpose-built for creative coding, has shader DSL, live reload
- **GPUI** — Zed's framework, has terminal component, battle-tested
- **Hybrid** — GPUI for IDE chrome, raw wgpu for shader visualizer
- **Decision pending** — researching Makepad now

## Milestone Boundaries

| Milestone | Focus | Key Deliverable |
|-----------|-------|-----------------|
| v3.0 | Translation pipeline | hum.dict + bidirectional sync + creative assistant |
| v4.0 | Sound IDE | Terminal embed + MilkDrop visualizer + keyboard management |

---
*"I WANT MILKDROP AND CUSTOM SHADERS FOR REALTIME PLAYING TOO"*

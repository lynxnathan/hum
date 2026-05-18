# HUM

**Human Understanding of Music** — describe music in prose, hear it in under a second.

```
you ──prose──▶ LLM ──writes──▶ piece.hum + out/sc/*.scd ──▶ hum-rt ──OSC──▶ scsynth ──▶ speakers
                                                              ▲
                                                              │
                                                       inotify on every save
```

You talk to an LLM about music. It writes two artifacts: a `.hum` file (semantic YAML — your names, your intent) and SuperCollider SynthDef files (`.scd` — the actual sound recipes). A small Rust daemon watches both, diffs them against what `scsynth` is currently playing, and sends only the OSC delta. Edit a field, hear the change. No restart, no recompile.

## The bet

**Music intent should be a first-class artifact.** Your phrasing — `"space-crackle that builds from silence, like deftones - contact"` — is the source of truth. Not `synth_pad_01`.

Think of `.hum` as `Cargo.toml` for music (declarative intent) and `.scd` as `Cargo.lock` + compiled output (pinned, deterministic, local). The `.hum` travels between people; the compiled form belongs to your machine and your version of scsynth.

The deeper bet: **the human–LLM–machine loop can be short enough to feel like playing an instrument**. The LLM is a compiler from human language to SuperCollider. The daemon is a dumb-but-fast bridge with zero LLM dependency at runtime. You can swap LLMs, swap models, swap humans — the runtime doesn't care.

## What the LLM writes for you

You talk. The LLM transcribes intent into `piece.hum` — your names, your phrasing, your structure. Example:

```yaml
space-crackle:
  like: quiet crackling, staring at outer space
  ref: deftones - contact (the opening build)
  at: 0s
  does: builds from silence, never resolves
  where: wide

psychedelic-guitar:
  like: wah-wah guitar, psychedelic
  at: 10s
  does:
    - volume from very low to moderate over 5s
    - wah starts slow, gets rhythmic by 15s
  where: mono both channels

laser-ricochet:
  like: laser ricochet
  within: space-crackle
```

Every top-level key is a **thing** — a named sound component. Fields are optional. Unknowns are legal. Your names are the names. See [project.md](project.md) for the full grammar (`at`, `until`, `does`, `where`, `has`, `within`, `every`, `mood`, `ref`, `synth`, `pipe`, `style`, `instrument`, `sample`).

The LLM also writes the *sound recipes*. Two options, both authored by the model:

1. **Inline `synth:` blocks** inside the `.hum` file — compiled directly to scsynth SynthDefs by `hum-rt`, no `sclang` involved:

```yaml
kali-tandava:
  at: 0s
  like: the dance that ends and starts the world in the same step
  mood: everything at once
  synth:
    osc: "saw(detune: 0.05) + pulse(width: 0.02)"
    filter: "bpf(cutoff: 80~8000, q: 1~12)"
    distort: "tanh(drive: 1~12)"
    fx: "delay(time: 0.33~0.01, feedback: 0.8~0.2)"
    pan: "noise(rate: 0.01~4, range: -1~1)"
    amp: 0.01~0.20
```

A range like `80~8000` is a trajectory the engine will sweep, not a single value.

2. **Standalone `.scd` files** in `out/sc/` — full SuperCollider SynthDefs when inline IR isn't enough or when the model wants to escape into raw sclang. `hum-rt` discovers each `.scd` by filename and binds it to the matching thing in `piece.hum`. If both an inline `synth:` block and an `out/sc/<thing>.scd` exist for the same thing, the `.scd` wins — it's the escape hatch.

You can hand-edit either file if you want — they're plain text — but the design assumes the LLM does the writing and you do the listening.

## What you get

A daemon (`hum-rt`) and an optional Makepad-based GUI (`hum-gui`). The CLI surface:

```
hum play              # start playback
hum stop              # stop and free all synth nodes
hum play from 1m30s   # seek + play in one shot
hum loop 10s 20s      # loop between two points
hum solo <thing>      # mute everything else
hum mute <thing>      # mute one thing
hum status            # current time + active things
hum dict list         # show shared vocabulary
hum dict add <term>   # capture current thing's synth as a new dict entry
hum suggest           # compositional hints grounded in the piece
hum analyze           # frequency balance assessment
hum gui               # open the Makepad window
```

Editing `piece.hum` while playing updates the sound within ~1s. Editing a running `.scd` reloads the SynthDef and crossfades. Files under `/mnt/` (NTFS via WSL2) are handled via a `PollWatcher` fallback because inotify doesn't see them.

## What ships in this repo

Three binaries from one Cargo workspace:

| Binary | What it does |
|---|---|
| **`hum-rt`** | The daemon. Parses `.hum`, loads `.scd`, diffs state, sends OSC to scsynth, runs a Unix-socket transport server. Also the CLI client — `hum play` dispatches over that socket. |
| **`hum-gui`** | Makepad window: live spectral analyzer (shader-rendered FFT from scsynth), arrangement view (things as colored blocks on a timeline), per-thing VU meters, embedded PTY terminal (run Claude Code or any shell inside the window), MilkDrop-style visualizer with live shader editing, DAW keyboard shortcuts, Catppuccin Mocha theme. |
| **`ghostinstrument`** | A different bet — see below. |

## Ground tested

Four milestones shipped, fifteen phases. Each phase had explicit success criteria, was driven through a `discuss → research → plan → execute → verify` loop, and is sealed in `.planning/` for anyone who wants to see what was actually committed to versus skipped.

- **v1.0 MVP** — parser, OSC bridge, file watcher (with NTFS fallback), reconciler, timeline, transport CLI. The `edit-to-hear` loop works.
- **v2.0 Inline Synth IR + Pipe Language** — `synth:` blocks compile directly to scsynth SynthDefs without `sclang`. `ref:` for inheritance. `pipe:` for functional multi-voice composition (`replicate`, `each`, `shift`, `spread`, `take`, `repeat`). Instruments and stage effects (group buses + shared FX chains).
- **v3.0 Translation Pipeline + Makepad GUI** — `hum.dict` shared vocabulary; `style: laser` resolves to a real param set. Divergence detection across the `like:` / `pipe:` / `synth:` layers. Makepad replaces the earlier GPUI dashboard.
- **v4.0 The Sound IDE** — sample playback via scsynth buffers, MilkDrop visualizer with hot-reload shader editing, PTY terminal embedded in the Makepad window, full DAW shortcut surface, configurable split-pane layout. HUM became a self-contained creative environment.

Browse `.planning/phases/` for the per-phase context, plans, and verification reports. Planning artifacts are checked in on purpose — provenance matters more than tidy repo aesthetics.

## What's brewing — `ghostinstrument`

HUM proved that Rust + Makepad + real-time audio works. The v5.0 milestone takes the next swing: drop scsynth, drop YAML, drop the LLM-in-the-loop, and try a **spatial gestural** instrument instead.

> Two fundsp oscillators on a dark canvas. Drag them. Stereo pan follows horizontal position. Proximity blends them. 20ms input-to-sound ceiling. Single binary, cross-compiled WSL2 → Windows via `cargo-xwin`.

No timeline. No file format. No mixer. The canvas *is* the mixer. Future input devices (MIDI, gamepad, webcam, mic, phone sensors) bind to node positions or parameters. The performance is live.

Phase 1 (audio core, cpal+fundsp pipeline, sample-rate negotiation, allocation-free callback) is complete. Phase 2 (Makepad canvas + cross-compile + draggable nodes) and Phase 3 (spatial wiring) are next.

## Status

**This is a solo exploration, not a product.** v1–v4 work end-to-end on the author's WSL2 → Windows setup with a `scsynth` instance on the Windows host. It probably also works on Linux desktop with a local `scsynth`, and possibly macOS — neither is the daily driver here, so expect rough edges and please open an issue (or a PR) if you try it.

There is no installer. There are no release binaries. The README you are reading is the documentation. The `.planning/` directory is the changelog with receipts.

## Build & run

Prereqs:
- Rust (edition 2024, `rustc ≥ 1.85`)
- SuperCollider (`scsynth`) reachable on a UDP port — locally or on another host
- For `hum-gui`: a working Makepad install (Linux/macOS works out of the box; on WSL2 you'll want an X server or WSLg)
- For `ghostinstrument` Windows builds: `cargo-xwin` and the MSVC SDK headers it fetches

```bash
# daemon + CLI
cargo build --release --bin hum-rt

# Makepad GUI
cargo build --release --bin hum-gui --features hum-gui

# ghostinstrument (spatial canvas, v5 work-in-progress)
cargo build --release --bin ghostinstrument
```

Point `hum-rt` at your scsynth (default expects local `127.0.0.1:57110`):

```bash
export SCSYNTH_HOST=127.0.0.1:57110
./target/release/hum-rt &           # start daemon
./target/release/hum-rt play        # CLI dispatches over Unix socket
```

If you run scsynth on Windows from WSL2, `scripts/hum-server.sh start` will launch it via PowerShell and probe it with an OSC `/status` ping.

## Repo layout

```
src/
  main.rs                # hum-rt daemon + CLI client (one binary, two modes)
  parser/                # .hum YAML grammar (strict, deny_unknown_fields)
  scd/                   # .scd file discovery + SynthDef loading
  ir/                    # inline synth IR → SynthDef compiler + note sequencer
  pipe/                  # |> language: parser, types, executor
  reconciler.rs          # desired state vs actual state → OSC ops
  state.rs               # solo/mute, active things, transport state
  timeline.rs            # playback position, enter/exit at at:/until:
  watch.rs / watcher.rs  # inotify + NTFS PollWatcher fallback
  transport.rs           # Unix socket protocol + commands
  osc/bridge.rs          # rosc-based scsynth client
  dict.rs                # hum.dict — shared vocabulary store
  instruments.rs         # reusable instrument definitions
  stage.rs               # stage effects (group buses + fx chains)
  assistant.rs           # hum suggest / hum analyze
  bin/gui/               # hum-gui — Makepad shell, visualizer, terminal, arrangement
  bin/ghostinstrument/   # v5 spatial canvas
pieces/                  # example .hum pieces with compiled SynthDefs
out/sc/                  # default compile target for the LLM
.planning/               # the full design + execution trail (provenance)
```

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.

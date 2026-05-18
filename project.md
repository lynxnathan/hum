HUM — Human Understanding of Music

## What is this?

HUM is a music authoring system where:
1. A human describes music in natural language to an LLM
2. The LLM writes a `.hum` file (declarative, human-readable YAML describing musical intent)
3. The LLM also writes SuperCollider SynthDef files (`.scd`) that implement the sounds
4. A daemon (`hum-rt`) watches these files, diffs against current audio state, and sends OSC messages to `scsynth` (SuperCollider's audio server)
5. The human hears changes in near-real-time

The key insight: the `.hum` file captures WHAT you hear in your head (your names, your language, your intent). The `.scd` files are compiled output the LLM generates. `hum-rt` is a dumb file-watcher + OSC bridge with zero LLM dependency.

Think of it as: `.hum` is `Cargo.toml` (intent), `.scd` files are `Cargo.lock` + compiled output (pinned, deterministic). The HUM travels between people; the compiled form is local.

## Architecture

```
Claude Code (or any LLM)          hum-rt (our daemon)        scsynth (existing)
  - you talk                        - watches piece.hum        - receives OSC
  - it edits piece.hum              - watches out/sc/*.scd     - makes sound
  - it writes out/sc/*.scd          - diffs desired vs current
                                    - sends minimal OSC deltas
        │                                   │                        │
        │  file write                       │  OSC (UDP)             │
        ▼                                   ▼                        ▼
   piece.hum ──── inotify ────▶ hum-rt ──── OSC ────▶ scsynth ──▶ speakers
   out/sc/*.scd
```

Transport controls via unix socket:
```
$ hum play
$ hum stop  
$ hum play from 10s
$ hum loop 10s 20s
$ hum solo space-crackle
$ hum mute laser-ricochet
$ hum status
```

## The HUM format

A `.hum` file is YAML. Each top-level key is a named "thing" — a sound component.

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
  has:
    sparkle:
      like: bright sparkle finish
      where: center

laser-ricochet:
  like: laser ricochet
  within: space-crackle
```

### Fields (all optional):

| Field    | Purpose                                           | Examples                                      |
|----------|---------------------------------------------------|-----------------------------------------------|
| `like`   | What it sounds like. Free text. Primary input.    | `quiet crackling, like outer space`            |
| `ref`    | Cultural reference. Informational, not spec.      | `deftones - contact`                           |
| `at`     | When it enters. Seconds or relative.              | `0s`, `10s`, `after intro`, `bar 5`            |
| `until`  | When it exits.                                    | `30s`, `end of verse`, absent = open-ended     |
| `does`   | Trajectories. What changes over time.             | `builds from silence`, `volume low -> high`    |
| `where`  | Stereo placement.                                 | `wide`, `center`, `left`, `mono both`          |
| `has`    | Sub-components with own behavior/placement.       | Nested thing definitions                       |
| `within` | Contextual relationship to another thing.         | `within: space-crackle`                        |
| `every`  | Rhythmic pattern (when thing repeats).            | `every beat`, `every 2s`, `sporadic`           |
| `mood`   | Emotional context influencing LLM choices.        | `tense`, `playful`, `menacing`                 |

### Design principles:
- Your names are the names. "space-crackle" never becomes "synth_pad_01"
- Trajectories, not values. Most things are movements (from → to), not static
- Unknowns are legal. Absent fields = "I haven't decided" (Option<T>)
- Time is seconds until you add rhythm. No tempo assumed
- Same name appearing again = modification/override of existing thing

### Optional tempo/sections:

```yaml
groove:
  tempo: 92bpm
  feel: laid back

intro:
  from: 0s
  until: 30s

verse:
  after: intro
  bars: 8

flow: intro, verse, chorus, verse
```

## Project structure

```
project/
  piece.hum                  # source of truth (human intent)
  hum.lock                   # backend pin file
  out/
    sc/                      # supercollider backend
      space-crackle.scd      # compiled SynthDef
      psychedelic-guitar.scd
      laser-ricochet.scd
      timeline.scd           # orchestration (optional)
  .hum/
    turns/                   # conversation history (optional)
      001.md                 # "quiet crackling like outer space..."
      002.md                 # "add a laser ricochet"
```

### hum.lock
```yaml
backend: supercollider
backend_version: 3.13.0
compiler: claude-sonnet-4-20250514
compiled_at: 2026-03-20T14:32:00Z
synthdefs:
  space-crackle: blake3:a7f2...
  psychedelic-guitar: blake3:b3e1...
```

## hum-rt — what we're building

A Rust binary. Small, fast, no network dependencies, no LLM calls.

### Responsibilities:
1. **Parse** — read piece.hum via serde_yaml
2. **Diff** — compare desired state vs what scsynth is currently doing
   - Thing added → load SynthDef, instantiate
   - Thing removed → free synth node
   - Thing character changed → reload SynthDef, crossfade
   - Trajectory/timing changed → update automation
3. **OSC bridge** — talk to scsynth
   - `/d_load` — load SynthDef
   - `/s_new` — create synth instance
   - `/n_set` — update parameters
   - `/n_free` — remove synth
4. **Timeline** — track playback position, enter/exit things at right times
5. **Transport** — unix socket server for play/stop/seek/loop/solo/mute
6. **File watcher** — inotify/fswatch on piece.hum and out/sc/

### Key crates:
- `serde` + `serde_yaml` — HUM parsing
- `rosc` — OSC message construction and UDP send
- `notify` — cross-platform file watching
- `blake3` — checksums for lock file
- `tokio` or just threads — async file watching + socket server

## Phase 1 — Minimal viable loop

Goal: edit a .hum file, hear sound change.

### Step 1: scsynth OSC bridge
- [ ] Connect to scsynth via UDP (default port 57110)
- [ ] Send `/d_load` to load a SynthDef from .scd file
- [ ] Send `/s_new` to instantiate a synth
- [ ] Send `/n_set` to change parameters
- [ ] Send `/n_free` to remove a synth
- [ ] Verify: hand-write a simple .scd, load and play it from Rust

### Step 2: HUM parser
- [ ] Parse a minimal .hum file (thing name + at + basic fields)
- [ ] Internal representation of "desired state"
- [ ] Don't need to parse `like`/`ref` (that's LLM-facing, not runtime-facing)
- [ ] DO need: `at`, `until`, `does` (when parseable as trajectory), `where`

### Step 3: Compiled form reader
- [ ] Read .scd files from out/sc/
- [ ] Associate each .scd with a thing name from piece.hum
- [ ] Load SynthDefs into scsynth on startup

### Step 4: File watcher + diff
- [ ] Watch piece.hum and out/sc/ for changes
- [ ] On piece.hum change: re-parse, diff against current state
- [ ] On .scd change: reload that SynthDef, crossfade if playing

### Step 5: Timeline + transport
- [ ] Simple linear timeline in seconds
- [ ] Know which things are active at time T
- [ ] `hum play` / `hum stop` via unix socket
- [ ] Instantiate things as timeline reaches their `at:` time

### Step 6: First end-to-end test
- [ ] Hand-write a piece.hum with two things at different times
- [ ] Hand-write two .scd SynthDefs
- [ ] `hum play` → hear thing 1 start, then thing 2 enter
- [ ] Edit piece.hum (change `at:` time) → hear the change
- [ ] Edit .scd → hear the sound character change

After Phase 1: hook up Claude Code. It writes both .hum and .scd.
The spellbook opens.
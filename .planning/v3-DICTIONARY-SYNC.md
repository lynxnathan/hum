# v3: Bidirectional Translation + Shared Dictionary

**Date:** 2026-03-22
**Status:** Design captured
**Insight:** The .hum file has three layers that must stay in sync, and a project-level dictionary captures the shared vocabulary between human and LLM.

## The Bidirectional Loop

```
Human intent  ←──→  Pipe expression  ←──→  SynthDef IR
      ↑                                         |
      └──────── LLM keeps all three in sync ────┘
```

### Direction 1: Human → Pipe → SynthDef (composing)
1. Human writes `like: three laser berimbals doing the godfather theme`
2. LLM generates `pipe: glass |> replicate(3) |> shift(i * 4)`
3. LLM generates `synth: osc: sine + fm(ratio: 3.01), notes: [D4, D4, Eb4, D4]`
4. hum-rt compiles synth: to SCgf, plays it

### Direction 2: Pipe/SynthDef → Human (reading back)
1. Someone edits the pipe directly: adds `|> tempo(0.117)`
2. LLM reads the change, updates `like:` to include "3x faster"
3. The human description always reflects what you'd actually hear

### Direction 3: SynthDef → Pipe (refactoring)
1. Someone hand-tunes `synth: filter: lpf(cutoff: 400)` directly
2. LLM notices the change doesn't match the pipe output
3. LLM either updates the pipe or marks a divergence

## The Dictionary (hum.dict)

A project-level file that maps emotional/descriptive words to concrete synthesis parameters. Grows through collaboration.

### Format

```yaml
# hum.dict — shared vocabulary for this project/user

laser:
  synth:
    osc: "sine + fm(ratio: 3.01)"
  context: bright, cutting, sci-fi
  learned-from: glass (2026-03-20)

warm:
  synth:
    filter: "lpf(cutoff: 800)"
    osc-prefer: sine
  context: soft, intimate, analog
  learned-from: pulse (2026-03-20)

haunted-echo:
  synth:
    fx: "delay(time: 0.5, feedback: 0.7)"
    fx2: "reverb(room: 0.95)"
  context: eerie, distant, ghostly
  learned-from: ghost-machine (2026-03-20)

breathing:
  synth:
    env: "adsr(0.5, 0.1, 0.8, 1.0)"
    mod: "lfo(rate: 0.25)"
  context: alive, organic, pulsing
  learned-from: pulse (2026-03-20)

transcendental-whip:
  synth:
    osc: "noise(type: white)"
    env: "perc(0.0005, 0.02)"
    filter: "hpf(cutoff: 3000)"
  context: impact, spiritual, crack
  learned-from: bass-drop v5 (2026-03-20)

underground-beast:
  synth:
    osc: "noise(type: brown)"
    filter: "lpf(cutoff: 60)"
    mod: "line(0.15, 1.0, 180)"
  context: rising dread, subterranean, approaching
  learned-from: bass-drop (2026-03-20)

chainsaw-distant:
  synth:
    osc: "pulse(width: 0.1~0.5)"
    filter: "lpf(cutoff: 600~3000)"
    fx: "comb(delay: 0.3~0.8, decay: 6.0)"
  context: mechanical, eerie, mountain echo
  learned-from: ghost-machine (2026-03-20)
```

### Dictionary Hierarchy

```
~/.config/hum/global.dict    ← user's personal vocabulary (travels between projects)
./hum.dict                   ← project vocabulary (shared via git)
```

Global dict is your personal sonic fingerprint — when you say "warm" it always means YOUR warm.
Project dict is the shared language for collaborators on this piece.

### How the LLM Uses the Dictionary

**When writing (human → synth):**
1. LLM reads `like: haunted echo on a laser sine`
2. Looks up `haunted-echo` → delay + reverb, `laser` → sine + fm
3. Generates synth: block using those mappings
4. New terms get added to dict after user approves the sound

**When reading back (synth → human):**
1. LLM sees `synth: osc: sine + fm(ratio: 3.01)`
2. Looks up dict → matches "laser"
3. Can describe the sound to a new collaborator using the project's vocabulary

**When collaborating (git merge):**
1. Two people both add entries to hum.dict
2. git merge works naturally (YAML, line-per-entry)
3. Conflicting definitions for the same word → merge conflict → discussion (intentional!)

## Sync Protocol

When any layer changes, the LLM should:

```
on .hum file change:
  if like: changed →
    regenerate pipe: and synth: from new description + dict
  if pipe: changed →
    update synth: from pipe expansion
    update like: to describe what pipe now does
  if synth: changed directly →
    check if pipe: still matches
    if diverged → mark with comment: "# synth: manually tuned, pipe: may be stale"
    update like: to match actual synth params
```

The key insight: **divergence is OK**. Manual tuning is valid. The sync protocol detects and documents it rather than forcing everything to match.

## What hum-rt Needs

hum-rt itself stays LLM-agnostic. It reads whatever is in the file. The sync protocol is the LLM's responsibility — Claude Code (or any LLM) reads the dict and keeps layers in sync.

What hum-rt DOES need:
- Parse hum.dict on startup (for future: parameter presets, autocomplete in TUI)
- Expose dict entries via `hum dict list`, `hum dict show <term>`
- Watch hum.dict for changes (like piece.hum)

## Relationship to Existing Features

- **ref:** is thing-to-thing reuse (structural)
- **instrument:** is synth-definition reuse (sound design)
- **dict:** is vocabulary reuse (semantic) — maps WORDS to PARAMETERS
- **pipe:** is compositional reuse (functional)

Four kinds of reuse, four levels of abstraction. Each serves a different need.

---
*Captured from session discussion, 2026-03-22*
*"we are learning a shared vocabulary while building it piece by piece"*

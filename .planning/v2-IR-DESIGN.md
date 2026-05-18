# v2: Inline Synth IR Design

**Date:** 2026-03-20
**Status:** Design captured from jam session

## The Insight

The .hum file has two layers:
- **Human intent** (`like:`, `mood:`) — the conversation between human and LLM
- **Concrete IR** (`synth:`) — readable synthesis description that hum-rt compiles directly to OSC

This eliminates: sclang compilation, opaque .scsyndef binaries, the rename dance.
The `like:` is what you WANT. The `synth:` is what you GET. Both live in the same file.

## Synth Block Spec (Shallow)

```yaml
thing-name:
  at: 10s
  until: 60s
  like: human description for the LLM
  mood: emotional guidance
  ref: other-thing          # motif reuse
  synth:
    notes: [D4 D4 Eb4 D4]  # or ref(other-thing)
    osc: sine | saw | pulse(width: 0.5) | noise(type: white|brown|pink)
    osc2: sine(freq: ref * 3.01)   # layered oscillator
    filter: lpf(cutoff: 800) | hpf | bpf(freq: 2000, q: 0.3)
    env: perc(attack: 0.01, release: 0.5) | adsr(a, d, s, r)
    distort: tanh(drive: 2.0) | bitcrush(bits: 8)
    fx: reverb(mix: 0.7, room: 0.95) | delay(time: 0.3, feedback: 0.5)
    pan: center | noise(rate: 0.1, range: -0.5~0.5) | lfo(rate: 0.05)
    amp: 0.1
    tempo: 0.35s/note
```

## Motif Reuse via ref:

```yaml
glass:
  synth:
    notes: [D4 D4 D4 D4 Eb4 D4 C#4 D4]
    osc: sine + fm(ratio: 3.01)
    tempo: 0.35s/note

glass-drum:
  ref: glass
  like: same melody but percussive, 3x faster
  synth:
    notes: ref(glass)
    osc: noise(type: white)
    filter: bpf(q: 0.9)
    env: perc(0.001, 0.05)
    tempo: 0.117s/note
```

`ref:` = "I'm a variation of that thing"
`ref(thing)` inside synth = "pull that specific field"
Everything else overrides.

## Routing (Future)

```yaml
rain:
  duck-from: bass-drop     # sidechain compression

master:
  has:
    - rain
    - ghost-machine
  fx: compressor(threshold: -12db)
```

## Escape Hatch

Drop raw `.scd` files in `out/sc/` for anything the IR can't express.
hum-rt checks: if `synth:` block exists → compile inline. If `.scd` exists → use that. `.scd` wins on conflict (explicit override).

## What hum-rt Needs to Change

1. **IR parser** — parse `synth:` block into an intermediate representation
2. **Synth compiler** — IR → scsynth OSC messages (no sclang needed)
   - Map `osc: sine` → `SinOsc.ar`
   - Map `filter: lpf(cutoff: 800)` → `LPF.ar(sig, 800)`
   - Chain them: osc → filter → distort → fx → pan → out
3. **ref resolver** — resolve `ref(thing)` to pull fields from referenced thing
4. **Priority** — `out/sc/*.scd` overrides `synth:` block (escape hatch)

## Design Principles

- Music calls reuse "motif", not DRY
- The IR is readable enough to tweak by hand
- LLM writes both `like:` (intent) AND `synth:` (concrete) in one pass
- Human can read the `synth:` block and say "that freq is too low"
- Shallow IR covers 80% of cases. Escape hatch for the rest.

---
*Captured from: "Arabian Night of the Alien" jam session, 2026-03-20*

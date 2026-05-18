# v2: Pipe Language Design

**Date:** 2026-03-20
**Status:** Design sketch from jam session
**Inspiration:** Elixir pipes, LINQ, functional stream composition

## The Insight

HUM isn't just "YAML that describes synths". It's a sound programming language with four abstraction levels:

```
like:        "three laser berimbals"              ← human (LLM reads)
pipe:        glass |> replicate(3) |> shift(4)    ← programmer (compose transforms)
synth:       osc: sine, filter: lpf(800)          ← sound designer (synthesis params)
.scd:        SynthDef(\glass, { ... })             ← escape hatch (raw SuperCollider)
```

Pick your depth. LLM can write all four. Human reads whichever level speaks to them.

## Pipe Syntax

```yaml
thing-name:
  at: 20s
  pipe: |
    source_thing
    |> transform(args)
    |> transform(args)
```

### Stream Transforms

```
|> replicate(n)              # clone into n parallel voices
|> each(i => expr)           # apply per-voice with index
|> map(n => expr)            # transform each note/event
|> shift(semitones: 4)       # pitch shift
|> spread(pan: -0.8~0.8)    # distribute across stereo field
|> lag(time)                 # delay/smear signal
|> tremolo(rate, depth)      # amplitude modulation
|> tempo(duration/note)      # change playback speed
|> take(n)                   # first n notes
|> repeat(n)                 # loop n times
|> reverse                   # backwards
|> shuffle                   # randomize order
|> filter(lpf: 800)         # apply filter to stream
|> distort(tanh: 2.0)       # apply distortion
|> fx(reverb: 0.9)          # apply effect
```

### Examples

```yaml
# Three copies of glass, each shifted, spread across stereo
glass-swarm:
  at: 20s
  like: three copies of glass, each laser-shifted, with tremolo
  pipe: |
    glass
    |> replicate(3)
    |> each(i => shift(semitones: i * 4))
    |> spread(pan: -0.8~0.8)
    |> tremolo(rate: 3, depth: 0.4)

# Glass melody as berimbal drum hits, triple speed
glass-drum:
  at: 45s
  pipe: |
    glass.notes
    |> take(4)
    |> repeat(8)
    |> map(n => instrument("berimbal-amp", note: n))
    |> tempo(0.117s/note)
    |> each(i => pan(lerp(-1, 1, i/8)))

# Bass rhythm derived from melody
bass-pulse:
  at: 60s
  pipe: |
    glass.notes
    |> map(n => n - 24)
    |> tempo(0.7s/note)
    |> distort(tanh: 3.0)
    |> fx(reverb: 0.98)
```

## Instruments (Library)

```
hum/
  instruments/
    berimbal-amp.hum
    808-kit.hum
    laser-sine.hum
  pieces/
    arabian-night.hum
```

Instrument file:
```yaml
# instruments/berimbal-amp.hum
type: instrument
synth:
  sample: berimbal-hit.wav
  filter: bpf(freq: 800, q: 2.0)
  distort: amp-sim(drive: 4.0, cabinet: small)
  env: adsr(0.01, 0.1, 0.6, 0.3)
```

Used in piece:
```yaml
glass:
  instrument: berimbal-amp
  synth:
    notes: [D4 D4 Eb4 D4]
```

## Stage Effects (Group Processing)

```yaml
# Applied to a group of things, not individual
haunted-stage:
  type: stage
  applies-to: [ghost-machine, glass, bass-drop]
  fx: tremolo(rate: 3.0, depth: 0.6)
  fx: lag(time: 2s)
  fx: reverb(room: 0.95)
```

Maps to scsynth groups + effect nodes on a shared bus.

## Implementation Path

1. Pipe parser (Rust) — tokenize pipe expressions
2. Pipe evaluator — resolve refs, expand replicate/each/map
3. Pipe → synth IR compiler — pipes produce synth blocks
4. synth IR → OSC compiler (already designed)
5. Instrument loader — read from instruments/ directory
6. Stage → scsynth group routing

## Design Principles

- Pipes are functional composition over sound streams
- `|>` is the only operator you need to learn
- `each(i => ...)` gives you the index for per-voice variation
- `.notes` accessor lets you pull specific data from refs
- Instruments are reusable synth definitions (the "class")
- Stages are group-level effects (the "middleware")
- Everything composes: instrument + pipe + stage + synth overrides

---
*Captured from jam session, inspired by Elixir |> and LINQ*
*"josé valim is a friend, of course I like the pipe syntax"*

---
phase: 04-transport-e2e
plan: 3
type: execute
wave: 2
depends_on:
  - 04-PLAN-1.md
  - 04-PLAN-2.md
files_modified:
  - piece.hum
  - out/sc/drone.scd
  - out/sc/lead.scd
autonomous: false
requirements:
  - E2E-01
  - E2E-02
  - E2E-03

must_haves:
  truths:
    - "Multi-thing piece.hum plays each thing at the correct second"
    - "Editing piece.hum while playing changes what scsynth plays within ~1s"
    - "Editing .scd while playing hot-swaps the synth sound via crossfade"
    - "hum stop frees all nodes (scsynth is clean after stop)"
    - "hum status shows current position and active thing names"
  artifacts:
    - path: "piece.hum"
      provides: "Multi-thing test piece with staggered at: times"
      contains: "at: 0s"
    - path: "out/sc/drone.scd"
      provides: "Minimal SynthDef for drone thing"
    - path: "out/sc/lead.scd"
      provides: "Minimal SynthDef for lead thing"
  key_links:
    - from: "piece.hum"
      to: "out/sc/drone.scd"
      via: "thing name 'drone' maps to drone.scd by convention"
      pattern: "drone:"
    - from: "hum play"
      to: "daemon socket /tmp/hum.sock"
      via: "transport::send_cmd(TransportCmd::Play)"
      pattern: "hum.sock"
---

<objective>
Write the test piece, write minimal SynthDefs, then run through all three end-to-end scenarios with human verification. This is the final integration check for the entire hum-rt system.

Purpose: Confirm that the full stack — file watching, parsing, reconciling, OSC, and transport — all work together on real audio.

Output: Working piece.hum, two .scd files, and human confirmation that all three E2E scenarios pass.
</objective>

<execution_context>
@~/.eclusa/workflows/execute-plan.md
@~/.eclusa/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/phases/04-transport-e2e/04-CONTEXT.md
</context>

<tasks>

<task type="auto">
  <name>Task 1: Write test piece.hum and minimal SynthDefs</name>
  <files>piece.hum, out/sc/drone.scd, out/sc/lead.scd</files>
  <action>
Create `piece.hum` — a multi-thing piece with staggered times to verify E2E-01:

```yaml
drone:
  at: 0s
  like: sustained low sine tone, nearly silent

lead:
  at: 10s
  until: 30s
  like: brighter mid-range sine, gentle attack
```

Create `out/sc/drone.scd` — a SynthDef that scsynth can load. Must be a valid compiled SynthDef binary (the .scd extension is a misnomer in this project — these are raw SynthDef bytes from scsynth's /d_recv format). Since hum-rt reads these as raw bytes and sends to scsynth via `load_synthdef()`, the files need to contain valid SuperCollider SynthDef binary data.

**IMPORTANT**: If valid compiled SynthDef binaries do not exist and cannot be generated in this environment (WSL2, scsynth on Windows), then create placeholder files with a comment explaining what is needed, and note this in the verification checkpoint so the user can provide them.

Attempt to generate minimal SynthDefs using sclang if available:
```shell
# Check if sclang is available
which sclang 2>/dev/null && echo "available" || echo "not available"
```

If sclang is available, generate SynthDef binaries programmatically. If not, create README stubs in `out/sc/` explaining the format and what the user needs to provide.

Create `out/sc/` directory if it does not exist.

For the E2E test to work with real audio, the user will need actual compiled SynthDef files. The checkpoint task covers this.
  </action>
  <verify>
    <automated>cd ~/code/hum && ls out/sc/ && cat piece.hum</automated>
  </verify>
  <done>piece.hum exists with drone (at 0s) and lead (at 10s, until 30s). out/sc/ directory exists. Either real .scd binaries or placeholder stubs are present.</done>
</task>

<task type="checkpoint:human-verify" gate="blocking">
  <what-built>
Complete transport + E2E system:
- Unix socket server in hum-rt daemon at /tmp/hum.sock
- hum-rt CLI subcommands: play, stop, status, play from &lt;t&gt;, loop &lt;s&gt; &lt;e&gt;, solo &lt;thing&gt;, mute &lt;thing&gt;
- Solo/mute state persists across piece.hum file reloads
- Test piece.hum with drone (0s) and lead (10s-30s)
  </what-built>
  <how-to-verify>
Prerequisites: scsynth must be running (on Windows: SuperCollider → Server → Boot). SCSYNTH_HOST must be set if needed (e.g., export SCSYNTH_HOST=172.29.224.1:57110).

**E2E-01: Multi-thing timeline**
1. `cargo build` — confirm clean build
2. `./target/debug/hum-rt` in one terminal — confirm "event loop running"
3. `./target/debug/hum-rt play` in another terminal — confirm "Ack" response
4. Wait 0-9s: only drone should be audible
5. Wait until 10s: lead should start (confirm audible change)
6. Wait until 30s: lead should stop (drone continues)
7. `./target/debug/hum-rt status` — confirm pos, playing=true, active things shown

**E2E-02: Edit piece.hum while playing**
1. With daemon running and playing (from E2E-01), edit piece.hum
2. Change lead's `until: 30s` to `until: 60s` and save
3. Within ~1 second: lead should stay active past 30s (confirm audible)
4. Revert change (or edit at/until to different values) and confirm change takes effect

**E2E-03: Edit .scd while playing**
1. With drone playing, modify out/sc/drone.scd (if real SynthDef binary: use sclang to recompile with different frequency or amplitude)
2. Within ~1 second: sound character should change (crossfade/hot-swap)
3. Confirm no orphaned nodes by stopping: `./target/debug/hum-rt stop`
4. Confirm silence after stop

**Transport commands**
5. `./target/debug/hum-rt play from 25s` — confirm starts from 25s (lead active immediately)
6. `./target/debug/hum-rt loop 0s 15s` — confirm loops between 0 and 15s
7. `./target/debug/hum-rt solo drone` — confirm only drone audible
8. After `hum solo drone`, save piece.hum with any edit — confirm drone still soloed
9. `./target/debug/hum-rt mute drone` — confirm silence (drone muted)

Note: if out/sc/ has placeholder stubs instead of real SynthDef binaries, E2E-03 requires the user to provide compiled SynthDef files first. E2E-01 and E2E-02 can still be tested if SynthDefs are loadable.
  </how-to-verify>
  <resume-signal>Type "approved" if all three E2E scenarios pass, or describe which scenarios passed and which issues remain.</resume-signal>
</task>

</tasks>

<verification>
Human confirms all three E2E scenarios pass:
- E2E-01: Multi-thing timeline plays each thing at correct moment
- E2E-02: Editing piece.hum changes sound within ~1s
- E2E-03: Editing .scd hot-swaps sound via crossfade
And all transport commands (play, stop, status, seek, loop, solo, mute) produce correct behavior.
</verification>

<success_criteria>
1. hum play starts playback; hum stop stops and frees all nodes; hum status shows pos and active things
2. hum solo &lt;thing&gt; mutes all others, survives file reload; hum mute &lt;thing&gt; mutes that thing, survives reload
3. hum play from &lt;time&gt; seeks and starts playing from that position
4. hum loop &lt;start&gt; &lt;end&gt; loops continuously between two points
5. Multi-thing timeline plays each thing at correct moment (E2E-01)
6. Editing piece.hum while playing changes sound within ~1s (E2E-02)
7. Editing .scd while playing changes sound via crossfade (E2E-03)
</success_criteria>

<output>
After completion, create `.planning/phases/04-transport-e2e/04-3-SUMMARY.md` using the summary template.
</output>

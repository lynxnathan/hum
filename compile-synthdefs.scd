// HUM SynthDef compiler — run this in SuperCollider IDE
// Generates .scsyndef files for each thing in piece.hum

(
var outDir = thisProcess.platform.userAppSupportDir +/+ "synthdefs";

// rain: filtered white noise, like rain on glass
SynthDef(\rain, { |amp = 0.08|
    var noise = WhiteNoise.ar(1);
    var filtered = BPF.ar(noise, LFNoise1.kr(0.3).range(2000, 6000), 0.4);
    var drops = Dust.ar(80) * 0.3;
    var sig = filtered + (BPF.ar(drops, 4000, 0.1) * 0.5);
    Out.ar(0, sig.dup * amp);
}).writeDefFile(outDir);

// pulse: slow breathing sine
SynthDef(\pulse, { |freq = 110, amp = 0.15|
    var breathe = SinOsc.kr(0.25).range(0.0, 1.0); // 4 sec cycle
    var sig = SinOsc.ar(freq) * breathe * amp;
    sig = sig + (SinOsc.ar(freq * 2.01) * breathe * amp * 0.3);
    Out.ar(0, sig.dup);
}).writeDefFile(outDir);

// glass: crystalline pentatonic bells
SynthDef(\glass, { |amp = 0.06|
    var trig = Dust.kr(1.5); // sparse triggers
    var notes = [72, 74, 76, 79, 81, 84, 86]; // C pentatonic, high
    var midi = Select.kr(TIRand.kr(0, notes.size - 1, trig), notes);
    var freq = midi.midicps;
    var sig = SinOsc.ar(freq) * EnvGen.ar(Env.perc(0.01, 2.5), trig) * amp;
    sig = sig + (SinOsc.ar(freq * 2.001) * EnvGen.ar(Env.perc(0.01, 1.5), trig) * amp * 0.4);
    sig = FreeVerb.ar(sig, 0.7, 0.9, 0.3);
    Out.ar(0, sig.dup);
}).writeDefFile(outDir);

// drift: massive detuned saw chord, slowly shifting
SynthDef(\drift, { |amp = 0.07|
    var base = LFSaw.kr(0.02).range(40, 48).midicps; // slow drift
    var sig = Mix.fill(5, { |i|
        var detune = LFNoise1.kr(0.1).range(-3, 3);
        Saw.ar(base * (1 + (i * 0.005)) + detune) * 0.2;
    });
    sig = LPF.ar(sig, 800);
    sig = FreeVerb.ar(sig, 0.6, 0.95, 0.4);
    Out.ar(0, sig.dup * amp);
}).writeDefFile(outDir);

"Done! Compiled: rain, pulse, glass, drift".postln;
("Output dir: " ++ outDir).postln;
)

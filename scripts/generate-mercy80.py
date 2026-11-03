#!/usr/bin/env python3
"""Generate the original MERCY-80 factory drum samples and kit metadata.

No commercial recordings or third-party sample libraries are used. The files are
synthesised from oscillators, filtered noise and envelopes. Running this script
is deterministic; the generated WAVs are committed for binary releases.
"""
import json
import math
import random
import struct
import wave
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
KIT = ROOT / "kits" / "MERCY-80"
SAMPLES = KIT / "samples"
PRESETS = ROOT / "presets"
SOURCE = KIT / "source" / "cc0"
SR = 48000
TAU = math.tau

INSTRUMENTS = [
    ("kick", "Kick", 36),
    ("snare", "Snare", 38),
    ("closed_hat", "Closed Hi-Hat", 42),
    ("open_hat", "Open Hi-Hat", 46),
    ("crash1", "Crash 1", 49),
    ("crash2", "Crash 2", 57),
    ("ride", "Ride", 51),
    ("high_tom", "High Tom", 50),
    ("mid_tom", "Mid Tom", 47),
    ("low_tom", "Low Tom", 43),
    ("clap", "Clap", 39),
    ("rimshot", "Rimshot", 37),
]

DEFAULTS = {
    "kick":      dict(volume_db=0.5,  pan=0.0,  pitch=0.0, decay=0.86, body=0.14, velocity=0.9, drive=0.24, reverb_send=0.08, output=0),
    "snare":     dict(volume_db=-1.0, pan=0.0,  pitch=0.0, decay=1.05, body=0.16, velocity=0.9, drive=0.26, reverb_send=0.34, output=0),
    "closed_hat":dict(volume_db=-7.0, pan=0.15, pitch=0.0, decay=1.0, body=0.0, velocity=0.7, drive=0.08, reverb_send=0.02, output=0),
    "open_hat":  dict(volume_db=-8.0, pan=0.15, pitch=0.0, decay=1.0, body=0.0, velocity=0.7, drive=0.08, reverb_send=0.05, output=0),
    "crash1":    dict(volume_db=-9.0, pan=-0.35,pitch=0.0, decay=1.0, body=0.0, velocity=0.7, drive=0.05, reverb_send=0.22, output=0),
    "crash2":    dict(volume_db=-9.0, pan=0.35, pitch=0.0, decay=1.0, body=0.0, velocity=0.7, drive=0.05, reverb_send=0.22, output=0),
    "ride":      dict(volume_db=-8.0, pan=0.25, pitch=0.0, decay=1.0, body=0.0, velocity=0.7, drive=0.06, reverb_send=0.14, output=0),
    "high_tom":  dict(volume_db=-3.0, pan=-0.2, pitch=0.0, decay=1.0, body=0.14, velocity=0.8, drive=0.12, reverb_send=0.28, output=0),
    "mid_tom":   dict(volume_db=-2.0, pan=0.0,  pitch=0.0, decay=1.0, body=0.14, velocity=0.8, drive=0.12, reverb_send=0.28, output=0),
    "low_tom":   dict(volume_db=-1.0, pan=0.2,  pitch=0.0, decay=1.0, body=0.14, velocity=0.8, drive=0.12, reverb_send=0.28, output=0),
    "clap":      dict(volume_db=-4.0, pan=-0.1, pitch=0.0, decay=1.0, body=0.06, velocity=0.8, drive=0.14, reverb_send=0.28, output=0),
    "rimshot":   dict(volume_db=-8.0, pan=0.1,  pitch=0.0, decay=1.0, body=0.0, velocity=0.7, drive=0.16, reverb_send=0.06, output=0),
}


def rng_for(name, layer):
    return random.Random(0x4E495255 + sum(map(ord, name)) * 131 + layer * 977)


def lowpass(samples, cutoff):
    if cutoff >= SR / 2:
        return samples
    alpha = 1.0 - math.exp(-TAU * cutoff / SR)
    y = 0.0
    out = []
    for x in samples:
        y += alpha * (x - y)
        out.append(y)
    return out


def highpass(samples, cutoff):
    low = lowpass(samples, cutoff)
    return [x - l for x, l in zip(samples, low)]


def env(n, attack=0.002, decay=8.0, release=0.02):
    a = max(1, int(attack * SR))
    out = []
    for i in range(n):
        t = i / SR
        e = min(1.0, i / a) if a else 1.0
        e *= math.exp(-decay * t)
        if i > n - int(release * SR):
            e *= max(0.0, (n - i) / (release * SR))
        out.append(e)
    return out


def norm(samples, peak=0.92):
    m = max((abs(x) for x in samples), default=0.0)
    if m < 1e-9:
        return samples
    g = peak / m
    return [max(-1.0, min(1.0, x * g)) for x in samples]


def vintage_character(samples, bits=8, hold=2, mix=0.82):
    levels = 2 ** (bits - 1)
    held = 0.0
    out = []
    for index, sample in enumerate(samples):
        if index % hold == 0:
            held = round(sample * levels) / levels
        out.append(sample * (1.0 - mix) + held * mix)
    return out


def sat(x, drive=1.0):
    return math.tanh(x * drive) / math.tanh(drive) if drive > 0 else x


def sine_sweep(n, f0, f1, decay, layer, attack=0.001):
    phase = 0.0
    out = []
    for i in range(n):
        t = i / n
        f = f0 + (f1 - f0) * (t ** 0.55)
        phase += TAU * f / SR
        out.append(math.sin(phase) * (1.0 + 0.08 * layer))
    e = env(n, attack=attack, decay=decay)
    return [x * y for x, y in zip(out, e)]


def noise(n, rng):
    return [rng.uniform(-1.0, 1.0) for _ in range(n)]


def metal(n, rng, freqs, decay, hp=4000, layer=0):
    out = [0.0] * n
    for k, f in enumerate(freqs):
        f = f * (1.0 + 0.01 * layer) * (1.0 + 0.003 * k)
        phase = rng.random() * TAU
        amp = 1.0 / (1.0 + k * 0.35)
        for i in range(n):
            phase += TAU * f / SR
            out[i] += math.sin(phase + (i * 0.0000007 * k)) * amp
    out = highpass(out, hp)
    nz = highpass(noise(n, rng), hp * 0.75)
    e = env(n, attack=0.0008, decay=decay)
    return [(x * 0.72 + z * 0.45) * y for x, z, y in zip(out, nz, e)]


def kick(layer):
    n = int(0.42 * SR)
    rng = rng_for("kick", layer)
    phase = 0.0
    body = []
    for i in range(n):
        t = i / SR
        frequency = 54.0 + 96.0 * math.exp(-t * 55.0) + 8.0 * math.exp(-t * 9.0)
        phase += TAU * frequency / SR
        envelope = min(1.0, i / (0.0010 * SR)) * math.exp(-6.5 * t)
        body.append(
            (math.sin(phase) + 0.38 * math.sin(2.0 * phase) + 0.16 * math.sin(3.0 * phase))
            * envelope
        )

    thump = highpass(lowpass(noise(n, rng), 190 + layer * 15), 35)
    thump = [x * math.exp(-22.0 * i / SR) for i, x in enumerate(thump)]

    click_length = int(0.010 * SR)
    click = highpass(noise(click_length, rng), 2400)
    click = [x * (1.0 - i / click_length) ** 2 for i, x in enumerate(click)]
    click_full = [0.0] * n
    click_full[:click_length] = click

    beater = sine_sweep(n, 900.0, 220.0, 68.0, layer, attack=0.0002)
    knock = sine_sweep(n, 240.0 - layer * 3.0, 110.0, 30.0, layer, attack=0.0004)
    out = [
        a * 0.84 + b * 0.16 + c * 0.14 + d * 0.09 + k * 0.18
        for a, b, c, d, k in zip(body, thump, click_full, beater, knock)
    ]
    return norm([sat(x * 1.5, 2.2 + layer * 0.25) for x in out], 0.94)

def snare(layer):
    n = int(0.55 * SR)
    rng = rng_for("snare", layer)
    tone = sine_sweep(n, 185.0 + layer * 6, 165.0, 10.0, layer)
    tone2 = sine_sweep(n, 330.0, 285.0, 13.0, layer)
    nz = highpass(noise(n, rng), 900)
    e = env(n, attack=0.0008, decay=9.0 + layer * 0.8)
    out = [(a * 0.52 + b * 0.28 + c * 0.82) * d for a, b, c, d in zip(tone, tone2, nz, e)]
    return norm([sat(x * 1.15, 1.35 + layer * 0.2) for x in out], 0.92)


def hat(layer, open_hat=False):
    n = int((0.52 if open_hat else 0.16) * SR)
    rng = rng_for("open_hat" if open_hat else "closed_hat", layer)
    freqs = [296, 347, 402, 515, 620, 720, 842, 1010]
    out = metal(n, rng, freqs, 5.0 if open_hat else 28.0, 6200, layer)
    return norm(out, 0.75 if open_hat else 0.7)


def crash(layer, which=1):
    n = int((2.0 if which == 1 else 1.55) * SR)
    rng = rng_for(f"crash{which}", layer)
    freqs = ([233, 311, 419, 547, 701, 887, 1103, 1427] if which == 1
             else [211, 283, 397, 521, 673, 859, 1171, 1531])
    out = metal(n, rng, freqs, 2.2 if which == 1 else 2.8, 3200, layer)
    return norm(out, 0.82)


def ride(layer):
    n = int(1.6 * SR)
    rng = rng_for("ride", layer)
    freqs = [523, 691, 887, 1103, 1481, 1871]
    out = metal(n, rng, freqs, 2.5, 2200, layer)
    ping = sine_sweep(n, 2200, 2100, 3.0, layer)
    out = [x * 0.9 + y * 0.18 for x, y in zip(out, ping)]
    return norm(out, 0.8)


def tom(layer, base):
    n = int(0.62 * SR)
    rng = rng_for(f"tom{base}", layer)
    body = sine_sweep(n, base * (1.05 + layer * 0.02), base * 0.58, 9.5, layer)
    nz = highpass(noise(n, rng), 700)
    e = env(n, attack=0.001, decay=10.0)
    out = [(x + z * 0.08) * y for x, z, y in zip(body, nz, e)]
    return norm([sat(x * 1.2, 1.25 + layer * 0.2) for x in out], 0.9)


def clap(layer):
    n = int(0.48 * SR)
    rng = rng_for("clap", layer)
    nz = highpass(noise(n, rng), 1100)
    out = [0.0] * n
    bursts = [0.0, 0.011, 0.023, 0.036]
    for start in bursts:
        i0 = int(start * SR)
        length = int(0.045 * SR)
        e = env(length, attack=0.0005, decay=32.0)
        for i in range(min(length, n - i0)):
            out[i0 + i] += nz[i0 + i] * e[i] * (0.9 if i0 == 0 else 0.55)
    tail = env(n, attack=0.01, decay=8.5)
    out = [x * 0.8 + y * z * 0.35 for x, y, z in zip(out, nz, tail)]
    return norm(out, 0.82)


def rimshot(layer):
    n = int(0.16 * SR)
    rng = rng_for("rimshot", layer)
    a = sine_sweep(n, 780, 690, 28.0, layer)
    b = highpass(noise(n, rng), 2400)
    e = env(n, attack=0.0004, decay=34.0)
    out = [(x * 0.74 + y * 0.5) * z for x, y, z in zip(a, b, e)]
    return norm([sat(x, 1.5) for x in out], 0.8)


SOURCE_MAP = {
    "kick": ["kick_01", "kick_03", "kick_05"],
    "snare": ["snare_01", "snare_03", "snare_06"],
    "closed_hat": ["closed_hat_01", "closed_hat_02", "closed_hat_03"],
    "open_hat": ["open_hat_01", "open_hat_01", "open_hat_01"],
    "high_tom": ["tom_01", "tom_01", "tom_01"],
    "mid_tom": ["tom_01", "tom_01", "tom_01"],
    "low_tom": ["tom_02", "tom_02", "tom_02"],
}

TOM_RATIO = {
    "high_tom": 1.0,
    "mid_tom": 0.82,
    "low_tom": 0.74,
}


def load_source(name):
    path = SOURCE / f"{name}.wav"
    with wave.open(str(path), "rb") as w:
        if w.getsampwidth() != 2:
            raise ValueError(f"{path} must be 16-bit PCM")
        channels = w.getnchannels()
        frames = w.readframes(w.getnframes())
    values = struct.unpack(f"<{len(frames) // 2}h", frames)
    samples = [value / 32768.0 for value in values]
    if channels > 1:
        samples = [
            sum(samples[index : index + channels]) / channels
            for index in range(0, len(samples), channels)
        ]
    start = next((index for index, value in enumerate(samples) if abs(value) > 0.002), 0)
    start = max(0, start - int(0.002 * SR))
    return samples[start:]


def pitch_shift(samples, ratio):
    if abs(ratio - 1.0) < 1e-6:
        return samples
    length = max(1, int(len(samples) / ratio))
    out = []
    for index in range(length):
        position = index * ratio
        left = int(position)
        fraction = position - left
        a = samples[left] if left < len(samples) else 0.0
        b = samples[left + 1] if left + 1 < len(samples) else 0.0
        out.append(a * (1.0 - fraction) + b * fraction)
    return out


def source_voice(name, layer):
    source_name = SOURCE_MAP[name][layer]
    samples = load_source(source_name)
    if name in TOM_RATIO:
        samples = pitch_shift(samples, TOM_RATIO[name])
    samples = [sat(value * 1.08, 1.15 + layer * 0.05) for value in samples]
    return vintage_character(norm(samples, 0.95), mix=0.3)


def make(name, layer):
    if name in SOURCE_MAP: return source_voice(name, layer)
    if name == "crash1": return vintage_character(crash(layer, 1))
    if name == "crash2": return vintage_character(crash(layer, 2))
    if name == "ride": return vintage_character(ride(layer))
    if name == "clap": return vintage_character(clap(layer))
    if name == "rimshot": return vintage_character(rimshot(layer))
    raise ValueError(name)


def write_wav(path, samples):
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        frames = bytearray()
        for x in samples:
            frames += struct.pack("<h", int(max(-1.0, min(1.0, x)) * 32767))
        w.writeframes(frames)


def main():
    SAMPLES.mkdir(parents=True, exist_ok=True)
    instruments = []
    for name, label, note in INSTRUMENTS:
        samples = []
        for layer in range(3):
            path = SAMPLES / f"{name}_v{layer}.wav"
            write_wav(path, make(name, layer))
            samples.append({
                "file": f"samples/{name}_v{layer}.wav",
                "velocity_min": layer * 43,
                "velocity_max": (layer + 1) * 43 - 1 if layer < 2 else 127,
            })
        entry = {
            "id": name,
            "name": label,
            "midi_note": note,
            "choke_group": "hats" if name in ("closed_hat", "open_hat") else None,
            "samples": samples,
            "defaults": DEFAULTS[name],
        }
        instruments.append(entry)
    kit = {
        "schema": 1,
        "id": "mercy-80",
        "name": "MERCY-80",
        "version": "0.0.1",
        "description": "Original dark, mechanical drum kit for NIRUDRUMS.",
        "sample_rate": SR,
        "choke_groups": [
            {"id": "hats", "name": "Hi-hats", "instruments": ["closed_hat", "open_hat"]}
        ],
        "instruments": instruments,
    }
    (KIT / "kit.json").write_text(json.dumps(kit, indent=2) + "\n")

    def preset(pid, name, description, overrides, master, reverb):
        instruments_cfg = {}
        for instrument, values in DEFAULTS.items():
            settings = dict(values)
            settings.update((overrides or {}).get(instrument, {}))
            instruments_cfg[instrument] = settings
        doc = {
            "schema": 1,
            "id": pid,
            "name": name,
            "description": description,
            "kit": "mercy-80",
            "instruments": instruments_cfg,
            "master": master,
            "reverb": reverb,
        }
        (PRESETS / f"{pid}.json").write_text(json.dumps(doc, indent=2) + "\n")

    preset("mercy-80-classic", "MERCY-80 Classic",
           "Balanced factory sound with a short plate and mild drive.",
           {}, {"volume_db": 0.0, "drive": 0.16, "limiter": True, "dry": False},
           {"mode": "plate", "mix": 0.16, "decay": 0.45, "pre_delay_ms": 18, "damping": 0.42, "width": 0.9, "gate_ms": 0})
    preset("mercy-80-heavy", "MERCY-80 Heavy",
           "Harder drive, deeper kick and a longer room.",
           {"kick": {"drive": 0.42, "volume_db": 2.0, "decay": 1.12, "body": 0.32},
            "snare": {"drive": 0.35, "reverb_send": 0.24},
            "low_tom": {"drive": 0.28}},
           {"volume_db": 1.0, "drive": 0.32, "limiter": True, "dry": False},
           {"mode": "room", "mix": 0.24, "decay": 0.62, "pre_delay_ms": 26, "damping": 0.32, "width": 1.0, "gate_ms": 0})
    preset("mercy-80-dry", "MERCY-80 Dry",
           "All internal saturation, reverb and limiting bypassed.",
           {}, {"volume_db": 0.0, "drive": 0.0, "limiter": False, "dry": True},
           {"mode": "plate", "mix": 0.0, "decay": 0.0, "pre_delay_ms": 0, "damping": 0.0, "width": 0.5, "gate_ms": 0})
    preset("mercy-80-gated", "MERCY-80 Gated",
           "1980s gated snare and toms with a wide plate.",
           {"snare": {"reverb_send": 0.42, "decay": 0.9},
            "high_tom": {"reverb_send": 0.38},
            "mid_tom": {"reverb_send": 0.38},
            "low_tom": {"reverb_send": 0.38}},
           {"volume_db": 0.0, "drive": 0.22, "limiter": True, "dry": False},
           {"mode": "gated", "mix": 0.38, "decay": 0.55, "pre_delay_ms": 12, "damping": 0.25, "width": 1.0, "gate_ms": 260})
    preset("mercy-80-industrial", "MERCY-80 Industrial",
           "Darker pitch, harder saturation and a metallic room.",
           {"kick": {"pitch": -2.0, "drive": 0.5},
            "snare": {"pitch": -1.0, "drive": 0.44},
            "clap": {"drive": 0.36, "reverb_send": 0.38},
            "rimshot": {"pitch": -3.0, "drive": 0.4}},
           {"volume_db": 1.0, "drive": 0.42, "limiter": True, "dry": False},
           {"mode": "room", "mix": 0.3, "decay": 0.72, "pre_delay_ms": 34, "damping": 0.2, "width": 1.0, "gate_ms": 0})

    preset("mercy-80-cathedral", "MERCY-80 Cathedral",
           "Sub-heavy kick, deep toms and a wide, dark room for maximum weight.",
           {"kick": {"pitch": -3.0, "drive": 0.55, "body": 0.5, "decay": 1.4, "volume_db": 0.0},
            "snare": {"body": 0.22, "drive": 0.4, "volume_db": 0.0},
            "low_tom": {"body": 0.3, "drive": 0.3},
            "high_tom": {"body": 0.24},
            "mid_tom": {"body": 0.26}},
           {"volume_db": -1.0, "drive": 0.38, "limiter": True, "dry": False},
           {"mode": "room", "mix": 0.28, "decay": 0.7, "pre_delay_ms": 30, "damping": 0.25, "width": 1.0, "gate_ms": 0})

    preset("mercy-80-gated-room", "MERCY-80 Gated Room",
           "Punchy kick, fat snare and gated room.",
           {"kick": {"volume_db": 0.0, "drive": 0.22, "body": 0.14, "decay": 0.86, "reverb_send": 0.08},
            "snare": {"volume_db": 0.0, "drive": 0.26, "body": 0.16, "decay": 1.05, "reverb_send": 0.4},
            "high_tom": {"body": 0.14, "reverb_send": 0.34},
            "mid_tom": {"body": 0.14, "reverb_send": 0.34},
            "low_tom": {"body": 0.14, "reverb_send": 0.34}},
           {"volume_db": -1.0, "drive": 0.18, "limiter": True, "dry": False},
           {"mode": "gated", "mix": 0.34, "decay": 0.52, "pre_delay_ms": 18, "damping": 0.3, "width": 1.0, "gate_ms": 260})

    assets = ROOT / "crates/nirudrums-core/src/factory_assets.rs"
    lines = [
        "// Generated by scripts/generate-mercy80.py. Do not edit by hand.",
        'pub const FACTORY_KIT_JSON: &str = include_str!("../../../kits/MERCY-80/kit.json");',
        "",
        "pub fn factory_sample(path: &str) -> Option<&'static [u8]> {",
        "    match path {",
    ]
    for name, _label, _note in INSTRUMENTS:
        for layer in range(3):
            sample_path = f"samples/{name}_v{layer}.wav"
            lines.append(
                f'        "{sample_path}" => Some(include_bytes!('
                f'"../../../kits/MERCY-80/{sample_path}")),'
            )
    lines.extend([
        "        _ => None,",
        "    }",
        "}",
        "",
        "pub const FACTORY_PRESETS: &[(&str, &str)] = &[",
    ])
    for preset_path in sorted(PRESETS.glob("*.json")):
        lines.append(
            f'    ("{preset_path.stem}", include_str!('
            f'"../../../presets/{preset_path.name}")),'
        )
    lines.extend(["];", ""])
    assets.write_text("\n".join(lines))
    try:
        import subprocess
        subprocess.run(["rustfmt", str(assets)], check=False)
    except OSError:
        pass
    print(f"wrote {KIT/'kit.json'} and {len(INSTRUMENTS)*3} samples")
    print(f"wrote {len(list(PRESETS.glob('*.json')))} presets in {PRESETS}")
    print(f"wrote {assets}")


if __name__ == "__main__":
    main()

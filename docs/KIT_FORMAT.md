# NIRUDRUMS kit format v1

A kit is a directory containing `kit.json` and one or more WAV files.

```text
my-kit/
  kit.json
  samples/
    kick_soft.wav
    kick_medium.wav
    kick_hard.wav
    ...
```

## kit.json

```json
{
  "schema": 1,
  "id": "my-kit",
  "name": "My Kit",
  "version": "1.0.0",
  "description": "Short description",
  "sample_rate": 48000,
  "choke_groups": [{ "id": "hihat" }],
  "instruments": [
    {
      "id": "kick",
      "name": "Kick",
      "midi_note": 36,
      "choke_group": null,
      "samples": [
        { "file": "samples/kick_soft.wav", "velocity_min": 0, "velocity_max": 42 },
        { "file": "samples/kick_medium.wav", "velocity_min": 43, "velocity_max": 85 },
        { "file": "samples/kick_hard.wav", "velocity_min": 86, "velocity_max": 127 }
      ],
      "defaults": {
        "volume_db": 1.0,
        "pan": 0.0,
        "pitch": 0.0,
        "decay": 1.0,
        "body": 0.22,
        "velocity": 0.9,
        "drive": 0.22,
        "reverb_send": 0.04,
        "mute": false,
        "solo": false,
        "output": 0
      }
    }
  ]
}
```

## Requirements

- `schema` must be `1`.
- The kit must contain exactly 12 instruments.
- Instrument `id` values must be unique.
- Each instrument needs at least one sample.
- Velocity ranges must not be inverted. Layers are sorted by `velocity_min`.
- WAV paths are relative to the kit directory and are validated on load.
- PCM 16-bit, PCM 32-bit and float WAV sources are accepted. Multichannel sources are downmixed to mono.
- 48 kHz sources are recommended. The engine resamples playback for other sample rates.
- `choke_group` refers to a group id; the closed/open hi-hat factory kit uses a choke group.
- `defaults` is optional. Missing fields use the built-in instrument defaults.
- `body` is a 0..1 low-end weight control. It defaults to 0.0 and is especially useful on kick and toms.

## Installation

Copy the kit directory into:

```text
${XDG_DATA_HOME:-~/.local/share}/nirudrums/kits/
```

or set `NIRUDRUMS_KIT_DIR` to a directory containing kit folders. Open the **KIT** screen and choose the kit from **KIT LIBRARY**, or enter its path in **Kit directory** and press **LOAD KIT**.

## Factory kit provenance

The MERCY-80 core one-shots are derived from CC0 sources documented in
`kits/MERCY-80/source/cc0/SOURCE.md`. The crash, ride, clap and rimshot layers are
original synthesis. No commercial recording or proprietary sample library is used.

## Versioning

Future kit schemas will use a higher `schema` number and will be rejected by 0.0.1 with a clear error rather than loaded incorrectly. Keep the original directory intact when editing a kit so projects can be repaired.

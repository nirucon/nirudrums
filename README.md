# NIRUDRUMS

A professional vintage drum machine for Linux and macOS — dark, mechanical, and built for gothic rock, post-punk, darkwave, and industrial music.

**Current version:** 0.0.1 (in development)
**Factory kit:** MERCY-80
**Design language:** NIRU NOIR

> NIRUDRUMS 0.0.1 — concept, design, and creative direction by Nicklas Rudolfsson.

---

## What it is

NIRUDRUMS is a native Rust drum machine delivered in two forms that share one audio engine:

- **VST3 instrument** for REAPER (and other VST3 hosts) on Linux and macOS.
- **Standalone application** with its own native GUI, sequencer, song arrangement, and WAV export.

It is not a browser app, a mockup, or a CLI renderer.

## Planned features

- **MERCY-80** factory kit — an original, legally distributable sample library in the spirit of early-1980s drum machines.
- Per-instrument volume, pan, pitch, decay, velocity response, mute/solo, analog drive, reverb send, and output routing.
- Hi-hat choke groups.
- 16/32/64-step sequencer with velocity, accents, swing, pattern banks, undo/redo, and keyboard-driven workflow.
- Song mode with a visual arrangement timeline, loop sections, and WAV/MIDI export.
- Pattern Mode / Song Mode plus external MIDI triggering in the plugin.
- Vintage reverb (plate, room, gated) and analog drive, with a fully dry bypass path.
- Integrated mixer with peak meters and multi-output routing.
- Extensible, versioned JSON + WAV drum kit format.
- Versioned project files that round-trip between standalone and plugin.

## Platforms

| Platform | Role | Status |
|---|---|---|
| Debian (x86_64) | Primary development and test platform | planned |
| Arch Linux / Omarchy | Primary target | planned |
| Void Linux | Primary target | planned |
| Other Linux distributions | Portable tarball | planned |
| macOS (Apple Silicon, Intel) | Secondary target | planned |

## Installation

Releases ship **prebuilt binaries only**. No Rust toolchain, compiler, or build step is required — installing NIRUDRUMS is copying and registering files.

- VST3 → `~/.vst3/`
- Standalone → `~/.local/bin/`
- User data → XDG directories (`~/.local/share/nirudrums`, `~/.config/nirudrums`)

Per-user installation; no root required. Distribution packages (`.deb`, Arch package, Void `xbps` template, portable tarball) and macOS bundles will be published with each release.

## Building from source

Building is for contributors only.

```sh
cargo build --release --locked
cargo test --locked
```

## Repository layout

```text
crates/          Rust workspace (core, dsp, sampler, sequencer, midi, kits, project, ui, vst3, standalone)
kits/MERCY-80/   Factory sound library
presets/         Factory presets
docs/            Architecture, kit format, build and test notes
```

## Development brief

The full engineering brief that governs this project lives in [`AGENTS.md`](AGENTS.md).

## License

TBD.

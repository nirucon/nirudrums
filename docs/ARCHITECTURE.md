# Architecture — NIRUDRUMS 0.0.1

This document describes the implemented 0.0.1 architecture and the constraints that remain.


## Goals that drive the architecture

1. One audio engine, two front ends: VST3 plugin and standalone.
2. Binary-only distribution: the shipped artifacts must run on Debian, Arch, and Void without a Rust toolchain.
3. Real-time safety in the audio callback.
4. A kit/sample format that survives new app versions and user-created content.
5. Deterministic offline rendering for WAV export and REAPER freezes.

## Implemented workspace

```text
crates/
  nirudrums-core        DSP, kit/sample loading, patterns, song slots, project/preset format,
                        voice pool, sequencer clock, mixer, reverb, limiter, WAV rendering
  nirudrums-plugin      NIH-plug VST3 + standalone wrapper, parameter tree, engine bridge,
                        egui editor, settings/export UI
  xtask                 NIH-plug release bundler
```

The core crate is deliberately front-end independent. `Nirudrums` in the plugin crate reads host parameters and note events, then calls `Engine::process`. The standalone and VST3 use the same `Engine`, `Kit`, `Project` and DSP code.

The factory kit and presets are embedded with `include_bytes!` into `factory_assets.rs`; external kits are loaded from the XDG data directory or `NIRUDRUMS_KIT_DIR`.

The UI communicates pad triggers through a bounded `ArrayQueue<EngineCommand>`. The UI holds a project `RwLock`; the audio thread uses `try_read`, so it never blocks if the editor is mid-edit. Kit swaps use a bounded queue and are applied at the start of a process block. Meters and playhead are atomics.

## Current real-time behavior

The audio callback avoids heap allocation, blocking locks, file I/O, logging and GUI calls in the normal path. The project is copied into fixed-size pattern arrays each block. Kit loading and WAV export happen outside the audio callback.

Known gaps are recorded in `KNOWN_LIMITATIONS.md`: there is one stereo output, and MIDI learn, MIDI export and multi-output routing are not implemented in 0.0.1.

## Framework decision

NIH-plug at rev `de421011f41a6d10fc8c7a6084e4f4dee0143683` is used for VST3 and standalone hosting, state persistence, parameters and the egui editor host. It was verified with REAPER 7.84 on Debian 13: VST3 discovery, MIDI triggering, editor opening and offline peak rendering all worked. The known limitation is that this implementation does not use multiple VST3 output buses.

## Kit and project formats

See `KIT_FORMAT.md`. Project and preset JSON are versioned with `schema: 1`; the core validates schema, BPM, pattern lengths, song references and kit data. The embedded kit is the recovery source when an external kit is unavailable.

## Build and release pipeline

`cargo xtask bundle nirudrums --release` produces `target/bundled/NIRUDRUMS` and `target/bundled/NIRUDRUMS.vst3`. `scripts/package.sh` stages the binaries, factory assets, installer, docs and test results into `dist/nirudrums-0.0.1-linux-x86_64.zip`, runs `--selftest`, and writes a SHA-256 checksum. CI and macOS/Arch/Void packaging are not part of 0.0.1.

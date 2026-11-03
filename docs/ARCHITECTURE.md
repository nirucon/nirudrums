# Architecture plan — NIRUDRUMS 0.0.1

This document records the intended technical foundation. It is a plan, not a claim of implemented functionality.

## Goals that drive the architecture

1. One audio engine, two front ends: VST3 plugin and standalone.
2. Binary-only distribution: the shipped artifacts must run on Debian, Arch, and Void without a Rust toolchain.
3. Real-time safety in the audio callback.
4. A kit/sample format that survives new app versions and user-created content.
5. Deterministic offline rendering for WAV export and REAPER freezes.

## Workspace

```text
crates/
  nirudrums-core        Engine orchestration, transport, voice allocation, rendering
  nirudrums-dsp         Reverb, drive, limiter, filters, meters (no GUI, no I/O)
  nirudrums-sampler     Sample loading, velocity layers, choke groups, pitch/decay
  nirudrums-sequencer   Patterns, song timeline, clock, swing, quantization
  nirudrums-midi        Note mapping, MIDI parsing, learn
  nirudrums-kits        Kit discovery, validation, import/export
  nirudrums-project     Versioned project format, save/load, migration
  nirudrums-ui          Shared NIRU NOIR widgets and views
  nirudrums-vst3        VST3 wrapper (nih-plug), host sync, state
  nirudrums-standalone  Native window, audio device, MIDI input, WAV export
```

`nirudrums-core` owns all audio behavior. Front ends only translate host/user events into engine commands and read meters/playhead state.

## Framework evaluation

NIH-plug is the default candidate for VST3 because it produces VST3/CLAP from one Rust implementation and provides an editor host for the standalone build. Before committing, verify at the pinned version:

- VST3 bundles load in REAPER on Linux (Wayland and X11).
- Shared-memory / GUI threading works with the chosen GUI library.
- Multiple audio output buses are supported (or a viable fallback exists).
- State save/restore covers the full project, not just parameters.
- License compatibility and maintenance activity.

If NIH-plug cannot satisfy multi-output and state requirements, fall back to a direct VST3 SDK binding and share only the engine and UI crates. Decide from evidence, and record the decision here.

## Binary portability strategy

| Concern | Approach |
|---|---|
| Rust runtime | Static CRT/runtime where the target allows; no `cargo` on the user machine |
| C/C++ deps (VST3 SDK, audio backends) | Prefer static linking; otherwise vendor the exact .so and ship it beside the binary with an rpath |
| glibc floor | Build against the oldest supported glibc (build container or CI image), verify on Debian stable, Arch, Void glibc |
| Void musl | Separate artifact if glibc build cannot run; document status honestly |
| X11/Wayland | Runtime `dlopen` of `libX11`/`libxkbcommon`/`libwayland-client`; package dependencies listed per format |
| Audio backends | ALSA + JACK/PipeWire at runtime; ship no server |
| macOS | Universal or per-arch `.app` + `.vst3`, ad-hoc or Developer ID signing, notarization status documented |

Each release includes a `RUNTIME-REQUIREMENTS.md` listing exact shared libraries and minimum versions, plus the distros and versions actually tested.

## Real-time contract

The audio thread:

- never allocates, locks (blocking), logs, touches files, or calls GUI code;
- reads parameters from atomic/shared snapshots published by the UI;
- uses a preallocated voice pool with deterministic stealing;
- receives events through a bounded lock-free queue (MIDI, transport, step triggers);
- publishes meters and playhead through atomics.

Sample data is fully loaded and validated before being published to the audio thread. Missing samples are reported in the UI and substituted with silence, never with a crash.

## Kit format (v1, draft)

```text
kits/MERCY-80/
  kit.json          # schema version, instrument list, defaults, choke groups
  samples/*.wav     # 24-bit or 16-bit PCM, 44.1/48 kHz source
  presets/*.json    # processing presets for this kit
```

`kit.json` is versioned and validated on load. Unknown fields are preserved on round-trip where practical. Velocity layers and round-robin are reserved in the schema so v1 kits remain loadable.

## Project format

A single versioned JSON document (plus references to kits by id + content hash). Missing kit or sample references produce a non-fatal warning with a repair path, not a failed load.

## Build and release pipeline

- CI builds Linux x86_64 and macOS arm64/x86_64 artifacts from a locked `Cargo.lock`.
- `cargo fmt --check`, `clippy -D warnings`, `test --locked`, and `build --release --locked` gate the release.
- Packaging step produces: portable tarball, `.deb`, Arch package (or PKGBUILD + prebuilt package), Void template, macOS bundles.
- Smoke test unpacks each artifact in a clean Debian/Arch/Void container and runs `--selftest` (audio-device-free engine check) plus an installer dry run.
- Release notes list exactly what was verified and what was not.

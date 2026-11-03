# Test results — NIRUDRUMS 0.0.2

Date: 2026-11-03

## Build host

- Linux x86_64
- Rust toolchain current at build time
- NIH-plug pinned to `de421011f41a6d10fc8c7a6084e4f4dee0143683`

## Commands

| Command | Result |
|---|---|
| `cargo fmt --check` | pass |
| `cargo clippy --workspace --all-targets` | pass, no warnings |
| `cargo test --locked` | pass |
| `cargo build --release --locked` | pass |
| `cargo xtask bundle nirudrums --release` | pass |
| `target/bundled/NIRUDRUMS --selftest` | pass: `48 kHz stereo render peak 0.694` |
| core unit tests | 18 passed |

## Core tests covered

- Factory kit loads 12 instruments and 36 velocity-layer samples; the CC0-derived kick is tight, punchy and reaches full-scale body.
- External kit directory loads from `kits/MERCY-80`.
- Factory project contains a playable demo pattern and MIDI mappings.
- Project and preset JSON round-trips preserve patterns, song slots, mixer and effects.
- WAV decoder accepts generated 16-bit sources.
- Sequencer advances on the sample clock (not GUI frames).
- Offline renderer writes a non-silent 48 kHz stereo WAV.
- Dry path renders finite audio; DSP is stable and finite.
- Parameter IDs are unique in the plugin.
- Body/low-end processing measurably increases kick energy.
- Seven factory presets parse, including MERCY-80 Cathedral and MERCY-80 Gated Room.

## REAPER 7.84 / Debian 13

Test machine: `othala`, Debian 13, REAPER 7.84, X11 display through Xvfb for automation.

- VST3 bundle copied to `~/.vst3/NIRUDRUMS.vst3`.
- REAPER discovered and scanned it as `VST3i: NIRUDRUMS (NIRU)`.
- A ReaScript inserted the instrument on a track and created a MIDI note at note 36.
- Offline render produced a 44.1 kHz 24-bit stereo WAV with peak `-4.27 dBFS`, proving the instrument produced audio from MIDI triggering without clipping.
- The VST3 editor window opened from REAPER and reported the NIRUDRUMS window title.
- The plug-in state chunk was written into the REAPER project and loaded again by REAPER during the test.

## Standalone

- Built both debug and release standalone binaries.
- Launched the release standalone under Xvfb with the dummy audio backend.
- Window `NIRUDRUMS` opened; pattern grid, pads, inspector, transport, meters and status bar rendered.
- Clicking steps toggled velocity and the transport play button advanced the pattern/current-step display.
- `--selftest` loaded the embedded kit and rendered a nonzero stereo WAV without a display or audio device.
- The KIT screen listed the installed MERCY-80 kit and loaded it from the XDG kit library without a crash.
- `niruxwork` (Debian 13, no REAPER): release standalone `--selftest` passed with peak `0.705`; the GUI opened under Xvfb.

## NIRUWM tiling regression

- Reproduced the reported standalone layout bug in a nested NIRUWM 0.16.162 session at 1920x1200: NIRUWM tiled the outer window to `1920x1172`, but the egui child stayed at its initial `1280x820`, leaving a large unused area.
- Added a Linux-only parent-window size synchronizer for the standalone executable. It finds the outer NIRUDRUMS window, then asks egui to resize the editor child to the outer window's current size. The VST3 path skips this because the host executable is REAPER, not the standalone binary.
- After the fix, the nested NIRUWM test reported outer and child windows both at `1920x1172`; the pattern and velocity views filled the window.
- `nirushot --full` completed successfully in the same nested NIRUWM session and produced a 1920x1200 screenshot. The earlier hang was observed with the stale layout; the corrected build did not reproduce it in this test.

## Installer smoke test

- Extracted `dist/nirudrums-0.0.1-linux-x86_64.zip` into a clean temporary directory.
- Ran `install.sh` with temporary `HOME`, `PREFIX`, `VST3_DIR` and `XDG_DATA_HOME`.
- Installer completed, copied the standalone, VST3 bundle, kit, presets, docs, launcher entry and icon; the installed binary passed `--selftest`.
- Final package was reinstalled on `othala` and `niruxwork`; installed checksums match the release bundle.
- Body/low-end processing, the vintage 8-bit character and the gated-room preset were included in the final REAPER render test.
- Ran `uninstall.sh`; binaries and factory assets were removed while a sentinel user project file was preserved.

## UX verification

- Standalone at 1280x820 under Xvfb: quick-guide overlay opened from the `?` action, preset browser applied a factory preset, drag-painting changed a run of steps, and the velocity lane showed the updated values.
- The pattern grid no longer scrolls while painting; selected-step feedback is limited to the active channel.
- Mixer showed per-channel peak meters, dB/pan readouts and a fixed master L/R summary; `C` centered a pan value.
- Song/Settings/FX layouts, sound replacement, session save/open and preset save/open were exercised in the same standalone session.
- Song arranger: palette click selected the pattern, dragging it into the empty timeline created a slot, and the selected-slot editor exposed repeats, Up/Down, Duplicate and Remove.
- Song timeline exposes drop markers, a delete drop target, Delete-key removal and automatic total-bar display.
- FX rack renders equal-size modules with vintage knobs and a send matrix.
- At 1280x820 the mixer wrapped all 12 channels into a 9+3 grid; master L/R summary stayed pinned at the top.
- Window geometry was verified in the NIRUWM regression above.

## Not tested

- Arch Linux, Void Linux, Fedora, NixOS or other distributions.
- macOS and Windows.
- Physical audio interface monitoring on the test machine (PipeWire/ALSA hardware was not available during automation).
- Multi-output routing, MIDI export and MIDI learn, which are not implemented in 0.0.1.
- Long-duration stress, exhaustive REAPER render modes and every VST3 host.

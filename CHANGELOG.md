# Changelog

All notable changes to NIRUDRUMS are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [0.0.2] - 2026-11-03

### Added

- Drag-and-drop song arranger with pattern palette, drop markers, slot reordering, Up/Down moves, duplicate/remove, trash-drop deletion, Delete-key removal and repeat controls.
- Mixer redesign with responsive wrapping, per-channel peak meters, volume/pan readouts, pan centering, mute/solo, body/send values and a fixed master L/R summary.
- Vintage FX rack with equal-size reverb, master and send modules, custom knobs and an extensible `fx_module` layout.
- Responsive mixer grid that wraps all 12 channels into rows before using vertical scroll.
- Card-based Song, FX and Settings layouts; improved Kit and sound-replacement workflow.
- Session/song open and save actions in the sidebar, Song view and Settings; user preset save/load in the plugin sidebar.
- Per-channel peak metering in the shared engine.

### Changed

- Pattern painting, interactive velocity editing, keyboard shortcuts, quick guide, preset browser and responsive NIRUWM window handling from the 0.0.1 UX pass.
- Neutral prototype README and no artist/equipment references in the public docs or UI.
- Version bumped to 0.0.2.

## [0.0.1] - 2026-11-03

### Added

- Native NIRUDRUMS VST3 instrument and standalone application using one shared Rust engine.
- MERCY-80 original 12-instrument, three-layer velocity drum kit with 48 kHz mono sources.
- CC0-licensed core one-shots, 8-bit character and a tighter, punchier MERCY-80 kick.
- Source and license provenance documented in `kits/MERCY-80/source/dx/SOURCE.md`.
- MERCY-80 Gated Room preset with gated room, fat snare and tom sends.
- Analog-style channel processing: volume, pan, pitch, decay, body/low-end weight, velocity response, drive, reverb send, mute and solo.
- Hi-hat choke groups, 16-voice-per-instrument pool and deterministic voice stealing.
- 16/32/64-step patterns, multiple pattern slots, pattern naming, copy/paste, duplicate, clear, undo/redo, step velocity, accents and swing.
- Internal sample-accurate sequencer, host tempo/transport following, external MIDI note triggering and user-editable MIDI mapping.
- Song mode with ordered pattern slots and repeat counts.
- Plate, room and gated reverb, master drive, soft limiter, dry bypass and stereo meters.
- egui interface with pattern, kit, mixer, song, FX and settings views.
- Seven factory presets, including MERCY-80 Gated Room and MERCY-80 Cathedral; user preset save/load, versioned project save/load and stereo WAV export.
- Portable kit format, external kit discovery and kit loading.
- Binary installer, uninstaller, release packaging and `--selftest`.
- Core tests for kit loading, project/preset round-trips, direct WAV decoding, sequencer timing, dry rendering and offline WAV rendering.

### Known limitations

See `KNOWN_LIMITATIONS.md`. Multi-output routing, MIDI export, MIDI learn, drag-and-drop song editing, macOS builds and Arch/Void-native packages are not part of 0.0.1.

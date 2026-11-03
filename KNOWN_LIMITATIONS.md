# Known limitations — NIRUDRUMS 0.0.1

NIRUDRUMS 0.0.1 is a complete Linux x86_64 first release, but it is not feature-complete against every item in the long-term brief. This file records the gaps honestly.

## Platform and packaging

- Only Linux x86_64 is built and tested. macOS, Windows, Arch-native packages, Void-native packages and 32-bit builds are not included.
- The release is a portable ZIP with a user-level installer. There is no `.deb`, PKGBUILD or `xbps` template in 0.0.1.
- The binaries dynamically link common X11/OpenGL/ALSA libraries. See `RUNTIME-REQUIREMENTS.md`.

## Audio and routing

- There is one stereo master output. Per-instrument multiple-output routing to separate REAPER tracks is not implemented.
- The `output` field is reserved in the kit/project schema but the mixer currently routes every instrument to the stereo master.
- The sequencer assumes 4/4 and 16th-note steps. Configurable time signatures are not implemented.
- Quantization is implicit in the step sequencer; there is no separate record-quantize feature because there is no MIDI recording workflow.

## Sequencer and song mode

- Song arrangement is a slot list with pattern and repeat controls. It is not a drag-and-drop timeline with loop-section editing.
- Pattern undo/redo covers pattern edits. Mixer and effect moves are not individually undoable.
- Keyboard shortcuts cover transport, pattern selection and undo/redo; there is no full shortcut editor.
- Dedicated accent lanes are not present; accents are expressed through per-step velocity.

## MIDI

- MIDI note-on, velocity and user note mapping work. MIDI learn, MIDI channel filtering and MIDI activity indicators are not implemented.
- Standalone MIDI device selection is provided by the NIH-plug command-line backend options rather than a custom in-app device dialog.
- MIDI export is not implemented. WAV export is implemented.

## Kits and presets

- External kits must contain exactly 12 instruments and match the v1 kit schema. The KIT screen can load a kit directory; individual sample replacement is done by editing the kit directory and reloading it.
- A project that references an external kit reloads the embedded factory kit on a fresh application start; the external kit must be selected again in the KIT screen. Projects still open because the kit id is preserved, but the samples may differ until the kit is loaded.
- User presets store processing settings, not patterns or song arrangement.

## Host integration

- VST3 discovery, note triggering, host transport/tempo following, state save/restore, offline rendering and editor open/close were tested in REAPER 7.84 on Debian 13.
- Repeated long-duration stress testing, every REAPER render mode and every Linux host were not exhaustively verified.
- Multi-output and CLAP formats are not implemented.

## macOS

- No macOS build is included. The source is intended to be portable, but macOS audio/editor integration was not compiled or verified for 0.0.1.

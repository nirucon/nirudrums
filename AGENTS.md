# NIRUDRUMS – MASTER DEVELOPMENT PROMPT

## ROLE AND OBJECTIVE

Act as a senior Rust software architect, professional audio DSP engineer, VST3 plugin developer, Linux/macOS application developer, UX/UI designer, and experienced music production software engineer.

Your assignment is to design, implement, test, package, and deliver **NIRUDRUMS**, a professional-quality vintage drum machine inspired by the dark, powerful, mechanical drum sound associated with early 1980s gothic rock, post-punk, darkwave, and industrial music.

The primary sonic reference is the atmosphere and drum production of **The Sisters of Mercy – First and Last and Always (1985)** and the early Doktor Avalanche drum machine aesthetic.

Do not copy copyrighted recordings, extract samples from commercial releases, or reproduce proprietary sample libraries. Create an original, legally distributable sound library that captures the general production characteristics of the era.

The application must be called:

**NIRUDRUMS**

Initial version: **0.0.1**

Default factory drum kit: **MERCY-80**

Credit line, used in the application About screen, README, release notes, and documentation:

> **NIRUDRUMS 0.0.1 — concept, design, and creative direction by Nicklas Rudolfsson.**

All application UI text, documentation, parameter names, menus, tooltips, and error messages must be in English.

The goal is a genuinely useful, polished, efficient, stable, and musically inspiring instrument.

**This must be a real audio application, not a mockup, browser-based prototype, or command-line WAV renderer.**

---

## 0. DELIVERY MODEL – ONE SHOT, PREBUILT BINARIES

The first genuinely usable version of NIRUDRUMS must be delivered as a complete, working product in a single continuous development effort ("one shot"). Do not deliver an intermediate skeleton, a documentation-only milestone, or a partially wired application and call it done.

Release artifacts must be **prebuilt binaries**. End users must never compile Rust source, install a toolchain, run `cargo build`, or resolve build dependencies on their own machines.

- Every release ships runnable standalone executables and VST3 bundles for each supported platform.
- Installers and install scripts must only copy, register, and verify already-built files.
- If a distribution-specific package is provided (`.deb`, Arch package, Void `xbps` template, portable tarball, macOS `.pkg`/`.dmg`), it must contain the compiled binaries.
- The build happens on the developer/CI side only. The install side is a pure binary installation.
- Never require root for a normal per-user installation.

If a platform or architecture cannot be built and verified in the current environment, say so explicitly and ship only what is honestly verified. Never invent build or test results.

---

## 1. TARGET PLATFORMS

### Primary priorities

1. **Debian** (stable/testing, x86_64) — primary development and verification platform.
2. **Arch Linux**, including Omarchy (x86_64).
3. **Void Linux** (glibc and, where practical, musl; x86_64).
4. REAPER running on Linux.
5. Native standalone Linux application.
6. macOS on Apple Silicon, and Intel where practical.
7. REAPER running on macOS.
8. Additional Linux distributions (Fedora, Ubuntu, openSUSE, NixOS, etc.) where the binary artifacts run without modification or with a documented, binary-only packaging step.

Binary portability across distributions is a first-class requirement: prefer static linking of the Rust runtime and vendor or statically link third-party C/C++ dependencies where licensing allows, so the same artifact runs on Debian, Arch, and Void. Document any remaining shared-library requirements and the minimum versions. Test on Debian, Arch, and Void (VM or container is acceptable, but state exactly what was tested).

### Plugin format

- VST3 is mandatory.
- The plugin must load and function correctly in REAPER.
- It must receive MIDI from its REAPER track.
- It must follow the host tempo and transport.
- It must correctly save and restore plugin state within REAPER projects.
- It must support offline rendering.
- It should support multiple audio outputs for individual drum instruments.
- Avoid unnecessary external runtime dependencies.

Use Rust as the primary programming language.

Evaluate NIH-plug or another suitable Rust audio plugin framework based on actual compatibility, maintenance status, licensing, and technical requirements. Do not blindly select a framework without verifying that it supports the intended architecture.

### Standalone

The standalone application must use the same audio engine, sequencer, DSP, preset format, and sound library as the VST3 plugin.

Use an appropriate native audio backend, such as JACK/PipeWire or ALSA on Linux and CoreAudio on macOS. Provide device selection, sample rate configuration, buffer settings where supported, and MIDI input configuration.

The standalone application must be a native executable with its own GUI. Do not use a local web server and browser as the final application interface.

---

## 2. SOUND DESIGN – MERCY-80

Create an original factory sound library called MERCY-80.

Its character should be:

- Dark and aggressive.
- Powerful and punchy.
- Mechanical and precise.
- Vintage 1980s drum machine inspired.
- Slightly gritty and saturated.
- Suitable for gothic rock, post-punk, darkwave, industrial, and alternative rock.
- Capable of producing a large, hard-hitting drum sound without requiring external plugins.

The factory kit must sound impressive immediately after loading.

### Instruments

Include at least:

| Instrument | MIDI Note |
|---|---|
| Kick | 36 |
| Snare | 38 |
| Closed Hi-Hat | 42 |
| Open Hi-Hat | 46 |
| Crash 1 | 49 |
| Crash 2 | 57 |
| Ride | 51 |
| High Tom | 50 |
| Mid Tom | 47 |
| Low Tom | 43 |
| Clap | 39 |
| Rimshot | 37 |

Provide a clear MIDI note mapping screen. MIDI mappings must be user-editable and savable.

### Sound processing

Each drum channel should support:

- Volume
- Pan
- Pitch
- Decay
- Velocity response
- Mute
- Solo
- Analog drive
- Reverb send
- Output routing

Implement choke groups, especially for open and closed hi-hats.

### Factory sound philosophy

**Processed by default, completely dry when requested.**

The default MERCY-80 preset should produce a polished, aggressive, vintage-inspired sound. Users must be able to disable all internal processing and access the clean underlying drum samples.

Ensure that dry mode actually bypasses all processing, including master saturation, limiting, reverb, and distortion.

Include a few factory presets demonstrating different sonic characteristics, such as:

- MERCY-80 Classic
- MERCY-80 Heavy
- MERCY-80 Dry
- MERCY-80 Gated
- MERCY-80 Industrial

The presets should use the same factory sound library with different processing settings.

---

## 3. EXTENSIBLE DRUM KIT SYSTEM

Design a modular sample library architecture.

Users must be able to:

- Install additional drum kits.
- Create their own kits.
- Import WAV samples.
- Replace individual drum sounds.
- Save custom presets.
- Export and import drum kit definitions.
- Organize kits into a library.

Use a documented, versioned kit format, preferably JSON metadata and WAV samples. Support future extensions such as velocity layers, round-robin samples, additional instruments, and sample variations.

Ensure existing projects remain compatible when new kits or application versions are installed. Missing samples must produce clear error messages rather than crashing the application.

---

## 4. SEQUENCER

Implement a complete, responsive step sequencer.

### Required functionality

- 16, 32, and 64-step patterns.
- Adjustable pattern length.
- Multiple patterns.
- Pattern banks.
- Copy/paste patterns.
- Duplicate patterns.
- Clear pattern.
- Undo/redo.
- Velocity per step.
- Accents.
- Swing.
- Tempo control.
- Quantization.
- Adjustable time signatures where practical.
- Pattern naming.
- Pattern selection using keyboard shortcuts.

The sequencer should be intuitive for users familiar with classic drum machines, FL Studio, and modern grooveboxes.

Clicking steps must provide immediate visual feedback. The active playback position must be clearly visible. Sequencer timing must be accurate and must not depend on GUI frame rate or browser timers.

---

## 5. SONG MODE AND ARRANGEMENT

Standalone NIRUDRUMS must support creating complete drum arrangements.

Implement a visual song arrangement editor where patterns can be placed along a timeline.

Users should be able to:

- Create an intro, verse, chorus, bridge, breakdown, and outro.
- Drag and rearrange patterns.
- Duplicate sections.
- Loop selected sections.
- Change pattern lengths.
- Save complete songs.
- Load existing songs.
- Export arrangements to WAV.
- Export MIDI where practical.

Provide a clear distinction between Pattern Mode and Song Mode.

In VST3 mode, the user must be able to choose between external MIDI triggering and internally sequenced playback. Avoid double-triggering when both MIDI input and internal sequencing are enabled.

---

## 6. MIDI INTEGRATION

MIDI integration is essential.

### VST3 mode

NIRUDRUMS must respond to MIDI notes placed on the REAPER track containing the plugin.

Support:

- MIDI note-on.
- MIDI note-off where relevant.
- Velocity.
- MIDI channel selection.
- User-editable note mapping.
- MIDI learn where practical.
- Sample-accurate triggering.
- Host tempo synchronization.
- Host transport synchronization.

The plugin must remain usable without its internal sequencer.

### Standalone mode

Support external MIDI controllers and MIDI keyboards. Provide MIDI device selection and clear activity indicators.

---

## 7. MIXER

Create an integrated drum mixer.

Each drum instrument must have an independent channel.

Provide:

- Volume fader.
- Pan.
- Mute.
- Solo.
- Peak meter.
- Effect sends.
- Output routing.
- Channel naming.

Support a stereo master output and, where technically supported, individual audio outputs for routing instruments to separate REAPER tracks.

The mixer should remain readable and usable at smaller plugin window sizes.

---

## 8. VINTAGE EFFECTS

Develop efficient DSP effects in Rust.

### Vintage Reverb

Provide a reverb inspired by early digital studio effects.

Modes: Plate, Room, Gated.

Parameters: Decay, Pre-delay, Damping, Width, Gate time, Wet/dry mix.

The gated reverb should be especially suitable for powerful 1980s snares and toms.

### Analog Drive

Implement a musical saturation/distortion processor.

Provide: Drive, Tone, Output gain, Wet/dry mix, Soft/hard saturation modes, optional oversampling.

Avoid unnecessary aliasing and harsh digital artifacts.

### Master section

Include:

- Master volume.
- Output meter.
- Optional soft limiter.
- Global effect bypass.
- Completely dry mode.

Effects must be computationally efficient and stable at common sample rates.

---

## 9. GUI AND UX – NIRU NOIR

Create a visually impressive but restrained interface. The design language is called **NIRU NOIR**.

### Visual direction

- Dark charcoal and near-black surfaces.
- Subtle gray contrasts.
- Carefully controlled highlights.
- Clean typography.
- Minimal decoration.
- No excessive gradients.
- No generic AI-generated dashboard appearance.
- No unnecessary animations.
- Consistent spacing and alignment.
- Excellent readability.
- Professional audio software aesthetics.

The interface should feel like a modern, sophisticated drum machine with subtle vintage industrial influences.

### Main sections

Organize the interface around:

1. DRUMS
2. SEQUENCER
3. SONG
4. MIXER
5. FX
6. SETTINGS

Provide a compact transport bar containing: Play, Stop, Record where applicable, BPM, Swing, Pattern selection, Current kit, Playback position, Host sync status.

### UX requirements

- Fast workflows.
- Minimal clicking.
- Contextual controls.
- Keyboard shortcuts.
- Undo/redo.
- Clear tooltips.
- Resizable window.
- High-DPI support.
- Good mouse-wheel behavior.
- Clear error and status messages.
- No unnecessary modal dialogs.
- No disruptive page or view resets.

The user should be able to create a convincing drum pattern within seconds of opening the application. Do not sacrifice usability for visual appearance.

---

## 10. RUST ARCHITECTURE

Use a modular Cargo workspace.

Suggested architecture:

```text
nirudrums/
├── crates/
│   ├── nirudrums-core/
│   ├── nirudrums-dsp/
│   ├── nirudrums-sampler/
│   ├── nirudrums-sequencer/
│   ├── nirudrums-midi/
│   ├── nirudrums-kits/
│   ├── nirudrums-project/
│   ├── nirudrums-ui/
│   ├── nirudrums-vst3/
│   └── nirudrums-standalone/
├── kits/
│   └── MERCY-80/
├── presets/
├── examples/
├── tests/
├── scripts/
├── docs/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── CHANGELOG.md
└── VERSION
```

The DSP engine must be independent of the GUI. The VST3 plugin and standalone application must share the same core implementation. Do not duplicate audio processing logic.

### Real-time audio safety

The audio callback must avoid: heap allocations, blocking locks, file operations, network operations, logging, GUI operations, and unbounded computations.

Use preloaded samples, bounded processing, and appropriate lock-free communication where needed. Handle sample rates and audio buffer sizes correctly. Implement voice management and predictable voice stealing. Support deterministic offline rendering.

---

## 11. PERFORMANCE

NIRUDRUMS must be lightweight.

Prioritize: low CPU usage, low memory consumption, fast startup, responsive UI, stable real-time audio, no unnecessary background services, minimal dependencies.

Benchmark the application with realistic drum patterns and effects enabled. Do not claim performance targets without measuring them. The GUI must not affect audio timing.

---

## 12. PROJECTS AND PRESETS

Implement reliable project persistence.

Save: selected drum kit, instrument settings, MIDI mappings, patterns, song arrangement, tempo, swing, mixer settings, effect settings, output routing.

Use a versioned project format. Validate imported data. Handle missing files gracefully. Ensure projects can be reopened with the same sound and arrangement.

VST3 plugin state must be correctly saved and restored by REAPER.

---

## 13. INSTALLATION AND DISTRIBUTION

Deliver actual working binaries. **No end-user compilation, ever.**

### Linux

Provide, at minimum:

- Compiled standalone executable.
- Compiled VST3 plugin bundle.
- MERCY-80 sound library.
- Factory presets.
- Installation script.
- Uninstallation script.
- Documentation.

Distribution coverage:

- **Debian / Ubuntu** – `.deb` package and/or portable tarball.
- **Arch / Omarchy** – portable tarball plus, where practical, an installable package definition (PKGBUILD or prebuilt package).
- **Void Linux** – portable tarball plus, where practical, an `xbps` package template.
- **Other distributions** – one portable tarball that runs without recompilation, with documented runtime requirements.

Install VST3 into the appropriate user plugin directory, typically `~/.vst3/`. Install standalone into `~/.local/bin/`. Use appropriate XDG directories for application data.

Do not require root permissions for normal installation.

Verify that the shipped binaries start and load correctly on Debian, Arch, and Void (state clearly what was actually tested and what was not).

### macOS

Provide appropriate builds for Apple Silicon and Intel where supported. Use standard macOS application and VST3 installation locations. Document signing, notarization, and any remaining platform restrictions. Ship prebuilt universal or per-architecture binaries; no Xcode or Rust toolchain requirement for users.

### Uninstallation

Provide safe removal procedures. Do not delete user-created projects, presets, or custom drum kits without explicit confirmation.

---

## 14. VERSIONING AND UPGRADES

Use consistent semantic versioning. Initial release: **0.0.1**.

Keep version information synchronized between Cargo metadata, application GUI, plugin metadata, documentation, release packages, and changelog. Do not hardcode inconsistent version numbers. Use Cargo package metadata as the authoritative version source where practical.

Ensure `Cargo.lock` matches `Cargo.toml`. Never distribute a release that fails because of a stale lock file. Preserve backward compatibility where reasonable.

---

## 15. TESTING AND QUALITY ASSURANCE

Do not consider a release finished until it has passed relevant tests.

### Required verification

- `cargo fmt --check`
- `cargo clippy`
- `cargo test --locked`
- `cargo build --release --locked`
- Sample library validation.
- Project save/load tests.
- MIDI note mapping tests.
- Sequencer timing tests.
- Dry processing bypass tests.
- Audio rendering tests.
- Installation smoke tests (binary-only install path).
- Uninstallation tests.

### REAPER testing

Verify plugin discovery, initialization, MIDI triggering, audio output, transport synchronization, tempo changes, project save/reopen, offline rendering, multi-output routing where implemented, window open/close, and repeated loading/unloading without crashes.

Use an actual REAPER installation for integration tests whenever possible. If a platform cannot be tested, explicitly document that limitation. Never describe an untested binary as verified.

---

## 16. DEVELOPMENT PROCESS

Work incrementally, but deliver a complete, usable first release in a single sustained effort.

1. Inspect any existing source code and release archives.
2. Identify working functionality.
3. Preserve functioning components.
4. Review architecture and dependencies.
5. Implement the shared Rust audio engine.
6. Implement real-time playback.
7. Implement VST3 integration.
8. Implement standalone audio and GUI.
9. Integrate MERCY-80.
10. Implement sequencer, mixer, effects, and song arrangement.
11. Test and benchmark.
12. Package and deliver prebuilt binaries.

Avoid unnecessary rewrites. If a previous implementation is unsuitable, explain why before replacing it. Do not silently remove existing functionality. Do not use a browser interface as a substitute for the requested native application. Do not deliver a command-line renderer as though it were a finished plugin.

---

## 17. DELIVERABLES

Produce downloadable release ZIP files. At minimum:

`nirudrums-0.0.1-linux-x86_64.zip`

Include source code separately where appropriate. For macOS, provide platform-specific packages when they can be built and verified.

Each release must contain: working application binaries, VST3 plugin bundle, MERCY-80 factory kit, factory presets, examples, installation instructions, uninstallation instructions, changelog, known limitations, and test results.

The installer must check for errors and must not report success when installation or verification fails.

---

## 18. FUTURE DEVELOPMENT

Design the architecture to support future improvements without major rewrites: more vintage drum kits, advanced sample editor, additional DSP effects, humanization, probability per step, polyrhythms, pattern variations, MIDI drag-and-drop, audio drag-and-drop, advanced routing, preset browser, kit designer, additional plugin formats such as CLAP, optional theme selection.

Do not let speculative future functionality delay the initial working release.

---

## 19. IMPORTANT ENGINEERING PRINCIPLES

Priorities, in order:

1. Correct and stable audio behavior.
2. Working REAPER VST3 integration on Linux.
3. Excellent MERCY-80 sound.
4. Responsive and intuitive NIRU NOIR GUI.
5. Efficient Rust architecture.
6. Functional standalone application.
7. Sequencer and song arrangement.
8. Reliable, binary-only installation and updates across Debian, Arch, and Void.
9. macOS compatibility.
10. Future extensibility.

A small, reliable, polished instrument is preferable to a large collection of partially implemented features. However, the first release must be a genuinely usable drum machine, not merely a technical demonstration.

---

## 20. FINAL INSTRUCTIONS

Start by evaluating the architecture and defining a realistic implementation plan. Then proceed with actual implementation.

Do not stop after writing documentation or generating a project skeleton. Do not claim functionality that has not been implemented. Do not substitute a web application, CLI renderer, or static GUI mockup for the requested VST3 instrument.

If required build tools or platform environments are unavailable, clearly identify the blocker and deliver only what can be honestly verified. Never invent successful compilation or testing results.

**The final goal is simple:**

I want to open REAPER on Linux, insert NIRUDRUMS as a VST3 instrument, select MERCY-80, program or play hard-hitting vintage drum patterns through MIDI, adjust the individual instruments and effects, and immediately hear an impressive, powerful drum sound.

I also want to launch NIRUDRUMS as a standalone application, create patterns, arrange a complete drum song, and export the result.

Both modes must feel like parts of the same professionally designed instrument.

**Build NIRUDRUMS as a real product worth continuing to develop.**

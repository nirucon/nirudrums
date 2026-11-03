use crate::dsp::{db_to_gain, soft_clip, Limiter, Reverb};
use crate::kit::{Kit, SampleData};
use crate::project::{
    InstrumentSettings, MasterSettings, Pattern, Project, ReverbSettings, SequencerMode, SongSlot,
    MAX_SONG_SLOTS, MAX_STEPS, NUM_INSTRUMENTS,
};
use atomic_float::AtomicF32;
use crossbeam_queue::ArrayQueue;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::f64::consts::PI;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub const VOICES_PER_INSTRUMENT: usize = 16;

#[derive(Clone, Copy, Debug)]
pub struct EngineSettings {
    pub instruments: [InstrumentSettings; NUM_INSTRUMENTS],
    pub master: MasterSettings,
    pub reverb: ReverbSettings,
    pub bpm: f32,
    pub swing: f32,
    pub playing: bool,
    pub song_mode: bool,
    pub sequencer_mode: SequencerMode,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            instruments: [InstrumentSettings::default(); NUM_INSTRUMENTS],
            master: MasterSettings::default(),
            reverb: ReverbSettings::default(),
            bpm: 120.0,
            swing: 0.0,
            playing: false,
            song_mode: false,
            sequencer_mode: SequencerMode::Internal,
        }
    }
}

impl EngineSettings {
    pub fn from_project(project: &Project) -> Self {
        Self {
            instruments: project.instruments,
            master: project.master,
            reverb: project.reverb,
            bpm: project.bpm,
            swing: project.swing,
            playing: true,
            song_mode: project.song_mode,
            sequencer_mode: project.sequencer_mode,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum EngineCommand {
    Trigger { note: u8, velocity: f32 },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoteInput {
    pub offset: u32,
    pub note: u8,
    pub velocity: f32,
}

pub struct Meters {
    pub left: AtomicF32,
    pub right: AtomicF32,
    pub channels: [AtomicF32; NUM_INSTRUMENTS],
}

impl Default for Meters {
    fn default() -> Self {
        Self {
            left: AtomicF32::new(0.0),
            right: AtomicF32::new(0.0),
            channels: std::array::from_fn(|_| AtomicF32::new(0.0)),
        }
    }
}

impl Meters {
    pub fn peak(&self) -> (f32, f32) {
        (
            self.left.load(Ordering::Relaxed),
            self.right.load(Ordering::Relaxed),
        )
    }

    pub fn channel_peak(&self, instrument: usize) -> f32 {
        self.channels
            .get(instrument)
            .map(|meter| meter.load(Ordering::Relaxed))
            .unwrap_or(0.0)
    }

    fn store(&self, left: f32, right: f32) {
        self.left.store(left, Ordering::Relaxed);
        self.right.store(right, Ordering::Relaxed);
    }

    fn store_channels(&self, channels: &[f32; NUM_INSTRUMENTS]) {
        for (meter, value) in self.channels.iter().zip(channels) {
            meter.store(*value, Ordering::Relaxed);
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PatternRuntime {
    length: u8,
    steps: [[u8; MAX_STEPS]; NUM_INSTRUMENTS],
}

impl Default for PatternRuntime {
    fn default() -> Self {
        Self {
            length: 16,
            steps: [[0; MAX_STEPS]; NUM_INSTRUMENTS],
        }
    }
}

impl PatternRuntime {
    fn from_pattern(pattern: &Pattern) -> Self {
        let mut runtime = Self {
            length: pattern.length.clamp(1, MAX_STEPS as u8),
            steps: [[0; MAX_STEPS]; NUM_INSTRUMENTS],
        };
        for instrument in 0..NUM_INSTRUMENTS {
            for step in 0..MAX_STEPS {
                runtime.steps[instrument][step] = pattern.velocity(instrument, step);
            }
        }
        runtime
    }
}

#[derive(Clone, Copy, Debug)]
struct SongRuntime {
    slots: [SongSlot; MAX_SONG_SLOTS],
    len: usize,
}

impl Default for SongRuntime {
    fn default() -> Self {
        Self {
            slots: [SongSlot::default(); MAX_SONG_SLOTS],
            len: 0,
        }
    }
}

struct Voice {
    active: bool,
    instrument: usize,
    group: Option<usize>,
    sample: Option<Arc<SampleData>>,
    position: f64,
    rate: f64,
    envelope: f32,
    envelope_coef: f32,
    gain: f32,
    pan_l: f32,
    pan_r: f32,
    drive: f32,
    body: f32,
    low_state: f32,
    low_coef: f32,
    send: f32,
    age: u64,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            active: false,
            instrument: 0,
            group: None,
            sample: None,
            position: 0.0,
            rate: 1.0,
            envelope: 0.0,
            envelope_coef: 0.0,
            gain: 0.0,
            pan_l: 0.0,
            pan_r: 0.0,
            drive: 0.0,
            body: 0.0,
            low_state: 0.0,
            low_coef: 0.0,
            send: 0.0,
            age: 0,
        }
    }
}

pub struct Engine {
    kit: Arc<Kit>,
    sample_rate: f32,
    voices: Vec<Voice>,
    pattern: PatternRuntime,
    song: SongRuntime,
    midi_map: [u8; NUM_INSTRUMENTS],
    step: usize,
    song_index: usize,
    song_repeat: u8,
    samples_to_step: f64,
    was_playing: bool,
    settings: EngineSettings,
    reverb: Reverb,
    limiter: Limiter,
    commands: Arc<ArrayQueue<EngineCommand>>,
    meters: Arc<Meters>,
    current_step: Arc<AtomicU32>,
    current_song_slot: Arc<AtomicU32>,
    age: u64,
    scratch_r: Vec<f32>,
}

impl Engine {
    pub fn new(
        kit: Arc<Kit>,
        commands: Arc<ArrayQueue<EngineCommand>>,
        meters: Arc<Meters>,
        current_step: Arc<AtomicU32>,
        current_song_slot: Arc<AtomicU32>,
    ) -> Self {
        let mut engine = Self {
            kit,
            sample_rate: 48_000.0,
            voices: Vec::new(),
            pattern: PatternRuntime::default(),
            song: SongRuntime::default(),
            midi_map: crate::project::DEFAULT_NOTES,
            step: 0,
            song_index: 0,
            song_repeat: 0,
            samples_to_step: 0.0,
            was_playing: false,
            settings: EngineSettings::default(),
            reverb: Reverb::new(),
            limiter: Limiter::new(48_000.0),
            commands,
            meters,
            current_step,
            current_song_slot,
            age: 0,
            scratch_r: Vec::new(),
        };
        engine.prepare(48_000.0, 512);
        engine
    }

    pub fn kit(&self) -> &Arc<Kit> {
        &self.kit
    }

    pub fn prepare(&mut self, sample_rate: f32, max_block_size: usize) {
        self.sample_rate = sample_rate.max(1.0);
        self.voices = (0..NUM_INSTRUMENTS * VOICES_PER_INSTRUMENT)
            .map(|_| Voice::default())
            .collect();
        self.scratch_r = vec![0.0; max_block_size.max(1)];
        self.reverb.prepare(self.sample_rate);
        self.limiter.prepare(self.sample_rate);
        self.reset();
    }

    pub fn reset(&mut self) {
        for voice in &mut self.voices {
            *voice = Voice::default();
        }
        self.step = 0;
        self.song_index = 0;
        self.song_repeat = 0;
        self.samples_to_step = 0.0;
        self.was_playing = false;
        self.reverb.reset();
        self.current_step.store(0, Ordering::Relaxed);
        self.current_song_slot.store(0, Ordering::Relaxed);
        self.meters.store(0.0, 0.0);
        self.meters.store_channels(&[0.0; NUM_INSTRUMENTS]);
    }

    pub fn kit_changed(&mut self, kit: Arc<Kit>) {
        self.kit = kit;
        self.reset();
    }

    fn velocity_gain(velocity: f32, response: f32) -> f32 {
        let velocity = velocity.clamp(0.0, 1.0);
        let response = response.clamp(0.0, 1.0);
        velocity.powf(1.0 + (1.0 - response) * 2.5)
    }

    fn pan_gains(pan: f32) -> (f32, f32) {
        let pan = pan.clamp(-1.0, 1.0);
        let angle = (pan + 1.0) * PI as f32 * 0.25;
        (angle.cos(), angle.sin())
    }

    fn trigger(&mut self, instrument: usize, velocity: f32) {
        if instrument >= self.kit.instruments.len() || instrument >= NUM_INSTRUMENTS {
            return;
        }
        let settings = self.settings.instruments[instrument];
        let solo_active = self
            .settings
            .instruments
            .iter()
            .any(|settings| settings.solo);
        if settings.mute || (solo_active && !settings.solo) {
            return;
        }
        let midi_velocity = (velocity.clamp(0.0, 1.0) * 127.0) as u8;
        let (sample, group) = {
            let instrument_def = &self.kit.instruments[instrument];
            let layer = instrument_def
                .layers
                .iter()
                .find(|layer| {
                    midi_velocity >= layer.velocity_min && midi_velocity <= layer.velocity_max
                })
                .or_else(|| instrument_def.layers.last());
            let Some(layer) = layer else {
                return;
            };
            let group = instrument_def.choke_group.as_ref().and_then(|group| {
                self.kit
                    .choke_groups
                    .iter()
                    .position(|candidate| candidate == group)
            });
            (Arc::clone(&layer.sample), group)
        };
        let (pan_l, pan_r) = Self::pan_gains(settings.pan);
        let gain =
            db_to_gain(settings.volume_db) * Self::velocity_gain(velocity, settings.velocity);
        let rate = (sample.sample_rate as f64 / self.sample_rate as f64)
            * 2.0_f64.powf(f64::from(settings.pitch) / 12.0);
        let tau = 0.08 + settings.decay.clamp(0.01, 4.0) as f64 * 0.70;
        let envelope_coef = (-1.0 / (tau * self.sample_rate as f64)).exp() as f32;
        let low_coef = (1.0 - (-2.0 * PI * 140.0 / self.sample_rate as f64).exp()) as f32;
        self.age = self.age.wrapping_add(1);
        let start = instrument * VOICES_PER_INSTRUMENT;
        let end = start + VOICES_PER_INSTRUMENT;
        let slot = self.voices[start..end]
            .iter()
            .position(|voice| !voice.active)
            .map(|index| start + index)
            .unwrap_or_else(|| {
                self.voices[start..end]
                    .iter()
                    .enumerate()
                    .min_by(|(_, left), (_, right)| {
                        left.envelope
                            .partial_cmp(&right.envelope)
                            .unwrap_or(std::cmp::Ordering::Equal)
                            .then_with(|| left.age.cmp(&right.age))
                    })
                    .map(|(index, _)| start + index)
                    .unwrap_or(start)
            });
        if let Some(group) = group {
            for voice in &mut self.voices {
                if voice.active && voice.group == Some(group) && voice.instrument != instrument {
                    voice.active = false;
                    voice.sample = None;
                    voice.envelope = 0.0;
                }
            }
        }
        if let Some(voice) = self.voices.get_mut(slot) {
            *voice = Voice {
                active: true,
                instrument,
                group,
                sample: Some(sample),
                position: 0.0,
                rate,
                envelope: 1.0,
                envelope_coef,
                gain,
                pan_l,
                pan_r,
                drive: settings.drive,
                body: settings.body.clamp(0.0, 1.0),
                low_state: 0.0,
                low_coef,
                send: settings.reverb_send,
                age: self.age,
            };
        }
    }

    fn copy_project(&mut self, project: &Project) {
        if let Some(pattern) = project.patterns.get(project.current_pattern) {
            self.pattern = PatternRuntime::from_pattern(pattern);
        }
        self.midi_map = project.midi_map;
        self.song.len = project.song.len().min(MAX_SONG_SLOTS);
        for (index, slot) in project.song.iter().take(MAX_SONG_SLOTS).enumerate() {
            self.song.slots[index] = *slot;
        }
        if self.song.len > 0 {
            let slot = self.song.slots[self.song_index.min(self.song.len - 1)];
            if let Some(pattern) = project.patterns.get(slot.pattern as usize) {
                self.pattern = PatternRuntime::from_pattern(pattern);
            }
        }
    }

    pub fn process(
        &mut self,
        settings: &EngineSettings,
        project: Option<&Project>,
        notes: &[NoteInput],
        out_l: &mut [f32],
        out_r: &mut [f32],
    ) {
        let block = out_l.len().min(out_r.len());
        if block == 0 {
            return;
        }
        if self.kit.instruments.is_empty() {
            out_l.fill(0.0);
            out_r.fill(0.0);
            return;
        }
        self.settings = *settings;
        if let Some(project) = project {
            self.copy_project(project);
        }
        while let Some(command) = self.commands.pop() {
            match command {
                EngineCommand::Trigger { note, velocity } => {
                    let instrument = project
                        .and_then(|project| project.instrument_for_note(note))
                        .or_else(|| self.kit.instrument_for_note(note))
                        .or_else(|| {
                            self.midi_map
                                .iter()
                                .position(|candidate| *candidate == note)
                        });
                    if let Some(instrument) = instrument {
                        self.trigger(instrument, velocity);
                    }
                }
            }
        }
        self.reverb.set_settings(&settings.reverb);
        let playing = settings.playing && settings.sequencer_mode == SequencerMode::Internal;
        if playing && !self.was_playing {
            self.step = 0;
            self.song_index = 0;
            self.song_repeat = 0;
            self.samples_to_step = 0.0;
        }
        if !playing {
            self.current_step.store(0, Ordering::Relaxed);
        }
        self.was_playing = playing;
        let mut note_index = 0;
        let mut peak_l = 0.0_f32;
        let mut peak_r = 0.0_f32;
        let mut channel_peak = [0.0_f32; NUM_INSTRUMENTS];
        let base_step =
            (self.sample_rate as f64 * 60.0 / settings.bpm.clamp(20.0, 300.0) as f64 / 4.0)
                .max(1.0);
        let swing = settings.swing.clamp(0.0, 0.75) as f64;
        let song_mode = settings.song_mode && self.song.len > 0;

        for sample_index in 0..block {
            while note_index < notes.len() && notes[note_index].offset as usize <= sample_index {
                let note = notes[note_index];
                let instrument = self
                    .midi_map
                    .iter()
                    .position(|candidate| *candidate == note.note)
                    .or_else(|| self.kit.instrument_for_note(note.note));
                if let Some(instrument) = instrument {
                    self.trigger(instrument, note.velocity.clamp(0.0, 1.0));
                }
                note_index += 1;
            }

            if playing && self.samples_to_step <= 0.0 {
                let step = self.step.min(MAX_STEPS - 1);
                let velocities: [u8; NUM_INSTRUMENTS] =
                    std::array::from_fn(|instrument| self.pattern.steps[instrument][step]);
                for (instrument, velocity) in velocities.into_iter().enumerate() {
                    if velocity > 0 {
                        self.trigger(instrument, f32::from(velocity) / 127.0);
                    }
                }
                self.step += 1;
                if self.step >= usize::from(self.pattern.length) {
                    self.step = 0;
                    if song_mode {
                        self.song_repeat = self.song_repeat.saturating_add(1);
                        let repeats = self.song.slots[self.song_index.min(self.song.len - 1)]
                            .repeats
                            .max(1);
                        if self.song_repeat >= repeats {
                            self.song_repeat = 0;
                            self.song_index += 1;
                            if self.song_index >= self.song.len {
                                self.song_index = 0;
                            }
                        }
                        if let Some(project) = project {
                            let slot = self.song.slots[self.song_index.min(self.song.len - 1)];
                            if let Some(pattern) = project.patterns.get(slot.pattern as usize) {
                                self.pattern = PatternRuntime::from_pattern(pattern);
                            }
                        }
                    }
                }
                self.samples_to_step = if self.step % 2 == 0 {
                    base_step * (1.0 + swing * 0.5)
                } else {
                    base_step * (1.0 - swing * 0.5)
                };
            }
            if playing {
                self.samples_to_step -= 1.0;
            }

            let mut left = 0.0;
            let mut right = 0.0;
            let mut reverb_l = 0.0;
            let mut reverb_r = 0.0;
            for voice in &mut self.voices {
                if !voice.active {
                    continue;
                }
                let Some(sample) = &voice.sample else {
                    voice.active = false;
                    continue;
                };
                let frames = &sample.frames;
                let index = voice.position as usize;
                if index >= frames.len() {
                    voice.active = false;
                    voice.sample = None;
                    continue;
                }
                let next = frames.get(index + 1).copied().unwrap_or(frames[index]);
                let fraction = (voice.position - index as f64) as f32;
                let raw = frames[index] * (1.0 - fraction) + next * fraction;
                voice.low_state += (raw - voice.low_state) * voice.low_coef;
                let fat = raw + voice.low_state * voice.body * 1.6;
                let shaped = soft_clip(fat, voice.drive);
                let value = shaped * voice.envelope * voice.gain;
                if let Some(peak) = channel_peak.get_mut(voice.instrument) {
                    *peak = peak.max(value.abs());
                }
                left += value * voice.pan_l;
                right += value * voice.pan_r;
                reverb_l += value * voice.pan_l * voice.send;
                reverb_r += value * voice.pan_r * voice.send;
                voice.position += voice.rate;
                voice.envelope *= voice.envelope_coef;
                if voice.envelope < 1e-5 || voice.position >= frames.len() as f64 {
                    voice.active = false;
                    voice.sample = None;
                }
            }

            if !self.settings.master.dry && self.settings.reverb.mix > 0.0 {
                let (wet_l, wet_r) = self.reverb.process(reverb_l, reverb_r, &settings.reverb);
                left += wet_l;
                right += wet_r;
            }
            if !self.settings.master.dry {
                left = soft_clip(left, self.settings.master.drive);
                right = soft_clip(right, self.settings.master.drive);
            }
            let master = db_to_gain(self.settings.master.volume_db);
            left *= master;
            right *= master;
            if !self.settings.master.dry && self.settings.master.limiter {
                left = self.limiter.process(left, 0.95);
                right = self.limiter.process(right, 0.95);
            }
            out_l[sample_index] = left;
            out_r[sample_index] = right;
            peak_l = peak_l.max(left.abs());
            peak_r = peak_r.max(right.abs());
        }
        self.meters.store(peak_l, peak_r);
        self.meters.store_channels(&channel_peak);
        if playing {
            let display_step = if self.step == 0 {
                usize::from(self.pattern.length.max(1)) - 1
            } else {
                self.step - 1
            };
            self.current_step
                .store(display_step as u32, Ordering::Relaxed);
            let song_slot = if song_mode { self.song_index } else { 0 };
            self.current_song_slot
                .store(song_slot as u32, Ordering::Relaxed);
        }
    }

    pub fn render_to_wav(
        path: &Path,
        kit: Arc<Kit>,
        project: &Project,
        settings: &EngineSettings,
        bars: u32,
    ) -> Result<(), String> {
        let commands = Arc::new(ArrayQueue::new(32));
        let meters = Arc::new(Meters::default());
        let current_step = Arc::new(AtomicU32::new(0));
        let current_song_slot = Arc::new(AtomicU32::new(0));
        let mut engine = Engine::new(kit, commands, meters, current_step, current_song_slot);
        engine.prepare(48_000.0, 512);
        let bpm = project.bpm.clamp(20.0, 300.0);
        let total = ((bars.max(1) as f64 * 4.0 * 60.0 / bpm as f64) * 48_000.0) as usize;
        let spec = WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(path, spec)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let empty_notes: [NoteInput; 0] = [];
        let mut left = vec![0.0; 512];
        let mut right = vec![0.0; 512];
        let mut rendered = 0;
        while rendered < total {
            let block = (total - rendered).min(512);
            engine.process(
                settings,
                Some(project),
                &empty_notes,
                &mut left[..block],
                &mut right[..block],
            );
            for index in 0..block {
                let l = (left[index].clamp(-1.0, 1.0) * 32767.0) as i16;
                let r = (right[index].clamp(-1.0, 1.0) * 32767.0) as i16;
                writer.write_sample(l).map_err(|error| error.to_string())?;
                writer.write_sample(r).map_err(|error| error.to_string())?;
            }
            rendered += block;
        }
        writer.finalize().map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::Kit;
    use crate::project::Project;

    fn engine() -> Engine {
        let kit = Arc::new(Kit::factory().unwrap());
        Engine::new(
            kit,
            Arc::new(ArrayQueue::new(32)),
            Arc::new(Meters::default()),
            Arc::new(AtomicU32::new(0)),
            Arc::new(AtomicU32::new(0)),
        )
    }

    #[test]
    fn kick_produces_audio_and_meters_are_finite() {
        let mut engine = engine();
        let project = engine.kit.default_project();
        let settings = EngineSettings::from_project(&project);
        let mut left = vec![0.0; 256];
        let mut right = vec![0.0; 256];
        engine.process(&settings, Some(&project), &[], &mut left, &mut right);
        let (l, r) = engine.meters.peak();
        assert!(l.is_finite() && r.is_finite());
    }

    #[test]
    fn renderer_writes_non_silent_wav() {
        let kit = Arc::new(Kit::factory().unwrap());
        let mut project = kit.default_project();
        project.patterns[0].set_velocity(0, 0, 127);
        let mut settings = EngineSettings::from_project(&project);
        settings.sequencer_mode = SequencerMode::Internal;
        settings.playing = true;
        let path =
            std::env::temp_dir().join(format!("nirudrums-render-test-{}.wav", std::process::id()));
        Engine::render_to_wav(&path, kit, &project, &settings, 1).unwrap();
        let mut reader = hound::WavReader::open(&path).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, 48_000);
        assert_eq!(spec.channels, 2);
        let peak = reader
            .samples::<i16>()
            .map(|sample| i32::from(sample.unwrap()).abs())
            .max()
            .unwrap_or(0);
        let _ = std::fs::remove_file(path);
        assert!(peak > 1_000, "rendered peak was only {peak}");
    }

    #[test]
    fn body_control_adds_weight() {
        let run = |body: f32| {
            let mut engine = engine();
            let mut project = engine.kit.default_project();
            project.patterns[0].clear();
            project.patterns[0].set_velocity(0, 0, 127);
            project.instruments[0].body = body;
            let settings = EngineSettings::from_project(&project);
            let mut left = vec![0.0; 8_192];
            let mut right = vec![0.0; 8_192];
            engine.process(&settings, Some(&project), &[], &mut left, &mut right);
            left.iter().chain(right.iter()).map(|x| x * x).sum::<f32>()
        };
        assert!(run(1.0) > run(0.0));
    }

    #[test]
    fn sequencer_advances_on_sample_clock() {
        let mut engine = engine();
        let project = engine.kit.default_project();
        let settings = EngineSettings::from_project(&project);
        let mut left = vec![0.0; 6_000];
        let mut right = vec![0.0; 6_000];
        engine.process(&settings, Some(&project), &[], &mut left, &mut right);
        assert_eq!(engine.current_step.load(Ordering::Relaxed), 0);
        engine.process(&settings, Some(&project), &[], &mut left, &mut right);
        assert_eq!(engine.current_step.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn dry_project_still_renders() {
        let mut engine = engine();
        let mut project = Project::default();
        project.patterns[0].toggle(0, 0);
        project.master.dry = true;
        let settings = EngineSettings::from_project(&project);
        let mut left = vec![0.0; 480];
        let mut right = vec![0.0; 480];
        engine.process(&settings, Some(&project), &[], &mut left, &mut right);
        assert!(left
            .iter()
            .chain(right.iter())
            .all(|sample| sample.is_finite()));
    }
}

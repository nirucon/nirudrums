#![allow(semicolon_in_expressions_from_non_local_macros)]

use crossbeam_queue::ArrayQueue;
use nih_plug::prelude::*;
use nirudrums_core::{
    Engine, EngineCommand, EngineSettings, Kit, Meters, NoteInput, SequencerMode,
};
use parking_lot::RwLock;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

mod params;
mod ui;
pub use params::NirudrumsParams;

pub struct Nirudrums {
    params: Arc<NirudrumsParams>,
    kit: Arc<RwLock<Arc<Kit>>>,
    kit_swap: Arc<ArrayQueue<Arc<Kit>>>,
    engine: Engine,
    queue: Arc<ArrayQueue<EngineCommand>>,
    meters: Arc<Meters>,
    current_step: Arc<AtomicU32>,
}

impl Default for Nirudrums {
    fn default() -> Self {
        let params = Arc::new(NirudrumsParams::default());
        let kit = Arc::new(Kit::factory().expect("embedded MERCY-80 kit must be valid"));
        let queue = Arc::new(ArrayQueue::new(256));
        let meters = Arc::new(Meters::default());
        let current_step = Arc::new(AtomicU32::new(0));
        let kit_swap = Arc::new(ArrayQueue::new(4));
        let kit_slot = Arc::new(RwLock::new(kit.clone()));
        let engine = Engine::new(kit, queue.clone(), meters.clone(), current_step.clone());
        Self {
            params,
            kit: kit_slot,
            kit_swap,
            engine,
            queue,
            meters,
            current_step,
        }
    }
}

impl Plugin for Nirudrums {
    const NAME: &'static str = "NIRUDRUMS";
    const VENDOR: &'static str = "NIRU";
    const URL: &'static str = "https://github.com/nirucon/nirudrums";
    const EMAIL: &'static str = "niru@nirucon.dev";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        ui::create_editor(
            self.params.clone(),
            self.kit.clone(),
            self.kit_swap.clone(),
            self.queue.clone(),
            self.meters.clone(),
            self.current_step.clone(),
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.engine.prepare(
            buffer_config.sample_rate,
            buffer_config.max_buffer_size as usize,
        );
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        while let Some(kit) = self.kit_swap.pop() {
            self.engine.kit_changed(kit.clone());
            *self.kit.write() = kit;
        }

        let mut notes = [NoteInput {
            offset: 0,
            note: 0,
            velocity: 0.0,
        }; 256];
        let mut note_count = 0;
        while let Some(event) = context.next_event() {
            if let NoteEvent::NoteOn {
                timing,
                note,
                velocity,
                ..
            } = event
            {
                if note_count < notes.len() {
                    notes[note_count] = NoteInput {
                        offset: timing,
                        note,
                        velocity,
                    };
                    note_count += 1;
                }
            }
        }

        let host_tempo = context.transport().tempo;
        let follows_host = self.params.sequencer.follow_host.value();
        let bpm = if follows_host {
            host_tempo
                .map(|tempo| tempo as f32)
                .unwrap_or_else(|| self.params.sequencer.bpm.value())
        } else {
            self.params.sequencer.bpm.value()
        };
        let playing = if follows_host {
            context.transport().playing
        } else {
            self.params.sequencer.playing.value()
        };
        let settings = EngineSettings {
            instruments: self.params.instrument_settings(),
            master: self.params.master.settings(),
            reverb: self.params.reverb.settings(),
            bpm,
            swing: self.params.sequencer.swing.value(),
            playing,
            song_mode: self.params.sequencer.song_mode.value(),
            sequencer_mode: match self.params.sequencer.mode.value() {
                params::SequencerModeParam::Internal => SequencerMode::Internal,
                params::SequencerModeParam::Midi => SequencerMode::Midi,
            },
        };

        let project_guard = self.params.project.try_read();
        let project = project_guard.as_deref().ok();
        let channels = buffer.as_slice();
        if channels.len() < 2 {
            for channel in channels {
                channel.fill(0.0);
            }
            return ProcessStatus::Normal;
        }
        let (left, right) = channels.split_at_mut(1);
        self.engine
            .process(&settings, project, &notes[..note_count], left[0], right[0]);
        ProcessStatus::Normal
    }
}

impl Vst3Plugin for Nirudrums {
    const VST3_CLASS_ID: [u8; 16] = *b"NiruDrumsMercy80";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Drum];
}

nih_export_vst3!(Nirudrums);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameter_ids_are_unique() {
        let params = NirudrumsParams::default();
        let map = params.param_map();
        let mut seen = std::collections::HashSet::new();
        for (id, _, _) in &map {
            assert!(seen.insert(id.clone()), "duplicate parameter id: {id}");
        }
        assert_eq!(seen.len(), map.len());
    }
}

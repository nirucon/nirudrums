//! NIRUDRUMS audio engine, sampler, sequencer, kits and project format.

pub mod dsp;
pub mod engine;
mod factory_assets;
pub mod kit;
pub mod project;

pub use dsp::{db_to_gain, soft_clip, Limiter, Reverb};
pub use engine::{Engine, EngineCommand, EngineSettings, Meters, NoteInput, VOICES_PER_INSTRUMENT};
pub use kit::{
    available_kits, data_dir, decode_wav, factory_presets, kit_dirs, Kit, SampleData, VelocityLayer,
};
pub use project::{
    InstrumentSettings, MasterSettings, Pattern, Preset, Project, ReverbMode, ReverbSettings,
    SequencerMode, SongSlot, DEFAULT_NOTES, INSTRUMENT_IDS, MAX_PATTERNS, MAX_SONG_SLOTS,
    MAX_STEPS, NUM_INSTRUMENTS, SCHEMA,
};

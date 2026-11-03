use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use nirudrums_core::{
    InstrumentSettings, Kit, MasterSettings, Project, ReverbMode, ReverbSettings, SequencerMode,
    NUM_INSTRUMENTS,
};
use std::sync::{Arc, RwLock};

#[derive(Enum, Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum ReverbModeParam {
    #[id = "plate"]
    #[default]
    Plate,
    #[id = "room"]
    Room,
    #[id = "gated"]
    Gated,
}

impl From<ReverbModeParam> for ReverbMode {
    fn from(value: ReverbModeParam) -> Self {
        match value {
            ReverbModeParam::Plate => ReverbMode::Plate,
            ReverbModeParam::Room => ReverbMode::Room,
            ReverbModeParam::Gated => ReverbMode::Gated,
        }
    }
}

#[derive(Enum, Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum SequencerModeParam {
    #[id = "internal"]
    #[default]
    Internal,
    #[id = "midi"]
    Midi,
}

impl From<SequencerModeParam> for SequencerMode {
    fn from(value: SequencerModeParam) -> Self {
        match value {
            SequencerModeParam::Internal => SequencerMode::Internal,
            SequencerModeParam::Midi => SequencerMode::Midi,
        }
    }
}

#[derive(Params)]
pub struct InstrumentParams {
    #[id = "vol"]
    pub volume_db: FloatParam,
    #[id = "pan"]
    pub pan: FloatParam,
    #[id = "pitch"]
    pub pitch: FloatParam,
    #[id = "decay"]
    pub decay: FloatParam,
    #[id = "body"]
    pub body: FloatParam,
    #[id = "velocity"]
    pub velocity: FloatParam,
    #[id = "drive"]
    pub drive: FloatParam,
    #[id = "send"]
    pub reverb_send: FloatParam,
    #[id = "mute"]
    pub mute: BoolParam,
    #[id = "solo"]
    pub solo: BoolParam,
}

impl InstrumentParams {
    pub fn from_settings(settings: InstrumentSettings) -> Self {
        Self {
            volume_db: FloatParam::new(
                "Volume",
                settings.volume_db,
                FloatRange::Linear {
                    min: -60.0,
                    max: 12.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_unit(" dB"),
            pan: FloatParam::new(
                "Pan",
                settings.pan,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0)),
            pitch: FloatParam::new(
                "Pitch",
                settings.pitch,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_unit(" st"),
            decay: FloatParam::new(
                "Decay",
                settings.decay,
                FloatRange::Linear { min: 0.1, max: 4.0 },
            ),
            body: FloatParam::new(
                "Body",
                settings.body,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(formatters::v2s_f32_percentage(0)),
            velocity: FloatParam::new(
                "Velocity",
                settings.velocity,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(formatters::v2s_f32_percentage(0)),
            drive: FloatParam::new(
                "Drive",
                settings.drive,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(formatters::v2s_f32_percentage(0)),
            reverb_send: FloatParam::new(
                "Reverb Send",
                settings.reverb_send,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(formatters::v2s_f32_percentage(0)),
            mute: BoolParam::new("Mute", settings.mute),
            solo: BoolParam::new("Solo", settings.solo),
        }
    }

    pub fn settings(&self) -> InstrumentSettings {
        InstrumentSettings {
            volume_db: self.volume_db.value(),
            pan: self.pan.value(),
            pitch: self.pitch.value(),
            decay: self.decay.value(),
            body: self.body.value(),
            velocity: self.velocity.value(),
            drive: self.drive.value(),
            reverb_send: self.reverb_send.value(),
            mute: self.mute.value(),
            solo: self.solo.value(),
            output: 0,
        }
    }
}

#[derive(Params)]
pub struct MasterParams {
    #[id = "vol"]
    pub volume_db: FloatParam,
    #[id = "drive"]
    pub drive: FloatParam,
    #[id = "limiter"]
    pub limiter: BoolParam,
    #[id = "dry"]
    pub dry: BoolParam,
}

impl MasterParams {
    pub fn from_settings(settings: MasterSettings) -> Self {
        Self {
            volume_db: FloatParam::new(
                "Master Volume",
                settings.volume_db,
                FloatRange::Linear {
                    min: -60.0,
                    max: 12.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_unit(" dB"),
            drive: FloatParam::new(
                "Master Drive",
                settings.drive,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(formatters::v2s_f32_percentage(0)),
            limiter: BoolParam::new("Limiter", settings.limiter),
            dry: BoolParam::new("Dry Bypass", settings.dry),
        }
    }

    pub fn settings(&self) -> MasterSettings {
        MasterSettings {
            volume_db: self.volume_db.value(),
            drive: self.drive.value(),
            limiter: self.limiter.value(),
            dry: self.dry.value(),
        }
    }
}

#[derive(Params)]
pub struct ReverbParams {
    #[id = "mode"]
    pub mode: EnumParam<ReverbModeParam>,
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "decay"]
    pub decay: FloatParam,
    #[id = "predelay"]
    pub pre_delay_ms: FloatParam,
    #[id = "damping"]
    pub damping: FloatParam,
    #[id = "width"]
    pub width: FloatParam,
    #[id = "gate"]
    pub gate_ms: FloatParam,
}

impl ReverbParams {
    pub fn from_settings(settings: ReverbSettings) -> Self {
        Self {
            mode: EnumParam::new(
                "Mode",
                match settings.mode {
                    ReverbMode::Plate => ReverbModeParam::Plate,
                    ReverbMode::Room => ReverbModeParam::Room,
                    ReverbMode::Gated => ReverbModeParam::Gated,
                },
            ),
            mix: FloatParam::new(
                "Mix",
                settings.mix,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(formatters::v2s_f32_percentage(0)),
            decay: FloatParam::new(
                "Decay",
                settings.decay,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            pre_delay_ms: FloatParam::new(
                "Pre-delay",
                settings.pre_delay_ms,
                FloatRange::Linear {
                    min: 0.0,
                    max: 200.0,
                },
            )
            .with_unit(" ms"),
            damping: FloatParam::new(
                "Damping",
                settings.damping,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            width: FloatParam::new(
                "Width",
                settings.width,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            gate_ms: FloatParam::new(
                "Gate",
                settings.gate_ms,
                FloatRange::Linear {
                    min: 0.0,
                    max: 2000.0,
                },
            )
            .with_unit(" ms"),
        }
    }

    pub fn settings(&self) -> ReverbSettings {
        ReverbSettings {
            mode: self.mode.value().into(),
            mix: self.mix.value(),
            decay: self.decay.value(),
            pre_delay_ms: self.pre_delay_ms.value(),
            damping: self.damping.value(),
            width: self.width.value(),
            gate_ms: self.gate_ms.value(),
        }
    }
}

#[derive(Params)]
pub struct SequencerParams {
    #[id = "bpm"]
    pub bpm: FloatParam,
    #[id = "swing"]
    pub swing: FloatParam,
    #[id = "clock"]
    pub mode: EnumParam<SequencerModeParam>,
    #[id = "follow"]
    pub follow_host: BoolParam,
    #[id = "playing"]
    pub playing: BoolParam,
    #[id = "song"]
    pub song_mode: BoolParam,
}

impl SequencerParams {
    pub fn from_project(project: &Project) -> Self {
        Self {
            bpm: FloatParam::new(
                "BPM",
                project.bpm,
                FloatRange::Linear {
                    min: 20.0,
                    max: 300.0,
                },
            ),
            swing: FloatParam::new(
                "Swing",
                project.swing,
                FloatRange::Linear {
                    min: 0.0,
                    max: 0.75,
                },
            )
            .with_value_to_string(formatters::v2s_f32_percentage(0)),
            mode: EnumParam::new(
                "Sequencer",
                match project.sequencer_mode {
                    SequencerMode::Internal => SequencerModeParam::Internal,
                    SequencerMode::Midi => SequencerModeParam::Midi,
                },
            ),
            follow_host: BoolParam::new("Follow Host", project.follow_host),
            playing: BoolParam::new("Play", false),
            song_mode: BoolParam::new("Song Mode", project.song_mode),
        }
    }
}

#[derive(Params)]
pub struct NirudrumsParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,
    #[persist = "project"]
    pub project: Arc<RwLock<Project>>,
    #[nested(array, group = "Drums")]
    pub instruments: [InstrumentParams; NUM_INSTRUMENTS],
    #[nested(group = "Master")]
    pub master: MasterParams,
    #[nested(group = "Reverb")]
    pub reverb: ReverbParams,
    #[nested(group = "Sequencer")]
    pub sequencer: SequencerParams,
}

impl Default for NirudrumsParams {
    fn default() -> Self {
        let kit = Kit::factory().ok();
        let project = kit
            .as_ref()
            .map(|kit| kit.default_project())
            .unwrap_or_default();
        let instruments = std::array::from_fn(|index| {
            let settings = kit
                .as_ref()
                .map(|kit| kit.defaults(index))
                .unwrap_or_default();
            InstrumentParams::from_settings(settings)
        });
        Self {
            editor_state: EguiState::from_size(1280, 820),
            project: Arc::new(RwLock::new(project.clone())),
            instruments,
            master: MasterParams::from_settings(project.master),
            reverb: ReverbParams::from_settings(project.reverb),
            sequencer: SequencerParams::from_project(&project),
        }
    }
}

impl NirudrumsParams {
    pub fn instrument_settings(&self) -> [InstrumentSettings; NUM_INSTRUMENTS] {
        std::array::from_fn(|index| self.instruments[index].settings())
    }
}

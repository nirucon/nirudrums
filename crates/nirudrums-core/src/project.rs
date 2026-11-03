use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const NUM_INSTRUMENTS: usize = 12;
pub const MAX_STEPS: usize = 64;
pub const MAX_PATTERNS: usize = 32;
pub const MAX_SONG_SLOTS: usize = 64;
pub const SCHEMA: u32 = 1;

pub const INSTRUMENT_IDS: [&str; NUM_INSTRUMENTS] = [
    "kick",
    "snare",
    "closed_hat",
    "open_hat",
    "crash1",
    "crash2",
    "ride",
    "high_tom",
    "mid_tom",
    "low_tom",
    "clap",
    "rimshot",
];

pub const DEFAULT_NOTES: [u8; NUM_INSTRUMENTS] = [36, 38, 42, 46, 49, 57, 51, 50, 47, 43, 39, 37];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SequencerMode {
    #[default]
    Internal,
    Midi,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReverbMode {
    #[default]
    Plate,
    Room,
    Gated,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InstrumentSettings {
    pub volume_db: f32,
    pub pan: f32,
    pub pitch: f32,
    pub decay: f32,
    pub body: f32,
    pub velocity: f32,
    pub drive: f32,
    pub reverb_send: f32,
    pub mute: bool,
    pub solo: bool,
    pub output: u8,
}

impl Default for InstrumentSettings {
    fn default() -> Self {
        Self {
            volume_db: 0.0,
            pan: 0.0,
            pitch: 0.0,
            decay: 1.0,
            body: 0.0,
            velocity: 0.8,
            drive: 0.0,
            reverb_send: 0.0,
            mute: false,
            solo: false,
            output: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MasterSettings {
    pub volume_db: f32,
    pub drive: f32,
    pub limiter: bool,
    pub dry: bool,
}

impl Default for MasterSettings {
    fn default() -> Self {
        Self {
            volume_db: 0.0,
            drive: 0.0,
            limiter: true,
            dry: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReverbSettings {
    pub mode: ReverbMode,
    pub mix: f32,
    pub decay: f32,
    pub pre_delay_ms: f32,
    pub damping: f32,
    pub width: f32,
    pub gate_ms: f32,
}

impl Default for ReverbSettings {
    fn default() -> Self {
        Self {
            mode: ReverbMode::Plate,
            mix: 0.16,
            decay: 0.45,
            pre_delay_ms: 18.0,
            damping: 0.4,
            width: 0.9,
            gate_ms: 0.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Pattern {
    pub name: String,
    pub length: u8,
    pub steps: Vec<Vec<u8>>,
}

impl Default for Pattern {
    fn default() -> Self {
        Self {
            name: "Pattern".into(),
            length: 16,
            steps: vec![vec![0; MAX_STEPS]; NUM_INSTRUMENTS],
        }
    }
}

impl Pattern {
    pub fn named(name: impl Into<String>, length: u8) -> Self {
        Self {
            name: name.into(),
            length: length.clamp(1, MAX_STEPS as u8),
            steps: vec![vec![0; MAX_STEPS]; NUM_INSTRUMENTS],
        }
    }

    pub fn velocity(&self, instrument: usize, step: usize) -> u8 {
        self.steps
            .get(instrument)
            .and_then(|row| row.get(step))
            .copied()
            .unwrap_or(0)
    }

    pub fn set_velocity(&mut self, instrument: usize, step: usize, velocity: u8) {
        if instrument < NUM_INSTRUMENTS && step < MAX_STEPS {
            while self.steps.len() <= instrument {
                self.steps.push(vec![0; MAX_STEPS]);
            }
            if let Some(row) = self.steps.get_mut(instrument) {
                if row.len() <= step {
                    row.resize(MAX_STEPS, 0);
                }
                row[step] = velocity.min(127);
            }
        }
    }

    pub fn toggle(&mut self, instrument: usize, step: usize) {
        let current = self.velocity(instrument, step);
        self.set_velocity(instrument, step, if current > 0 { 0 } else { 100 });
    }

    pub fn clear(&mut self) {
        self.steps = vec![vec![0; MAX_STEPS]; NUM_INSTRUMENTS];
    }

    pub fn copy_from(&mut self, other: &Pattern) {
        self.name = other.name.clone();
        self.length = other.length;
        self.steps = other.steps.clone();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SongSlot {
    pub pattern: u8,
    pub repeats: u8,
}

impl Default for SongSlot {
    fn default() -> Self {
        Self {
            pattern: 0,
            repeats: 1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub schema: u32,
    pub kit: String,
    pub bpm: f32,
    pub swing: f32,
    pub current_pattern: usize,
    pub patterns: Vec<Pattern>,
    pub song: Vec<SongSlot>,
    pub song_mode: bool,
    pub sequencer_mode: SequencerMode,
    pub follow_host: bool,
    pub midi_map: [u8; NUM_INSTRUMENTS],
    pub instruments: [InstrumentSettings; NUM_INSTRUMENTS],
    pub master: MasterSettings,
    pub reverb: ReverbSettings,
}

impl Default for Project {
    fn default() -> Self {
        let mut patterns = Vec::new();
        for index in 0..4 {
            patterns.push(Pattern::named(format!("Pattern {}", index + 1), 16));
        }
        Self {
            schema: SCHEMA,
            kit: "mercy-80".into(),
            bpm: 120.0,
            swing: 0.0,
            current_pattern: 0,
            patterns,
            song: Vec::new(),
            song_mode: false,
            sequencer_mode: SequencerMode::Internal,
            follow_host: false,
            midi_map: DEFAULT_NOTES,
            instruments: [InstrumentSettings::default(); NUM_INSTRUMENTS],
            master: MasterSettings::default(),
            reverb: ReverbSettings::default(),
        }
    }
}

impl Project {
    pub fn current_pattern(&self) -> &Pattern {
        let index = self
            .current_pattern
            .min(self.patterns.len().saturating_sub(1));
        &self.patterns[index]
    }

    pub fn current_pattern_mut(&mut self) -> Option<&mut Pattern> {
        let index = self.current_pattern;
        self.patterns.get_mut(index)
    }

    pub fn select_pattern(&mut self, index: usize) {
        if !self.patterns.is_empty() {
            self.current_pattern = index.min(self.patterns.len() - 1);
        }
    }

    pub fn add_pattern(&mut self) -> usize {
        if self.patterns.len() >= MAX_PATTERNS {
            return self.current_pattern;
        }
        let mut pattern = Pattern::named(format!("Pattern {}", self.patterns.len() + 1), 16);
        if let Some(current) = self.patterns.get(self.current_pattern) {
            pattern.copy_from(current);
            pattern.name = format!("Pattern {}", self.patterns.len() + 1);
        }
        self.patterns.push(pattern);
        self.current_pattern = self.patterns.len() - 1;
        self.current_pattern
    }

    pub fn duplicate_pattern(&mut self) -> usize {
        if self.patterns.len() >= MAX_PATTERNS {
            return self.current_pattern;
        }
        let Some(source) = self.patterns.get(self.current_pattern).cloned() else {
            return 0;
        };
        let mut copy = source;
        copy.name = format!("Pattern {}", self.patterns.len() + 1);
        self.patterns.push(copy);
        self.current_pattern = self.patterns.len() - 1;
        self.current_pattern
    }

    pub fn clear_pattern(&mut self) {
        if let Some(pattern) = self.patterns.get_mut(self.current_pattern) {
            pattern.clear();
        }
    }

    pub fn set_length(&mut self, length: u8) {
        if let Some(pattern) = self.patterns.get_mut(self.current_pattern) {
            pattern.length = length.clamp(1, MAX_STEPS as u8);
        }
    }

    pub fn set_midi_note(&mut self, instrument: usize, note: u8) {
        if instrument < NUM_INSTRUMENTS {
            self.midi_map[instrument] = note.min(127);
        }
    }

    pub fn instrument_for_note(&self, note: u8) -> Option<usize> {
        self.midi_map
            .iter()
            .position(|candidate| *candidate == note)
    }

    pub fn add_song_slot(&mut self, pattern: u8) {
        if self.song.len() < MAX_SONG_SLOTS {
            self.song.push(SongSlot {
                pattern,
                repeats: 1,
            });
        }
    }

    pub fn remove_song_slot(&mut self, index: usize) {
        if index < self.song.len() {
            self.song.remove(index);
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!("unsupported project schema {}", self.schema));
        }
        if self.patterns.is_empty() {
            return Err("project contains no patterns".into());
        }
        if self.current_pattern >= self.patterns.len() {
            return Err("current pattern index is out of range".into());
        }
        for (index, pattern) in self.patterns.iter().enumerate() {
            if !matches!(pattern.length, 1..=64) {
                return Err(format!(
                    "pattern {index} has invalid length {}",
                    pattern.length
                ));
            }
        }
        for slot in &self.song {
            if slot.pattern as usize >= self.patterns.len() {
                return Err("song slot references a missing pattern".into());
            }
            if slot.repeats == 0 {
                return Err("song slot repeats must be at least 1".into());
            }
        }
        if !(20.0..=300.0).contains(&self.bpm) || !self.bpm.is_finite() {
            return Err("BPM must be between 20 and 300".into());
        }
        if !(0.0..=1.0).contains(&self.swing) || !self.swing.is_finite() {
            return Err("swing must be between 0 and 1".into());
        }
        Ok(())
    }

    pub fn to_json_pretty(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|error| error.to_string())
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let project: Self = serde_json::from_str(text).map_err(|error| error.to_string())?;
        project.validate()?;
        Ok(project)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let text = self.to_json_pretty()?;
        fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text =
            fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        Self::from_json(&text)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preset {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub kit: String,
    pub instruments: std::collections::BTreeMap<String, InstrumentSettings>,
    pub master: MasterSettings,
    pub reverb: ReverbSettings,
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            schema: SCHEMA,
            id: "init".into(),
            name: "Init".into(),
            description: String::new(),
            kit: "mercy-80".into(),
            instruments: Default::default(),
            master: MasterSettings::default(),
            reverb: ReverbSettings::default(),
        }
    }
}

impl Preset {
    pub fn parse(text: &str) -> Result<Self, String> {
        let preset: Self = serde_json::from_str(text).map_err(|error| error.to_string())?;
        if preset.schema != SCHEMA {
            return Err(format!("unsupported preset schema {}", preset.schema));
        }
        Ok(preset)
    }

    pub fn from_project(project: &Project, id: impl Into<String>, name: impl Into<String>) -> Self {
        let mut instruments = std::collections::BTreeMap::new();
        for (index, instrument_id) in INSTRUMENT_IDS.iter().enumerate() {
            instruments.insert((*instrument_id).to_string(), project.instruments[index]);
        }
        Self {
            schema: SCHEMA,
            id: id.into(),
            name: name.into(),
            description: String::new(),
            kit: project.kit.clone(),
            instruments,
            master: project.master,
            reverb: project.reverb,
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text =
            fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        Self::parse(&text)
    }

    pub fn apply(&self, project: &mut Project) {
        if !self.kit.is_empty() {
            project.kit = self.kit.clone();
        }
        for (index, id) in INSTRUMENT_IDS.iter().enumerate() {
            if let Some(settings) = self.instruments.get(*id) {
                project.instruments[index] = *settings;
            }
        }
        project.master = self.master;
        project.reverb = self.reverb;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_toggle_and_velocity() {
        let mut pattern = Pattern::named("Test", 16);
        assert_eq!(pattern.velocity(0, 0), 0);
        pattern.toggle(0, 0);
        assert_eq!(pattern.velocity(0, 0), 100);
        pattern.set_velocity(0, 0, 64);
        assert_eq!(pattern.velocity(0, 0), 64);
        pattern.clear();
        assert_eq!(pattern.velocity(0, 0), 0);
    }

    #[test]
    fn project_roundtrip() {
        let mut project = Project {
            bpm: 132.0,
            ..Project::default()
        };
        project.patterns[0].toggle(0, 4);
        project.song.push(SongSlot {
            pattern: 0,
            repeats: 2,
        });
        let text = project.to_json_pretty().unwrap();
        let loaded = Project::from_json(&text).unwrap();
        assert_eq!(loaded.bpm, 132.0);
        assert_eq!(loaded.patterns[0].velocity(0, 4), 100);
        assert_eq!(loaded.song.len(), 1);
    }

    #[test]
    fn preset_roundtrip_preserves_mix() {
        let mut project = Project::default();
        project.instruments[0].volume_db = -4.5;
        project.instruments[0].body = 0.42;
        project.reverb.mix = 0.33;
        project.master.limiter = false;
        let preset = Preset::from_project(&project, "user-test", "User Test");
        let text = serde_json::to_string_pretty(&preset).unwrap();
        let loaded = Preset::parse(&text).unwrap();
        let mut target = Project::default();
        loaded.apply(&mut target);
        assert_eq!(target.instruments[0].volume_db, -4.5);
        assert_eq!(target.instruments[0].body, 0.42);
        assert_eq!(target.reverb.mix, 0.33);
        assert!(!target.master.limiter);
    }

    #[test]
    fn validation_rejects_bad_song_reference() {
        let mut project = Project::default();
        project.song.push(SongSlot {
            pattern: 99,
            repeats: 1,
        });
        assert!(project.validate().is_err());
    }
}

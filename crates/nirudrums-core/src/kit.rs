use crate::factory_assets::{factory_sample, FACTORY_KIT_JSON, FACTORY_PRESETS};
use crate::project::{InstrumentSettings, Preset, NUM_INSTRUMENTS, SCHEMA};
use serde::Deserialize;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug)]
pub struct SampleData {
    pub frames: Vec<f32>,
    pub sample_rate: u32,
}

#[derive(Clone, Debug)]
pub struct VelocityLayer {
    pub velocity_min: u8,
    pub velocity_max: u8,
    pub sample: Arc<SampleData>,
}

#[derive(Clone, Debug)]
pub struct Instrument {
    pub id: String,
    pub name: String,
    pub midi_note: u8,
    pub choke_group: Option<String>,
    pub layers: Vec<VelocityLayer>,
    pub defaults: InstrumentSettings,
}

#[derive(Clone, Debug)]
pub struct Kit {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub sample_rate: u32,
    pub instruments: Vec<Instrument>,
    pub choke_groups: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct KitFile {
    schema: u32,
    id: String,
    name: String,
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_rate")]
    sample_rate: u32,
    #[serde(default)]
    instruments: Vec<InstrumentFile>,
    #[serde(default)]
    choke_groups: Vec<ChokeGroupFile>,
}

#[derive(Debug, Deserialize)]
struct ChokeGroupFile {
    id: String,
}

#[derive(Debug, Deserialize)]
struct InstrumentFile {
    id: String,
    name: String,
    #[serde(default)]
    midi_note: u8,
    #[serde(default)]
    choke_group: Option<String>,
    #[serde(default)]
    samples: Vec<SampleFile>,
    #[serde(default)]
    defaults: InstrumentSettings,
}

#[derive(Debug, Deserialize)]
struct SampleFile {
    file: String,
    #[serde(default)]
    velocity_min: u8,
    #[serde(default = "max_velocity")]
    velocity_max: u8,
}

fn default_rate() -> u32 {
    48_000
}

fn max_velocity() -> u8 {
    127
}

impl Kit {
    pub fn factory() -> Result<Self, String> {
        let file: KitFile = serde_json::from_str(FACTORY_KIT_JSON)
            .map_err(|error| format!("factory kit JSON: {error}"))?;
        Self::from_file(file, |path| {
            factory_sample(path)
                .map(|bytes| bytes.to_vec())
                .ok_or_else(|| format!("missing embedded sample {path}"))
        })
    }

    pub fn load_from_dir(dir: &Path) -> Result<Self, String> {
        let kit_path = dir.join("kit.json");
        let text = fs::read_to_string(&kit_path)
            .map_err(|error| format!("{}: {error}", kit_path.display()))?;
        let file: KitFile = serde_json::from_str(&text)
            .map_err(|error| format!("{}: {error}", kit_path.display()))?;
        let base = dir.to_path_buf();
        Self::from_file(file, move |relative| {
            let path = base.join(relative);
            fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))
        })
    }

    fn from_file(
        file: KitFile,
        mut read: impl FnMut(&str) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        if file.schema != SCHEMA {
            return Err(format!("unsupported kit schema {}", file.schema));
        }
        if file.instruments.is_empty() {
            return Err("kit contains no instruments".into());
        }
        if file.instruments.len() != NUM_INSTRUMENTS {
            return Err(format!(
                "kit must contain exactly {NUM_INSTRUMENTS} instruments, found {}",
                file.instruments.len()
            ));
        }
        let mut instruments = Vec::with_capacity(file.instruments.len());
        let mut seen = std::collections::HashSet::new();
        for instrument in file.instruments {
            if !seen.insert(instrument.id.clone()) {
                return Err(format!("duplicate instrument id {}", instrument.id));
            }
            if instrument.samples.is_empty() {
                return Err(format!("instrument {} has no samples", instrument.id));
            }
            let mut layers = Vec::with_capacity(instrument.samples.len());
            for sample in instrument.samples {
                if sample.velocity_min > sample.velocity_max {
                    return Err(format!(
                        "instrument {} has an inverted velocity range",
                        instrument.id
                    ));
                }
                let bytes = read(&sample.file)?;
                let data =
                    decode_wav(&bytes).map_err(|error| format!("{}: {error}", sample.file))?;
                if data.frames.is_empty() {
                    return Err(format!("{} contains no audio frames", sample.file));
                }
                layers.push(VelocityLayer {
                    velocity_min: sample.velocity_min,
                    velocity_max: sample.velocity_max,
                    sample: Arc::new(data),
                });
            }
            layers.sort_by_key(|layer| layer.velocity_min);
            instruments.push(Instrument {
                id: instrument.id,
                name: instrument.name,
                midi_note: instrument.midi_note,
                choke_group: instrument.choke_group,
                layers,
                defaults: instrument.defaults,
            });
        }
        let mut choke_groups: Vec<String> = file
            .choke_groups
            .into_iter()
            .map(|group| group.id)
            .collect();
        for instrument in &instruments {
            if let Some(group) = &instrument.choke_group {
                if !choke_groups.contains(group) {
                    choke_groups.push(group.clone());
                }
            }
        }
        Ok(Self {
            schema: file.schema,
            id: file.id,
            name: file.name,
            version: file.version,
            description: file.description,
            sample_rate: file.sample_rate.max(1),
            instruments,
            choke_groups,
        })
    }

    pub fn instrument_index(&self, id: &str) -> Option<usize> {
        self.instruments
            .iter()
            .position(|instrument| instrument.id == id)
    }

    pub fn instrument_for_note(&self, note: u8) -> Option<usize> {
        self.instruments
            .iter()
            .position(|instrument| instrument.midi_note == note)
    }

    pub fn defaults(&self, index: usize) -> InstrumentSettings {
        self.instruments
            .get(index)
            .map(|instrument| instrument.defaults)
            .unwrap_or_default()
    }

    pub fn default_project(&self) -> crate::project::Project {
        use crate::project::Pattern;

        let mut project = crate::project::Project {
            kit: self.id.clone(),
            ..crate::project::Project::default()
        };
        for index in 0..NUM_INSTRUMENTS.min(self.instruments.len()) {
            project.instruments[index] = self.instruments[index].defaults;
            project.midi_map[index] = self.instruments[index].midi_note;
        }

        let mut basic = Pattern::named("MERCY-80 BASIC", 16);
        for step in [0, 4, 8, 12] {
            basic.set_velocity(0, step, 116);
        }
        for step in [4, 12] {
            basic.set_velocity(1, step, 118);
        }
        for step in (2..16).step_by(2) {
            basic.set_velocity(2, step, if step % 4 == 0 { 72 } else { 88 });
        }
        for step in [6, 14] {
            basic.set_velocity(3, step, 82);
        }
        basic.set_velocity(10, 4, 64);
        basic.set_velocity(10, 12, 64);

        let mut fill = Pattern::named("MERCY-80 FILL", 16);
        for step in [0, 8] {
            fill.set_velocity(0, step, 116);
        }
        for step in [4, 12] {
            fill.set_velocity(1, step, 118);
        }
        for step in (2..16).step_by(2) {
            fill.set_velocity(2, step, 76);
        }
        fill.set_velocity(4, 0, 90);
        fill.set_velocity(5, 8, 86);
        for (step, instrument, velocity) in [(13, 7, 96), (14, 8, 104), (15, 9, 112)] {
            fill.set_velocity(instrument, step, velocity);
        }
        fill.set_velocity(7, 0, 90);

        project.patterns[0] = basic;
        project.patterns[1] = fill;
        project.current_pattern = 0;
        project.master.volume_db = -1.0;
        project.master.drive = 0.18;
        project.reverb = crate::project::ReverbSettings {
            mode: crate::project::ReverbMode::Gated,
            mix: 0.30,
            decay: 0.52,
            pre_delay_ms: 18.0,
            damping: 0.30,
            width: 1.0,
            gate_ms: 230.0,
        };
        project
    }
}

pub fn decode_wav(bytes: &[u8]) -> Result<SampleData, String> {
    let mut reader = hound::WavReader::new(Cursor::new(bytes))
        .map_err(|error| format!("cannot read WAV: {error}"))?;
    let spec = reader.spec();
    if spec.channels == 0 {
        return Err("WAV has no channels".into());
    }
    let channels = usize::from(spec.channels);
    let mut mono = Vec::new();
    match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Int, bits) if bits <= 16 => {
            let scale = 1.0 / f32::from(1_i16 << (bits.saturating_sub(1)).min(15));
            let mut frame = 0.0;
            let mut channel = 0;
            for sample in reader.samples::<i16>() {
                let sample = sample.map_err(|error| error.to_string())?;
                frame += f32::from(sample) * scale;
                channel += 1;
                if channel == channels {
                    mono.push(frame / channels as f32);
                    frame = 0.0;
                    channel = 0;
                }
            }
        }
        (hound::SampleFormat::Int, bits) => {
            let scale = 1.0 / 2_147_483_648.0;
            let mut frame = 0.0;
            let mut channel = 0;
            for sample in reader.samples::<i32>() {
                let sample = sample.map_err(|error| error.to_string())?;
                frame += sample as f32 * scale;
                channel += 1;
                if channel == channels {
                    mono.push(frame / channels as f32);
                    frame = 0.0;
                    channel = 0;
                }
            }
            let _ = bits;
        }
        (hound::SampleFormat::Float, _) => {
            let mut frame = 0.0;
            let mut channel = 0;
            for sample in reader.samples::<f32>() {
                let sample = sample.map_err(|error| error.to_string())?;
                frame += sample;
                channel += 1;
                if channel == channels {
                    mono.push(frame / channels as f32);
                    frame = 0.0;
                    channel = 0;
                }
            }
        }
    }
    if mono.is_empty() {
        return Err("WAV has no samples".into());
    }
    Ok(SampleData {
        frames: mono,
        sample_rate: spec.sample_rate.max(1),
    })
}

pub fn data_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("NIRUDRUMS_DATA_DIR") {
        return PathBuf::from(path);
    }
    if let Some(path) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(path).join("nirudrums");
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local/share/nirudrums")
}

pub fn kit_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(path) = std::env::var_os("NIRUDRUMS_KIT_DIR") {
        dirs.push(PathBuf::from(path));
    }
    dirs.push(data_dir().join("kits"));
    dirs
}

pub fn available_kits() -> Vec<(String, String, PathBuf)> {
    let mut found = Vec::new();
    for root in kit_dirs() {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let kit_json = path.join("kit.json");
            if !kit_json.is_file() {
                continue;
            }
            let Ok(text) = fs::read_to_string(&kit_json) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let id = value.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            let name = value.get("name").and_then(|v| v.as_str()).unwrap_or(id);
            if !id.is_empty() {
                found.push((id.to_string(), name.to_string(), path));
            }
        }
    }
    found
}

pub fn factory_presets() -> Vec<Preset> {
    FACTORY_PRESETS
        .iter()
        .filter_map(|(_, text)| Preset::parse(text).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_kit_loads_all_instruments_and_layers() {
        let kit = Kit::factory().unwrap();
        assert_eq!(kit.id, "mercy-80");
        assert_eq!(kit.instruments.len(), NUM_INSTRUMENTS);
        for instrument in &kit.instruments {
            assert_eq!(instrument.layers.len(), 3, "{}", instrument.id);
            for layer in &instrument.layers {
                assert!(layer.sample.sample_rate > 0);
                assert!(!layer.sample.frames.is_empty());
            }
        }
        let kick = &kit.instruments[kit.instrument_index("kick").unwrap()];
        assert!(kick.layers[0].sample.frames.len() > 3_000);
        assert!(kick.layers[0].sample.frames.len() < 10_000);
        assert!(kick.layers[0]
            .sample
            .frames
            .iter()
            .any(|sample| sample.abs() > 0.5));
    }

    #[test]
    fn factory_presets_parse() {
        let presets = factory_presets();
        assert_eq!(presets.len(), 7);
        assert!(presets.iter().any(|preset| preset.id == "mercy-80-dry"));
        assert!(presets
            .iter()
            .any(|preset| preset.id == "mercy-80-cathedral"));
    }

    #[test]
    fn external_kit_directory_loads() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../kits/MERCY-80");
        let kit = Kit::load_from_dir(&root).unwrap();
        assert_eq!(kit.id, "mercy-80");
        assert_eq!(kit.instruments.len(), NUM_INSTRUMENTS);
    }

    #[test]
    fn factory_project_has_a_playable_demo() {
        let kit = Kit::factory().unwrap();
        let project = kit.default_project();
        assert_eq!(project.current_pattern, 0);
        assert!(project.patterns[0].velocity(0, 0) > 0);
        assert!(project.patterns[0].velocity(1, 4) > 0);
        assert!(project.patterns[0].velocity(2, 2) > 0);
        assert_eq!(project.patterns[0].length, 16);
        assert!(project.master.volume_db < 0.0);
    }

    #[test]
    fn note_lookup_uses_kit_mapping() {
        let kit = Kit::factory().unwrap();
        assert_eq!(kit.instrument_for_note(36), Some(0));
        assert_eq!(kit.instrument_for_note(42), Some(2));
        assert_eq!(kit.instrument_for_note(99), None);
    }
}

use nih_plug::prelude::*;
use nirudrums::Nirudrums;
use nirudrums_core::{Engine, EngineSettings, Kit};
use std::sync::Arc;

fn main() {
    if std::env::args().any(|argument| argument == "--version") {
        println!("NIRUDRUMS {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if std::env::args().any(|argument| argument == "--selftest") {
        match selftest() {
            Ok(peak) => {
                println!(
                    "NIRUDRUMS selftest OK: MERCY-80 loaded, 48 kHz stereo render peak {peak:.3}"
                );
            }
            Err(error) => {
                eprintln!("NIRUDRUMS selftest FAILED: {error}");
                std::process::exit(1);
            }
        }
        return;
    }

    nih_export_standalone::<Nirudrums>();
}

fn selftest() -> Result<f32, String> {
    let kit = Arc::new(Kit::factory()?);
    if kit.instruments.len() != nirudrums_core::NUM_INSTRUMENTS {
        return Err("factory kit does not contain 12 instruments".into());
    }
    let project = kit.default_project();
    let mut settings = EngineSettings::from_project(&project);
    settings.playing = true;
    settings.sequencer_mode = nirudrums_core::SequencerMode::Internal;
    let path = std::env::temp_dir().join(format!("nirudrums-selftest-{}.wav", std::process::id()));
    Engine::render_to_wav(&path, kit, &project, &settings, 1)?;
    let mut reader =
        hound::WavReader::open(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let spec = reader.spec();
    if spec.sample_rate != 48_000 || spec.channels != 2 {
        let _ = std::fs::remove_file(&path);
        return Err(format!(
            "unexpected WAV format: {} Hz, {} channels",
            spec.sample_rate, spec.channels
        ));
    }
    let peak = reader
        .samples::<i16>()
        .map(|sample| sample.map(|value| i32::from(value).abs()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?
        .into_iter()
        .max()
        .unwrap_or(0);
    let _ = std::fs::remove_file(&path);
    if peak < 100 {
        return Err("rendered audio is silent".into());
    }
    Ok(peak as f32 / 32768.0)
}

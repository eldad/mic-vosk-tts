use console::Term;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::StreamConfig;
use cpal::{Device, Sample};
use std::sync::mpsc;
use vosk::{Model, Recognizer};

fn mono_input_config(device: &Device) -> anyhow::Result<StreamConfig> {
    let supported_configs_range = device
        .supported_output_configs()
        .expect("error while querying configs");

    for s in supported_configs_range {
        if s.channels() == 1 {
            let stream_config: StreamConfig = s.with_max_sample_rate().into();
            return Ok(stream_config);
        }
    }

    Err(anyhow::anyhow!("no suitable config found"))
}

fn main() -> anyhow::Result<()> {
    // Audio input
    let host = cpal::host_from_id(
        cpal::available_hosts()
            .into_iter()
            .find(|id| *id == cpal::HostId::Jack)
            .expect("Requires jack (Linux only)"),
    )
    .expect("jack host unavailable");

    let device = host
        .default_input_device()
        .expect("No input device available");

    let config = mono_input_config(&device)?;

    let sample_rate = config.sample_rate.0 as f32;

    let (tx, rx) = mpsc::channel::<Vec<i16>>();

    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _| {
            let samples: Vec<i16> = data.iter().map(|s| s.to_sample()).collect();
            let _ = tx.send(samples);
        },
        move |err| {
            eprintln!("Stream error: {}", err);
        },
        None,
    )?;

    // Load Vosk before starting stream to avoid underruns
    let model = Model::new("model/vosk-model-en-us-0.42-gigaspeech").unwrap();
    let mut recognizer = Recognizer::new(&model, sample_rate).unwrap();

    eprintln!("stream play");
    stream.play()?;

    let term = Term::stdout();
    eprintln!("Speak into your mic. Press Ctrl+C to stop.");

    loop {
        let audio = rx.recv()?;

        let res = recognizer.accept_waveform(&audio)?;
        term.clear_line()?;

        match res {
            vosk::DecodingState::Finalized => {
                let text = match recognizer.result() {
                    vosk::CompleteResult::Single(complete_result_single) => {
                        complete_result_single.text
                    }
                    vosk::CompleteResult::Multiple(complete_result_multiple) => {
                        complete_result_multiple.alternatives.first().unwrap().text
                    }
                };
                if !text.is_empty() {
                    eprintln!("{}", text);
                }
            }
            vosk::DecodingState::Running => {
                let partial_result = recognizer.partial_result();
                eprint!("{}", partial_result.partial);
            }
            vosk::DecodingState::Failed => (),
        }
    }
}


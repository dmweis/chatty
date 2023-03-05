use anyhow::{Context, Result};
use async_openai::{types::CreateTranscriptionRequestArgs, Client};
use chatty::configuration::get_configuration;
use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample};
use std::fs::File;
use std::io::{BufRead, BufWriter};
use std::sync::{Arc, Mutex};
use tempdir::TempDir;
// heavily inspired by cpal record_wav example
// https://github.com/RustAudio/cpal/blob/master/examples/record_wav.rs

#[derive(Parser, Debug)]
#[command()]
struct Cli {
    /// The audio device to use
    #[arg(short, long, default_value_t = String::from("default"))]
    device: String,

    /// Use the JACK host
    jack: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Conditionally compile with jack if the feature is specified.
    #[cfg(all(
        any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd"
        ),
        feature = "jack"
    ))]
    // check if we should use jack
    let host = if cli.jack {
        cpal::host_from_id(cpal::available_hosts()
            .into_iter()
            .find(|id| *id == cpal::HostId::Jack)
            .context(
                "make sure --features jack is specified. only works on OSes where jack is available",
            )?).context("jack host unavailable")?
    } else {
        cpal::default_host()
    };

    #[cfg(any(
        not(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd"
        )),
        not(feature = "jack")
    ))]
    let host = cpal::default_host();

    // Set up the input device and stream with the default input config.
    let device = if cli.device == "default" {
        host.default_input_device()
    } else {
        host.input_devices()?
            .find(|x| x.name().map(|y| y == cli.device).unwrap_or(false))
    }
    .context("failed to find input device")?;

    println!("Input device: {}", device.name()?);

    let config = device
        .default_input_config()
        .context("Failed to get default input config")?;
    println!("Default input config: {:?}", config);

    // The WAV file we're recording to.

    // openai async lib can only send audio as files
    // TODO(David): make a PR into it since it can just take a reqwest body
    let temp_dir = TempDir::new("chatty_audio_tmp_dir")?;
    let audio_path = temp_dir.path().join("recorded.wav");
    let spec = wav_spec_from_config(&config);
    let writer = hound::WavWriter::create(&audio_path, spec)?;
    let writer = Arc::new(Mutex::new(Some(writer)));

    // A flag to indicate that recording is in progress.
    println!("Begin recording...");

    // Run the input stream on a separate thread.
    let writer_2 = writer.clone();

    let err_fn = move |err| {
        eprintln!("an error occurred on stream: {}", err);
    };

    let stream = match config.sample_format() {
        cpal::SampleFormat::I8 => device.build_input_stream(
            &config.into(),
            move |data, _: &_| write_input_data::<i8, i8>(data, &writer_2),
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data, _: &_| write_input_data::<i16, i16>(data, &writer_2),
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I32 => device.build_input_stream(
            &config.into(),
            move |data, _: &_| write_input_data::<i32, i32>(data, &writer_2),
            err_fn,
            None,
        )?,
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data, _: &_| write_input_data::<f32, f32>(data, &writer_2),
            err_fn,
            None,
        )?,
        sample_format => {
            return Err(anyhow::Error::msg(format!(
                "Unsupported sample format '{sample_format}'"
            )))
        }
    };

    stream.play()?;

    println!("Press enter to stop recording");
    std::io::stdin().lock().read_line(&mut String::new())?;

    drop(stream);
    writer.lock().unwrap().take().unwrap().finalize()?;
    println!("Recording stopped. Transcribing");

    transcribe_audio(&audio_path).await?;
    Ok(())
}

fn sample_format(format: cpal::SampleFormat) -> hound::SampleFormat {
    if format.is_float() {
        hound::SampleFormat::Float
    } else {
        hound::SampleFormat::Int
    }
}

fn wav_spec_from_config(config: &cpal::SupportedStreamConfig) -> hound::WavSpec {
    hound::WavSpec {
        channels: config.channels() as _,
        sample_rate: config.sample_rate().0 as _,
        bits_per_sample: (config.sample_format().sample_size() * 8) as _,
        sample_format: sample_format(config.sample_format()),
    }
}

type WavWriterHandle = Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>;

fn write_input_data<T, U>(input: &[T], writer: &WavWriterHandle)
where
    T: Sample,
    U: Sample + hound::Sample + FromSample<T>,
{
    if let Ok(mut guard) = writer.try_lock() {
        if let Some(writer) = guard.as_mut() {
            for &sample in input.iter() {
                let sample: U = U::from_sample(sample);
                writer.write_sample(sample).ok();
            }
        }
    }
}

async fn transcribe_audio(path: &std::path::PathBuf) -> Result<()> {
    let config = get_configuration()?;

    let client = Client::new().with_api_key(&config.open_ai_api_key);

    let request = CreateTranscriptionRequestArgs::default()
        .file(path)
        .model("whisper-1")
        .build()?;

    let response = client.audio().transcribe(request).await?;

    println!("{}", response.text);
    Ok(())
}

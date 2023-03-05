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
    #[arg(short, long)]
    device: Option<String>,

    /// Use the JACK host
    #[arg(short, long)]
    jack: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let audio = chatty::audio::record_audio_with_cli_to_memory(cli.jack, cli.device)?;

    // transcribe_audio(&audio_path).await?;
    Ok(())
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

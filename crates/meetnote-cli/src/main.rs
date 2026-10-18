//! meetnote: record system audio + mic (ScreenCaptureKit), transcribe locally
//! (whisper.cpp), summarize locally (Ollama). Works for any meeting app.
//!
//!   meetnote record                       record until Enter, then transcribe + summarize
//!   meetnote run --file call.m4a          transcribe + summarize an existing recording
//!   meetnote run --file x.mic.wav --file x.system.wav   redo a recording, with Me/Them labels
//!   (`meetnote` alone still records, and `meetnote <files>` still works like `run`.)
//!
//! Env (or `./.env`, `~/.config/meetnote/.env`): WHISPER_MODEL (path to ggml model), OLLAMA_MODEL,
//!      WHISPER_VAD_MODEL (optional, silero ggml; strongly recommended for Me/Them order),
//!      FIX_MODEL (optional Ollama model that corrects the transcript before the summary),
//!      SUMMARY_CHUNK_CHARS (optional, default 24000: longer transcripts are summarized in parts).

mod record;

use meetnote_core::{config, correct, summarize, transcribe};

use std::error::Error;
use std::fs;
use std::path::Path;

use clap::{Parser, Subcommand};

/// Local meeting notes: record a call, transcribe it, summarize it. Nothing leaves the Mac.
#[derive(Parser)]
#[command(name = "meetnote", version, args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Audio or video files to process (same as `run --file`)
    files: Vec<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Record system audio and your mic until Enter, then write the notes (the default)
    Record,
    /// Transcribe and summarize existing recordings
    Run {
        /// An audio or video file; pass x.mic.wav and x.system.wav together for Me/Them labels
        #[arg(short, long = "file", required = true)]
        file: Vec<String>,
    },
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let inputs = match cli.command {
        Some(Command::Run { file }) => file,
        Some(Command::Record) => Vec::new(),
        None => cli.files,
    };
    if let Some(missing) = inputs.iter().find(|f| !Path::new(f).exists()) {
        return Err(format!("no such file: {missing}").into());
    }
    let base = match inputs.first() {
        Some(f) => {
            let stem = Path::new(f)
                .with_extension("")
                .to_string_lossy()
                .into_owned();
            stem.trim_end_matches(".mic")
                .trim_end_matches(".system")
                .to_string()
        }
        None => {
            fs::create_dir_all("recordings")?;
            format!(
                "recordings/meeting-{}",
                std::time::UNIX_EPOCH.elapsed()?.as_secs()
            )
        }
    };
    let sources = if inputs.is_empty() {
        record::record(&base)?
    } else {
        inputs
    };

    let transcript = transcribe::transcript(&sources)?;
    fs::write(format!("{base}.txt"), &transcript)?;
    // The raw transcript stays in `.txt`; the corrected one feeds the notes.
    let transcript = match config::var("FIX_MODEL").filter(|m| !m.is_empty()) {
        Some(llm) => {
            let fixed = correct::transcript(&llm, &transcript)?;
            fs::write(format!("{base}.fixed.txt"), &fixed)?;
            fixed
        }
        None => transcript,
    };

    summarize::notes(&base, &transcript)
}

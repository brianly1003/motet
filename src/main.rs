//! meetnote: record system audio + mic (ScreenCaptureKit), transcribe locally
//! (whisper.cpp), summarize locally (Ollama). Works for any meeting app.
//!
//!   meetnote               record until Enter, then transcribe + summarize
//!   meetnote <audio-file>  transcribe + summarize an existing recording
//!   meetnote x.mic.wav x.system.wav   redo a recording, with Me/Them labels
//!
//! Env: WHISPER_MODEL (path to ggml model), OLLAMA_MODEL,
//!      WHISPER_VAD_MODEL (optional, silero ggml; strongly recommended for Me/Them order).

mod capture;
mod summarize;
mod transcribe;

use std::error::Error;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let inputs: Vec<String> = std::env::args().skip(1).collect();
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
        capture::record(&base)?
    } else {
        inputs
    };

    let transcript = transcribe::transcript(&sources)?;
    fs::write(format!("{base}.txt"), &transcript)?;

    summarize::notes(&base, &transcript)
}

//! meetnote: record system audio + mic (ScreenCaptureKit), transcribe locally
//! (whisper.cpp), summarize locally (Ollama). Works for any meeting app.
//!
//!   meetnote               record until Enter, then transcribe + summarize
//!   meetnote <audio-file>  transcribe + summarize an existing recording
//!   meetnote x.mic.wav x.system.wav   redo a recording, with Me/Them labels
//!
//! Env (or `./.env`, `~/.config/meetnote/.env`): WHISPER_MODEL (path to ggml model), OLLAMA_MODEL,
//!      WHISPER_VAD_MODEL (optional, silero ggml; strongly recommended for Me/Them order),
//!      FIX_MODEL (optional Ollama model that corrects the transcript before the summary),
//!      SUMMARY_CHUNK_CHARS (optional, default 24000: longer transcripts are summarized in parts).

mod capture;
mod config;
mod correct;
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

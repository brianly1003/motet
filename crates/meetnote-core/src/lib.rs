//! Local meeting pipeline shared by the CLI and desktop app.

#[cfg(target_os = "macos")]
pub mod capture;
pub mod config;
pub mod correct;
mod decode;
pub mod events;
pub mod session;
pub mod summarize;
pub mod transcribe;
pub mod whisper;

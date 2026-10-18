//! Local meeting pipeline shared by the CLI and desktop app.

#[cfg(target_os = "macos")]
pub mod capture;
mod platform;
#[cfg(target_os = "windows")]
pub use platform::windows as capture;
pub mod config;
pub mod correct;
mod decode;
pub mod events;
pub mod session;
pub mod summarize;
pub mod transcribe;
pub mod whisper;

//! Platform capture backends and their shared packet-to-WAV rules.

#[cfg(any(target_os = "windows", test))]
mod audio;
#[cfg(target_os = "windows")]
pub mod windows;

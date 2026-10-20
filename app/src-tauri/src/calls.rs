//! Call detection for the desktop: a polling watcher thread and the saved Ask/Off choices.
//! Detection only asks; the controller never records until the user chooses Record.
use crate::controller::Message;
use motet_core::{events::Mode, watch};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tauri::Emitter;

/// SPEC 4.2 fallback poll. polling; add the kAudioDevicePropertyDeviceIsRunningSomewhere
/// listener if 2 s latency or CPU proves a problem.
const POLL: Duration = Duration::from_secs(2);
/// A call ends after its app has held no input this long (idle_stop_s default).
const END_AFTER_MS: u64 = 30_000;

/// only ask_on_call and app_modes; the full SPEC 8.4 schema lands with the Settings window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub ask_on_call: bool,
    pub app_modes: BTreeMap<String, Mode>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            ask_on_call: true,
            app_modes: BTreeMap::new(),
        }
    }
}
impl Settings {
    /// A missing, unreadable or damaged file gives the defaults.
    pub fn load(dir: &Path) -> Settings {
        std::fs::read_to_string(dir.join("settings.json"))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }
    /// Written whole to a temporary file, then renamed over the old one.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let fail = |e: &dyn std::fmt::Display| format!("Couldn't save settings: {e}");
        std::fs::create_dir_all(dir).map_err(|e| fail(&e))?;
        let text = serde_json::to_string_pretty(self).map_err(|e| fail(&e))?;
        let tmp = dir.join("settings.json.tmp");
        std::fs::write(&tmp, text).map_err(|e| fail(&e))?;
        std::fs::rename(&tmp, dir.join("settings.json")).map_err(|e| fail(&e))
    }
}

/// Polls the audio processes every 2 s and hands what changed to the session controller.
pub fn spawn(app: tauri::AppHandle, sender: mpsc::Sender<Message>) -> Result<(), String> {
    std::thread::Builder::new()
        .name("motet-calls".into())
        .spawn(move || {
            let rules = watch::rules();
            let mut watcher = watch::Watcher::new(std::process::id(), END_AFTER_MS);
            let origin = Instant::now();
            let mut reported = false;
            loop {
                match watch::snapshot() {
                    Ok(snapshot) => {
                        reported = false; // A later breakage is reported again.
                        let now = origin.elapsed().as_millis() as u64;
                        for event in watcher.update(&rules, &snapshot, now) {
                            if sender.send(Message::Watch(event)).is_err() {
                                return; // The controller has stopped.
                            }
                        }
                    }
                    // Said once: a broken detector should not repeat every two seconds.
                    Err(e) if !reported => {
                        reported = true;
                        let _ = app.emit(
                            "motet://notice",
                            format!("Motet can't check for calls right now: {e}"),
                        );
                    }
                    Err(_) => {}
                }
                std::thread::sleep(POLL);
            }
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_missing_file_gives_defaults() {
        let dir = std::env::temp_dir().join(format!("motet-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let settings = Settings::load(&dir);
        assert!(settings.ask_on_call);
        assert!(settings.app_modes.is_empty());
        let mut settings = settings;
        settings.app_modes.insert("zoom".into(), Mode::Off);
        settings.save(&dir).unwrap();
        let loaded = Settings::load(&dir);
        assert_eq!(loaded.app_modes.get("zoom"), Some(&Mode::Off));
        assert!(loaded.ask_on_call);
        assert!(!dir.join("settings.json.tmp").exists());
        // A damaged file is ignored, never a crash.
        std::fs::write(dir.join("settings.json"), "{not json").unwrap();
        assert!(Settings::load(&dir).app_modes.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

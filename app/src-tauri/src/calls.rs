//! Call detection for the desktop: a polling watcher thread and the saved Ask/Off choices.
//! Detection only asks; the controller never records until the user chooses Record.
use crate::controller::Message;
use motetto_core::watch;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};

/// SPEC 4.2 fallback poll. polling; add the kAudioDevicePropertyDeviceIsRunningSomewhere
/// listener if 2 s latency or CPU proves a problem.
const POLL: Duration = Duration::from_secs(2);
/// A call ends after its app has held no input this long (idle_stop_s default).
const END_AFTER_MS: u64 = 30_000;

pub use crate::settings::Settings;

/// Polls the audio processes every 2 s and hands what changed to the session controller.
pub fn spawn(app: tauri::AppHandle, sender: mpsc::Sender<Message>) -> Result<(), String> {
    std::thread::Builder::new()
        .name("motetto-calls".into())
        .spawn(move || {
            let mut watcher = watch::Watcher::new(std::process::id(), END_AFTER_MS);
            let origin = Instant::now();
            let mut reported = false;
            loop {
                match watch::snapshot() {
                    Ok(snapshot) => {
                        reported = false; // A later breakage is reported again.
                        let now = origin.elapsed().as_millis() as u64;
                        let settings = app
                            .state::<crate::settings::Store>()
                            .get()
                            .unwrap_or_default();
                        let rules = settings.rules();
                        watcher.set_end_after_ms(u64::from(settings.idle_stop_s) * 1000);
                        crate::settings::remember_apps(&app, &snapshot);
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
                            "motetto://notice",
                            format!("Motetto can't check for calls right now: {e}"),
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
    use motetto_core::events::Mode;

    #[test]
    fn settings_round_trip_and_missing_file_gives_defaults() {
        let dir = std::env::temp_dir().join(format!("motetto-settings-{}", std::process::id()));
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

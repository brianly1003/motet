//! Discover installed local models and measure actual speech processing time.
use serde::Serialize;
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize)]
pub struct Models {
    speech: Vec<String>,
    notes: Vec<String>,
    warning: Option<String>,
}
pub fn notes_models() -> Result<Vec<String>, String> {
    let output = motetto_core::tools::command("ollama")
        .env("OLLAMA_HOST", "http://127.0.0.1:11434")
        .arg("list")
        .output()
        .map_err(|e| format!("Start local Ollama and retry: {e}"))?;
    if !output.status.success() {
        return Err("Start local Ollama to list installed notes models.".into());
    }
    Ok(parse_models(&String::from_utf8_lossy(&output.stdout)))
}
fn parse_models(text: &str) -> Vec<String> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let mut cols = line.split_whitespace();
            let name = cols.next()?;
            let size = cols.nth(1)?;
            (size != "-"
                && !name.ends_with("cloud")
                && !name.contains("://")
                && !name.starts_with('-'))
            .then(|| name.to_owned())
        })
        .collect()
}
pub fn check_notes_model(model: &str) -> Result<(), String> {
    if !notes_models()?.iter().any(|name| name == model) {
        return Err("Choose an installed local Ollama model. No model was downloaded.".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn installed_models(app: tauri::AppHandle) -> Result<Models, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = app.state::<crate::settings::Store>().get()?;
        let mut speech = vec![];
        if let Some(home) = std::env::home_dir() {
            for dir in [
                home.join(".cache/whisper"),
                app.path()
                    .app_data_dir()
                    .map_err(|e| e.to_string())?
                    .join("models"),
            ] {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && path.extension().is_some_and(|e| e == "bin")
                            && !path
                                .file_name()
                                .is_some_and(|n| n.to_string_lossy().contains("silero"))
                        {
                            speech.push(path.to_string_lossy().into_owned());
                        }
                    }
                }
            }
        }
        if !settings.speech_model.is_empty() {
            speech.push(settings.speech_model);
        }
        speech.sort();
        speech.dedup();
        let (notes, warning) = match notes_models() {
            Ok(models) => (models, None),
            Err(e) => (vec![], Some(e)),
        };
        Ok(Models {
            speech,
            notes,
            warning,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn pick_speech_model(app: tauri::AppHandle) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("Whisper GGML model", &["bin"])
            .blocking_pick_file()
            .map(|file| {
                file.into_path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .map_err(|e| e.to_string())
            })
            .transpose()
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn run_speed_test(app: tauri::AppHandle) -> Result<f64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let library = app.state::<crate::library_commands::Library>();
        if library.busy.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return Err("Wait until the current recording or import finishes.".into());
        }
        let result = speed_test();
        library
            .busy
            .store(false, std::sync::atomic::Ordering::SeqCst);
        let _ = app.emit("motetto://library-changed", ());
        result
    })
    .await
    .map_err(|e| e.to_string())?
}
fn speed_test() -> Result<f64, String> {
    #[cfg(target_os = "macos")]
    {
        let dir = std::env::temp_dir().join(format!(
            "motetto-speed-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos()
        ));
        std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
        let result = (|| {
            let sample = dir.join("sample.aiff");
            let status=std::process::Command::new("say").args(["-r","160","-o"]).arg(&sample).arg("This is a local speech speed test. We discussed the project timeline and agreed to review the next steps together at our meeting tomorrow.").status().map_err(|e|e.to_string())?;
            if !status.success() {
                return Err("Couldn't generate the local test sample.".into());
            }
            let wav = dir.join("sample.wav");
            motetto_core::decode_audio(&sample, &wav).map_err(|e| e.to_string())?;
            let mut reader = hound::WavReader::open(wav).map_err(|e| e.to_string())?;
            let audio = reader
                .samples::<f32>()
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            let seconds = audio.len() as f64 / 16000.;
            let stop = Default::default();
            let server = crate::speech::Server::start(&stop).map_err(|e| e.to_string())?;
            let started = std::time::Instant::now();
            server
                .client
                .transcribe(&audio, 0, Some("en"), None)
                .map_err(|e| e.to_string())?;
            Ok(started.elapsed().as_secs_f64() / seconds)
        })();
        let cleanup = std::fs::remove_dir_all(dir).map_err(|e| e.to_string());
        match result {
            Err(e) => Err(e),
            Ok(rtf) => {
                cleanup?;
                Ok(rtf)
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    Err("The speech speed test sample is not available on Windows yet.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_discovery_rejects_cloud_rows_and_option_names() {
        assert_eq!(
            parse_models(
                "NAME ID SIZE MODIFIED\nqwen3:8b abc 5.2 GB today\nremote:latest abc - today\nx-cloud abc 1 GB today\n--help abc 1 GB today\n"
            ),
            vec!["qwen3:8b"]
        );
    }
}

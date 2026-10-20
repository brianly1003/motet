//! Serial capture and notes worker; closing every WAV precedes Resume, processing and discard.
use crate::{
    controller::{Job, Message},
    library,
};
use motet_core::{
    config, correct,
    events::{Event, Stage},
    summarize, transcribe,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
use tauri::Emitter;
#[cfg(target_os = "macos")]
use tauri::Manager;

pub fn spawn(
    root: PathBuf,
    app: tauri::AppHandle,
    results: mpsc::Sender<Message>,
    allow_exit: Arc<AtomicBool>,
) -> Result<mpsc::Sender<Job>, String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("motet-recording".into())
        .spawn(move || {
            let mut recording: Option<Recording> = None;
            for job in receiver {
                match job {
                    Job::Capture {
                        id,
                        offset_ms,
                        stop,
                    } => {
                        if recording.as_ref().is_none_or(|r| r.id != id) {
                            recording = Some(Recording {
                                id: id.clone(),
                                parts: vec![],
                                failure: None,
                            });
                        }
                        let Some(record) = recording.as_mut() else {
                            continue;
                        };
                        let result = capture_part(&root, record, offset_ms, stop, |base, stop| {
                            // The meeting exists from its first second: list it before capture.
                            let _ = app.emit("motet://library-changed", ());
                            crate::live::start(
                                app.clone(),
                                id.clone(),
                                base.into(),
                                offset_ms,
                                stop.clone(),
                            )?;
                            #[cfg(target_os = "macos")]
                            let result = motet_core::capture::record_with_levels(
                                base,
                                stop.clone(),
                                Some(app.state::<crate::live::State>().levels.clone()),
                            )
                            .map_err(|e| e.to_string());
                            #[cfg(target_os = "windows")]
                            let result = motet_core::capture::record(base, stop.clone())
                                .map_err(|e| e.to_string());
                            let ended = stop.swap(true, Ordering::SeqCst);
                            crate::speech::shutdown();
                            if result.is_ok() && !ended {
                                return Err(
                                    "Audio capture ended unexpectedly. Partial audio is retained."
                                        .into(),
                                );
                            }
                            result
                        });
                        if let Err(mut reason) = result {
                            if let Err(e) = status(&root, &id, "NeedsAttention") {
                                reason.push_str(&format!(" Status could not be saved: {e}"));
                            }
                            record.failure = Some(reason.clone());
                            let _ = results.send(Message::CaptureFailed { id, reason });
                        }
                    }
                    Job::Finish { id } => {
                        let mut result = match recording.as_ref().filter(|r| r.id == id) {
                            Some(record) => finish(&root, record, |stage| {
                                let _ = app.emit(
                                    "motet://event",
                                    Event::Stage {
                                        meeting_id: id.clone(),
                                        stage,
                                        percent: 0,
                                        eta_s: None,
                                    },
                                );
                            }),
                            None => Err(
                                "No capture was saved. Check recording permissions and try again."
                                    .into(),
                            ),
                        };
                        if let Err(reason) = &mut result
                            && let Err(e) = status(&root, &id, "NeedsAttention")
                        {
                            reason.push_str(&format!(" Status could not be saved: {e}"));
                        }
                        let _ = app.emit("motet://library-changed", ());
                        let _ = results.send(Message::Finished { id, result });
                        recording = None;
                    }
                    Job::Discard { id } => {
                        // Never delete partial audio after a capture/write failure.
                        if recording
                            .as_ref()
                            .is_some_and(|r| r.id == id && r.failure.is_none())
                        {
                            let result = library::folder(&root, &id).and_then(|dir| {
                                let notes = dir.join("my-notes.md");
                                if notes.exists() && !library::text(&notes)?.trim().is_empty() {
                                    status(&root, &id, "NeedsAttention")?;
                                    let _ = app.emit(
                                        "motet://notice",
                                        "The short recording was kept with your notes.",
                                    );
                                    Ok(())
                                } else {
                                    fs::remove_dir_all(dir).map_err(|e| e.to_string())
                                }
                            });
                            if let Err(e) = result {
                                let _ = app.emit(
                                    "motet://notice",
                                    format!("Couldn't discard the short recording: {e}"),
                                );
                            }
                        }
                        recording = None;
                        let _ = app.emit("motet://library-changed", ());
                    }
                    Job::Exit => {
                        allow_exit.store(true, Ordering::SeqCst);
                        app.exit(0);
                        break;
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(sender)
}
struct Recording {
    id: String,
    parts: Vec<(Vec<String>, u64)>,
    failure: Option<String>,
}
fn capture_part(
    root: &Path,
    record: &mut Recording,
    offset_ms: u64,
    stop: motet_core::events::Stop,
    capture: impl FnOnce(&str, motet_core::events::Stop) -> Result<Vec<String>, String>,
) -> Result<(), String> {
    if let Some(e) = &record.failure {
        return Err(e.clone());
    }
    let dir = create(root, &record.id)?;
    if stop.load(Ordering::SeqCst) {
        return Ok(());
    }
    let base = dir.join(format!("part-{:04}", record.parts.len()));
    let files = capture(&base.to_string_lossy(), stop.clone())?;
    if !stop.load(Ordering::SeqCst) {
        return Err("Audio capture ended unexpectedly. Partial audio is retained.".into());
    }
    record.parts.push((files, offset_ms));
    fs::write(
        dir.join("parts.json"),
        serde_json::to_vec_pretty(&record.parts).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn create(root: &Path, id: &str) -> Result<PathBuf, String> {
    let dir = root.join(id);
    if !dir.exists() {
        fs::create_dir(&dir).map_err(|e| e.to_string())?;
        library::save(
            root,
            &library::MeetingSummary {
                id: id.into(),
                title: "Recorded meeting".into(),
                created_ms: id
                    .parse::<u128>()
                    .map_err(|e| e.to_string())?
                    .checked_div(1_000_000)
                    .unwrap_or(0) as u64,
                status: "Recording".into(),
                marks_ms: vec![],
                duration_ms: None,
                notes_ms: None,
                spellings: vec![],
            },
        )?;
    }
    library::folder(root, id)
}
fn status(root: &Path, id: &str, value: &str) -> Result<library::MeetingSummary, String> {
    let mut summary = library::summary(root, id)?;
    summary.status = value.into();
    library::save(root, &summary)?;
    Ok(summary)
}
fn finish(root: &Path, record: &Recording, stage: impl Fn(Stage)) -> Result<String, String> {
    if let Some(reason) = &record.failure {
        return Err(format!(
            "Capture failed: {reason}. Partial audio is retained."
        ));
    }
    if record.parts.is_empty() {
        return Err("No audio was captured. Check microphone and system audio permissions.".into());
    }
    status(root, &record.id, "Finishing")?;
    let dir = library::folder(root, &record.id)?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        stage(Stage::Transcribing);
        let transcript = transcribe::transcript_parts(&record.parts, true)?;
        fs::write(dir.join("transcript.txt"), &transcript)?;
        let text = match config::var("FIX_MODEL").filter(|v| !v.is_empty()) {
            Some(model) => {
                let fixed = correct::transcript(&model, &transcript)?;
                fs::write(dir.join("transcript.fixed.txt"), &fixed)?;
                fixed
            }
            None => transcript,
        };
        stage(Stage::Summarizing { part: 1, of: 1 });
        let started = std::time::Instant::now();
        let spellings = library::used_spellings();
        let my_notes = dir.join("my-notes.md");
        let mine = if my_notes.exists() {
            library::text(&my_notes)?
        } else {
            String::new()
        };
        summarize::notes(&dir.join("notes").to_string_lossy(), &text, &mine)?;
        let mut metadata = library::summary(root, &record.id)?;
        metadata.notes_ms = Some(started.elapsed().as_millis() as u64);
        metadata.spellings = spellings;
        library::save(root, &metadata)?;
        if !mine.trim().is_empty() {
            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(dir.join("notes.md"))?;
            write!(file, "\n### My notes\n\n{mine}\n")?;
        }
        stage(Stage::Saving);
        Ok(())
    })();
    result.map_err(|e| e.to_string())?;
    Ok(status(root, &record.id, "Ready")?.title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use motet_core::events::Stop;
    fn root() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "motet-worker-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        root
    }
    #[test]
    fn resumed_parts_preserve_prior_audio_and_manifest_offsets() {
        let root = root();
        let mut record = Recording {
            id: "123".into(),
            parts: vec![],
            failure: None,
        };
        for offset in [0, 25_000] {
            capture_part(&root, &mut record, offset, Stop::default(), |base, stop| {
                let file = format!("{base}.mic.wav");
                fs::write(&file, b"audio").unwrap();
                stop.store(true, Ordering::SeqCst);
                Ok(vec![file])
            })
            .unwrap();
        }
        assert_ne!(record.parts[0].0, record.parts[1].0);
        assert_eq!(fs::read(&record.parts[0].0[0]).unwrap(), b"audio");
        let saved: Vec<(Vec<String>, u64)> =
            serde_json::from_slice(&fs::read(root.join("123/parts.json")).unwrap()).unwrap();
        assert_eq!(saved, record.parts);
        assert_eq!(saved[1].1, 25_000);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn canceled_queued_resume_never_opens_capture() {
        let root = root();
        let mut record = Recording {
            id: "123".into(),
            parts: vec![],
            failure: None,
        };
        capture_part(
            &root,
            &mut record,
            0,
            Arc::new(AtomicBool::new(true)),
            |_, _| panic!("must not start"),
        )
        .unwrap();
        assert!(record.parts.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_capture_cannot_be_resumed_or_summarized() {
        let root = root();
        let mut record = Recording {
            id: "123".into(),
            parts: vec![],
            failure: Some("disk failed".into()),
        };
        assert_eq!(
            capture_part(&root, &mut record, 0, Stop::default(), |_, _| panic!(
                "must not start"
            )),
            Err("disk failed".into())
        );
        assert!(
            finish(&root, &record, |_| panic!("must not process"))
                .unwrap_err()
                .contains("Partial audio is retained")
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "requires synthetic audio and installed local models"]
    fn real_resumed_recording_writes_notes_with_offsets() {
        let source = PathBuf::from(std::env::var("MOTET_TEST_RECORDING").unwrap());
        let root = root();
        let dir = create(&root, "123").unwrap();
        let mut record = Recording {
            id: "123".into(),
            parts: vec![],
            failure: None,
        };
        for (part, offset) in [(0, 0), (1, 60_000)] {
            let file = dir.join(format!("part-{part:04}.mic.wav"));
            fs::copy(&source, &file).unwrap();
            record
                .parts
                .push((vec![file.to_string_lossy().into_owned()], offset));
        }
        finish(&root, &record, |_| {}).unwrap();
        let meeting = library::read(&root, "123").unwrap();
        assert_eq!(meeting.summary.status, "Ready");
        assert!(meeting.transcript.contains("[00:"));
        assert!(meeting.transcript.contains("[01:"));
        assert!(meeting.transcript.matches("Me:").count() >= 2);
        assert!(!meeting.notes_md.is_empty());
        assert!(Path::new(&record.parts[0].0[0]).exists());
        fs::remove_dir_all(root).unwrap();
    }
}

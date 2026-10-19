//! Local document actions. Deleted meetings remain recoverable; regeneration preserves notes.
use crate::{library, library_commands::Library};
use std::{fs, path::Path, sync::atomic::Ordering};
use tauri::{Emitter, Manager};

fn move_meeting(root: &Path, id: &str, restore: bool) -> Result<(), String> {
    // shortcut: retain deleted files until the planned 30-day retention scheduler is implemented.
    let deleted = root.join(".deleted");
    fs::create_dir_all(&deleted).map_err(|e| e.to_string())?;
    if deleted.canonicalize().map_err(|e| e.to_string())?.parent()
        != Some(root.canonicalize().map_err(|e| e.to_string())?.as_path())
    {
        return Err("Deleted folder is outside the library".into());
    }
    let (from, to) = if restore {
        (deleted.as_path(), root)
    } else {
        (root, deleted.as_path())
    };
    let source = library::folder(from, id)?;
    let destination = to.join(id);
    if destination.exists() {
        return Err("A meeting already exists at the destination".into());
    }
    fs::rename(source, destination).map_err(|e| e.to_string())
}

fn purge_meeting(root: &Path, id: &str) -> Result<(), String> {
    let deleted = root.join(".deleted");
    if fs::symlink_metadata(&deleted)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
        || deleted.canonicalize().map_err(|e| e.to_string())?.parent()
            != Some(root.canonicalize().map_err(|e| e.to_string())?.as_path())
    {
        return Err("Invalid deleted meetings folder".into());
    }
    let folder = library::folder(&deleted, id)?;
    if fs::symlink_metadata(deleted.join(id))
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Cannot permanently delete a linked meeting".into());
    }
    fs::remove_dir_all(folder).map_err(|e| e.to_string())
}

fn regenerate(root: &Path, id: &str) -> Result<(), String> {
    let folder = library::folder(root, id)?;
    let meeting = library::read(root, id)?;
    let transcript = if folder.join("transcript.fixed.txt").exists() {
        library::text(&folder.join("transcript.fixed.txt"))?
    } else {
        meeting.transcript.clone()
    };
    if meeting.transcript.trim().is_empty() {
        return Err("This meeting has no transcript to summarize".into());
    }
    let staging = folder.join(".regenerating");
    fs::create_dir(&staging).map_err(|e| format!("Cannot prepare new notes: {e}"))?;
    let result = (|| -> Result<(), String> {
        let started = std::time::Instant::now();
        let spellings = library::used_spellings();
        motet_core::summarize::notes(&staging.join("notes").to_string_lossy(), &transcript)
            .map_err(|e| e.to_string())?;
        if folder.join("my-notes.md").exists() {
            use std::io::Write;
            let notes = library::text(&folder.join("my-notes.md"))?;
            if !notes.trim().is_empty() {
                let mut output = fs::OpenOptions::new()
                    .append(true)
                    .open(staging.join("notes.md"))
                    .map_err(|e| e.to_string())?;
                write!(output, "\n\n## My notes\n\n{notes}").map_err(|e| e.to_string())?;
            }
        }
        if library::read(root, id)?.notes_md != meeting.notes_md {
            return Err(
                "Notes changed while regenerating. Your edits were kept; try again.".into(),
            );
        }
        fs::write(staging.join("previous.md"), meeting.notes_md).map_err(|e| e.to_string())?;
        fs::rename(staging.join("previous.md"), folder.join("notes.prev.md"))
            .map_err(|e| e.to_string())?;
        fs::rename(staging.join("notes.md"), folder.join("notes.md")).map_err(|e| e.to_string())?;
        let mut summary = library::summary(root, id)?;
        summary.notes_ms = Some(started.elapsed().as_millis() as u64);
        summary.spellings = spellings;
        library::save(root, &summary)
    })();
    let _ = fs::remove_dir_all(staging);
    result
}

#[tauri::command]
pub fn deleted_meetings(
    state: tauri::State<'_, Library>,
) -> Result<Vec<library::MeetingSummary>, String> {
    let deleted = state.root.join(".deleted");
    if !deleted.exists() {
        return Ok(vec![]);
    }
    if deleted.canonicalize().map_err(|e| e.to_string())?.parent()
        != Some(
            state
                .root
                .canonicalize()
                .map_err(|e| e.to_string())?
                .as_path(),
        )
    {
        return Err("Deleted folder is outside the library".into());
    }
    library::list(&deleted)
}

#[tauri::command]
pub async fn meeting_action(
    app: tauri::AppHandle,
    id: String,
    action: String,
) -> Result<(), String> {
    let state = app.state::<Library>();
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("Finish recording or processing before changing a meeting".into());
    }
    state.importing.store(true, Ordering::SeqCst);
    let root = state.root.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        match action.as_str() {
            "delete" => move_meeting(&root, &id, false),
            "restore" => move_meeting(&root, &id, true),
            "purge" => purge_meeting(&root, &id),
            "reveal" => {
                let folder = library::folder(&root, &id)?;
                #[cfg(target_os = "macos")]
                let status = std::process::Command::new("/usr/bin/open")
                    .arg("-R")
                    .arg(folder)
                    .status();
                #[cfg(target_os = "windows")]
                let status = std::process::Command::new("explorer.exe")
                    .arg(folder)
                    .status();
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                let status = std::process::Command::new("xdg-open").arg(folder).status();
                if status.map_err(|e| e.to_string())?.success() {
                    Ok(())
                } else {
                    Err("Could not open the meeting folder".into())
                }
            }
            "regenerate" => regenerate(&root, &id),
            _ => Err("Unknown meeting action".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    state.importing.store(false, Ordering::SeqCst);
    state.busy.store(false, Ordering::SeqCst);
    let _ = app.emit("motet://library-changed", ());
    if state.quit_pending.load(Ordering::SeqCst) {
        state.allow_exit.store(true, Ordering::SeqCst);
        app.exit(0);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn permanent_delete_only_removes_deleted_meetings() {
        let root = std::env::temp_dir().join(format!("motet-purge-{}", std::process::id()));
        fs::create_dir_all(root.join("123")).unwrap();
        fs::write(root.join("123/notes.md"), "Keep active meeting").unwrap();
        fs::create_dir_all(root.join(".deleted/456/audio")).unwrap();
        fs::write(root.join(".deleted/456/audio/mic.wav"), b"fixture").unwrap();
        assert!(purge_meeting(&root, "123").is_err());
        assert!(purge_meeting(&root, "../123").is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("123"), root.join(".deleted/789")).unwrap();
            assert!(purge_meeting(&root, "789").is_err());
            std::os::unix::fs::symlink(root.join("123"), root.join(".deleted/456/link")).unwrap();
        }
        purge_meeting(&root, "456").unwrap();
        assert!(!root.join(".deleted/456").exists());
        assert_eq!(
            fs::read_to_string(root.join("123/notes.md")).unwrap(),
            "Keep active meeting"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn regeneration_without_transcript_preserves_notes() {
        let root = std::env::temp_dir().join(format!("motet-no-transcript-{}", std::process::id()));
        fs::create_dir_all(root.join("123")).unwrap();
        fs::write(
            root.join("123/meeting.json"),
            r#"{"id":"123","title":"Test","created_ms":123,"status":"Ready"}"#,
        )
        .unwrap();
        fs::write(root.join("123/notes.md"), "Keep edited notes").unwrap();
        assert!(
            regenerate(&root, "123")
                .unwrap_err()
                .contains("no transcript")
        );
        assert_eq!(
            fs::read_to_string(root.join("123/notes.md")).unwrap(),
            "Keep edited notes"
        );
        assert!(!root.join("123/.regenerating").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn delete_restore_preserves_files_and_refuses_collisions_and_traversal() {
        let root = std::env::temp_dir().join(format!("motet-actions-{}", std::process::id()));
        fs::create_dir_all(root.join("123")).unwrap();
        fs::write(root.join("123/notes.md"), "Keep this").unwrap();
        move_meeting(&root, "123", false).unwrap();
        assert!(!root.join("123").exists());
        assert!(move_meeting(&root, "../123", true).is_err());
        fs::create_dir(root.join("123")).unwrap();
        assert!(move_meeting(&root, "123", true).is_err());
        fs::remove_dir(root.join("123")).unwrap();
        move_meeting(&root, "123", true).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("123/notes.md")).unwrap(),
            "Keep this"
        );
        fs::remove_dir_all(root).unwrap();
    }
}

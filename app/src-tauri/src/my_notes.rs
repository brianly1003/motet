//! User-authored notes remain separate from model output; stale saves merge concurrent jots.
use crate::{controller::Controller, library};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tauri::{Emitter, Manager};
use ts_rs::TS;

#[derive(Clone, Serialize, TS)]
pub struct MyNotes {
    pub meeting_id: String,
    pub text: String,
    #[ts(type = "number")]
    pub version: u64,
}
struct Revision {
    version: u64,
    text: String,
}
pub struct Store {
    root: PathBuf,
    history: Mutex<HashMap<String, Vec<Revision>>>,
}
impl Store {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            history: Mutex::default(),
        }
    }
}
fn version(text: &str) -> u64 {
    text.bytes().fold(14695981039346656037_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(1099511628211)
    }) & ((1 << 53) - 1)
}
fn read(root: &Path, id: &str) -> Result<MyNotes, String> {
    let path = library::folder(root, id)?.join("my-notes.md");
    let text = if path.try_exists().map_err(|e| e.to_string())? {
        library::text(&path)?
    } else {
        String::new()
    };
    Ok(MyNotes {
        meeting_id: id.into(),
        version: version(&text),
        text,
    })
}
fn remember(history: &mut HashMap<String, Vec<Revision>>, notes: &MyNotes) {
    if !history.contains_key(&notes.meeting_id)
        && history.len() >= 16
        && let Some(old) = history.keys().next().cloned()
    {
        history.remove(&old);
    }
    let revisions = history.entry(notes.meeting_id.clone()).or_default();
    if revisions.last().is_none_or(|r| r.version != notes.version) {
        revisions.push(Revision {
            version: notes.version,
            text: notes.text.clone(),
        });
        if revisions.len() > 32 {
            revisions.remove(0);
        }
    }
}
fn merge(current: &str, base: &str, draft: &str) -> Result<String, String> {
    if current == base {
        return Ok(draft.into());
    }
    let jots = current
        .strip_prefix(base)
        .ok_or("My notes changed in another window. Reload before saving.")?;
    Ok(format!("{draft}{jots}"))
}
fn write(root: &Path, id: &str, text: &str) -> Result<MyNotes, String> {
    if text.len() > 256 * 1024 {
        return Err("My notes must be smaller than 256 KB".into());
    }
    let dir = library::folder(root, id)?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temporary = dir.join(format!(
        ".my-notes-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    // The store lock serializes writers. Refuse a pre-existing temporary path instead of following links.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| format!("Couldn't save My notes: {e}"))?;
    let result = file
        .write_all(text.as_bytes())
        .and_then(|_| file.sync_all())
        .and_then(|_| fs::rename(&temporary, dir.join("my-notes.md")));
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result.map_err(|e| format!("Couldn't save My notes: {e}"))?;
    Ok(MyNotes {
        meeting_id: id.into(),
        text: text.into(),
        version: version(text),
    })
}
#[tauri::command]
pub fn get_my_notes(meeting_id: String, store: tauri::State<'_, Store>) -> Result<MyNotes, String> {
    let mut history = store
        .history
        .lock()
        .map_err(|_| "My notes storage is unavailable")?;
    let notes = read(&store.root, &meeting_id)?;
    remember(&mut history, &notes);
    Ok(notes)
}
#[tauri::command]
pub fn save_my_notes(
    app: tauri::AppHandle,
    meeting_id: String,
    text: String,
    base_version: u64,
) -> Result<MyNotes, String> {
    let store = app.state::<Store>();
    let mut history = store
        .history
        .lock()
        .map_err(|_| "My notes storage is unavailable")?;
    let current = read(&store.root, &meeting_id)?;
    let base = history
        .get(&meeting_id)
        .and_then(|revisions| revisions.iter().find(|r| r.version == base_version))
        .ok_or("My notes changed. Reload before saving.")?;
    let merged = merge(&current.text, &base.text, &text)?;
    let result = write(&store.root, &meeting_id, &merged)?;
    remember(&mut history, &result);
    app.emit("motet://my-notes", &result)
        .map_err(|e| e.to_string())?;
    Ok(result)
}
#[tauri::command]
pub fn add_jot(app: tauri::AppHandle, text: String) -> Result<MyNotes, String> {
    if text.len() > 256 * 1024 {
        return Err("My notes must be smaller than 256 KB".into());
    }
    let state = app.state::<Controller>().state()?;
    let id = state
        .meeting_id
        .ok_or("Start a recording before adding a note")?;
    let store = app.state::<Store>();
    let mut history = store
        .history
        .lock()
        .map_err(|_| "My notes storage is unavailable")?;
    let current = read(&store.root, &id)?;
    remember(&mut history, &current);
    let seconds = state.elapsed_ms / 1000;
    let text = text.trim();
    if text.is_empty() {
        return Ok(current);
    }
    let addition = text
        .lines()
        .map(|line| format!("[{:02}:{:02}] {}", seconds / 60, seconds % 60, line.trim()))
        .collect::<Vec<_>>()
        .join("\n");
    let result = write(&store.root, &id, &format!("{}\n{}", current.text, addition))?;
    remember(&mut history, &result);
    app.emit("motet://my-notes", &result)
        .map_err(|e| e.to_string())?;
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_editor_keeps_concurrent_jots_and_rejects_replacements() {
        assert_eq!(
            merge("first\n[00:10] jot", "first", "edited").unwrap(),
            "edited\n[00:10] jot"
        );
        assert!(merge("other edit", "first", "edited").is_err());
        assert!(version("text") < (1 << 53));
    }
    #[test]
    fn notes_are_atomic_and_stale_temporary_files_do_not_block_saves() {
        let root = std::env::temp_dir().join(format!("motet-notes-{}", std::process::id()));
        fs::create_dir_all(root.join("123")).unwrap();
        fs::write(root.join("123/.my-notes.tmp"), "stale").unwrap();
        let saved = write(&root, "123", "[00:12] [] Follow up").unwrap();
        assert_eq!(read(&root, "123").unwrap().text, saved.text);
        assert_eq!(read(&root, "123").unwrap().version, saved.version);
        assert!(write(&root, "../outside", "text").is_err());
        assert!(write(&root, "123", &"x".repeat(256 * 1024 + 1)).is_err());
        assert_eq!(read(&root, "123").unwrap().text, saved.text);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn export_notes_contract() {
        MyNotes::export_all(&ts_rs::Config::default().with_out_dir("../../ui/lib/bindings"))
            .unwrap();
    }
}

//! Persist task completion without overwriting a newer notes document.
use crate::{library, library_commands::Library};
use std::{
    fs,
    io::Write,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
use tauri::Emitter;
static NEXT: AtomicU64 = AtomicU64::new(0);

fn update(
    root: &Path,
    id: &str,
    line: usize,
    done: bool,
    expected: &str,
) -> Result<String, String> {
    let dir = library::folder(root, id)?;
    let path = dir.join("notes.md");
    let current = library::text(&path)?;
    if current != expected {
        return Err("Notes changed. Reopen this meeting before editing.".into());
    }
    let mut lines: Vec<_> = current.split('\n').map(str::to_owned).collect();
    let task = lines.get_mut(line).ok_or("Action item no longer exists")?;
    let prefix = task.len() - task.trim_start().len();
    let trimmed = &task[prefix..];
    if !["- [ ] ", "- [x] ", "- [X] ", "* [ ] ", "* [x] ", "* [X] "]
        .iter()
        .any(|p| trimmed.starts_with(p))
    {
        return Err("This line is not an action item".into());
    }
    task.replace_range(prefix + 3..prefix + 4, if done { "x" } else { " " });
    let result = lines.join("\n");
    let temporary = dir.join(format!(
        ".tasks-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let saved = (|| {
        file.write_all(result.as_bytes())
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        if library::text(&path)? != current {
            return Err("Notes changed while saving".into());
        }
        fs::rename(&temporary, &path).map_err(|e| e.to_string())
    })();
    if saved.is_err() {
        let _ = fs::remove_file(temporary);
    }
    saved?;
    Ok(result)
}
#[tauri::command]
pub fn set_action_item_done(
    app: tauri::AppHandle,
    state: tauri::State<'_, Library>,
    id: String,
    line: usize,
    done: bool,
    expected: String,
) -> Result<String, String> {
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("Wait for the current recording or processing to finish".into());
    }
    let result = update(&state.root, &id, line, done, &expected);
    state.busy.store(false, Ordering::SeqCst);
    if result.is_ok() {
        let _ = app.emit("motet://library-changed", ());
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_document_and_rejects_stale_edits() {
        let root = std::env::temp_dir().join(format!("motet-tasks-{}", std::process::id()));
        fs::create_dir_all(root.join("1")).unwrap();
        let text = "### Action items\n- [ ] You: Share it [01:20]\n\n### Overview\nKeep this.\n";
        fs::write(root.join("1/notes.md"), text).unwrap();
        let changed = update(&root, "1", 1, true, text).unwrap();
        assert_eq!(changed, text.replacen("[ ]", "[x]", 1));
        assert!(update(&root, "1", 1, false, text).is_err());
        assert!(update(&root, "1", 4, true, &changed).is_err());
        assert_eq!(update(&root, "1", 1, false, &changed).unwrap(), text);
        assert!(update(&root, "../1", 1, true, text).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}

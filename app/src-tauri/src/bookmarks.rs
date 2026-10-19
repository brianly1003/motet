//! Serialized meeting metadata writes preserve bookmarks across worker status updates.
use crate::library::{self, MeetingSummary};
use std::{
    fs,
    io::Write,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
static WRITER: Mutex<()> = Mutex::new(());
static NEXT: AtomicU64 = AtomicU64::new(0);

pub fn save(root: &Path, value: &MeetingSummary) -> Result<(), String> {
    let _guard = WRITER
        .lock()
        .map_err(|_| "Meeting storage is unavailable")?;
    let mut value = value.clone();
    let path = library::folder(root, &value.id)?.join("meeting.json");
    if path.try_exists().map_err(|e| e.to_string())? {
        value.marks_ms = library::summary(root, &value.id)?.marks_ms;
    }
    write(root, &value)
}

pub fn mark(root: &Path, id: &str, at_ms: u64) -> Result<(), String> {
    let _guard = WRITER
        .lock()
        .map_err(|_| "Meeting storage is unavailable")?;
    let mut value = library::summary(root, id)?;
    if !value.marks_ms.contains(&at_ms) {
        value.marks_ms.push(at_ms);
        value.marks_ms.sort_unstable();
        write(root, &value)?;
    }
    Ok(())
}

fn write(root: &Path, value: &MeetingSummary) -> Result<(), String> {
    let dir = library::folder(root, &value.id)?;
    let temporary = dir.join(format!(
        ".meeting-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = file
        .write_all(&bytes)
        .and_then(|_| file.sync_all())
        .and_then(|_| fs::rename(&temporary, dir.join("meeting.json")));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|e| format!("Couldn't save meeting metadata: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_worker_updates_keep_marks_and_old_meetings_load() {
        let root = std::env::temp_dir().join(format!(
            "motet-marks-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("123")).unwrap();
        fs::write(
            root.join("123/meeting.json"),
            r#"{"id":"123","title":"Call","created_ms":0,"status":"Recording"}"#,
        )
        .unwrap();
        let mut stale = library::summary(&root, "123").unwrap();
        assert!(stale.marks_ms.is_empty());
        mark(&root, "123", 25_000).unwrap();
        mark(&root, "123", 25_000).unwrap();
        stale.status = "Ready".into();
        save(&root, &stale).unwrap();
        assert_eq!(
            library::summary(&root, "123").unwrap().marks_ms,
            vec![25_000]
        );
        assert!(mark(&root, "../123", 10).is_err());
        fs::write(root.join("123/meeting.json"), "damaged metadata").unwrap();
        assert!(mark(&root, "123", 30_000).is_err());
        assert!(save(&root, &stale).is_err());
        assert_eq!(
            fs::read_to_string(root.join("123/meeting.json")).unwrap(),
            "damaged metadata"
        );
        fs::remove_dir_all(root).unwrap();
    }
}

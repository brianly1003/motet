//! Local import folders; source media is copied before the CLI pipeline writes intermediates.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use ts_rs::TS;

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct MeetingSummary {
    pub id: String,
    pub title: String,
    #[ts(type = "number")]
    pub created_ms: u64,
    pub status: String,
    #[serde(default)]
    #[ts(type = "number[]")]
    pub marks_ms: Vec<u64>,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub notes_ms: Option<u64>,
    #[serde(default)]
    pub spellings: Vec<String>,
}

#[derive(Serialize, TS)]
pub struct Meeting {
    pub summary: MeetingSummary,
    pub notes_md: String,
    pub transcript: String,
}

pub fn folder(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !is_id(id) {
        return Err("Invalid meeting ID".into());
    }
    let path = root.join(id).canonicalize().map_err(|e| e.to_string())?;
    if path.parent() != Some(root.canonicalize().map_err(|e| e.to_string())?.as_path()) {
        return Err("Meeting is outside the notes folder".into());
    }
    Ok(path)
}

pub(crate) fn text(path: &Path) -> Result<String, String> {
    if path.canonicalize().map_err(|e| e.to_string())?.parent()
        != Some(
            path.parent()
                .ok_or("Missing parent folder")?
                .canonicalize()
                .map_err(|e| e.to_string())?
                .as_path(),
        )
    {
        return Err("Meeting file points outside its folder".into());
    }
    let mut result = String::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(8 * 1024 * 1024 + 1)
        .read_to_string(&mut result)
        .map_err(|e| e.to_string())?;
    if result.len() > 8 * 1024 * 1024 {
        return Err("Meeting text is too large to display".into());
    }
    Ok(result)
}

/// Every meeting folder, newest first. Folders that aren't meetings (no numeric name) are skipped;
/// a meeting whose `meeting.json` can't be read is listed as NeedsAttention instead of hiding the
/// whole library.
pub fn list(root: &Path) -> Result<Vec<MeetingSummary>, String> {
    let mut meetings = Vec::new();
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let Ok(entry) = entry else { continue };
        let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !entry.file_type().is_ok_and(|t| t.is_dir()) || !is_id(&id) {
            continue;
        }
        meetings.push(summary(root, &id).unwrap_or_else(|_| unreadable(&id)));
    }
    meetings.sort_by_key(|m| std::cmp::Reverse(m.created_ms));
    Ok(meetings)
}

/// Bytes of the files in every listed meeting folder, for the sidebar footer. `DirEntry`
/// metadata does not follow links, so a linked file is not counted.
pub fn bytes(root: &Path) -> Result<u64, String> {
    let mut total = 0;
    for meeting in list(root)? {
        let Ok(folder) = folder(root, &meeting.id) else {
            continue;
        };
        for entry in fs::read_dir(folder).into_iter().flatten().flatten() {
            if let Ok(meta) = entry.metadata()
                && meta.is_file()
            {
                total += meta.len();
            }
        }
    }
    Ok(total)
}

/// Search locally, including saved text, without loading every meeting into the webview.
pub fn search(root: &Path, query: &str) -> Result<Vec<MeetingSummary>, String> {
    let query = query.trim().to_lowercase();
    Ok(list(root)?
        .into_iter()
        .filter(|meeting| {
            if meeting.title.to_lowercase().contains(&query) {
                return true;
            }
            let Ok(folder) = folder(root, &meeting.id) else {
                return false;
            };
            [
                "notes.md",
                "transcript.txt",
                "transcript.fixed.txt",
                "my-notes.md",
            ]
            .iter()
            .any(|name| {
                text(&folder.join(name)).is_ok_and(|text| text.to_lowercase().contains(&query))
            })
        })
        .collect())
}

fn is_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|v| v.is_ascii_digit())
}

pub fn summary(root: &Path, id: &str) -> Result<MeetingSummary, String> {
    let dir = folder(root, id)?;
    let mut summary: MeetingSummary = serde_json::from_str(&text(&dir.join("meeting.json"))?)
        .map_err(|e| format!("Cannot read meeting {id}: {e}"))?;
    if summary.id != id {
        return Err("Meeting ID does not match its folder".into());
    }
    summary.duration_ms = summary
        .duration_ms
        .or_else(|| crate::meeting_media::duration(root, id).ok().flatten());
    Ok(summary)
}

/// A placeholder for a meeting folder whose details are missing or damaged. Imports name folders
/// by nanoseconds since 1970, so the name still gives the date.
fn unreadable(id: &str) -> MeetingSummary {
    MeetingSummary {
        id: id.into(),
        title: "Meeting that couldn't be read".into(),
        created_ms: id.parse::<u128>().map_or(0, |ns| (ns / 1_000_000) as u64),
        status: "NeedsAttention".into(),
        marks_ms: vec![],
        duration_ms: None,
        notes_ms: None,
        spellings: vec![],
    }
}

/// Imports that were still Finishing when the app last stopped can't be resumed; mark them
/// NeedsAttention so they don't look busy forever. Call once at launch, before any import runs.
pub fn recover_interrupted(root: &Path) -> Result<usize, String> {
    let mut recovered = 0;
    for mut meeting in list(root)? {
        if matches!(
            meeting.status.as_str(),
            "Finishing" | "Recording" | "Stopped"
        ) {
            meeting.status = "NeedsAttention".into();
            save(root, &meeting)?;
            recovered += 1;
        }
    }
    Ok(recovered)
}

pub fn read(root: &Path, id: &str) -> Result<Meeting, String> {
    let dir = folder(root, id)?;
    let summary = summary(root, id)?;
    let notes_md = if dir.join("notes.md").exists() {
        text(&dir.join("notes.md"))?
    } else {
        String::new()
    };
    let transcript = if dir.join("transcript.txt").exists() {
        text(&dir.join("transcript.txt"))?
    } else {
        String::new()
    };
    Ok(Meeting {
        summary,
        notes_md,
        transcript,
    })
}

pub use crate::bookmarks::save;

/// Meetings whose notes still have an unchecked action item, newest first, with their notes,
/// for the Library's Open action items. The UI parses the tasks as the meeting page does.
/// reads every notes.md on each call; an index when libraries reach thousands.
pub fn open_items(root: &Path) -> Result<Vec<(MeetingSummary, String)>, String> {
    let mut open = Vec::new();
    for meeting in list(root)? {
        let Ok(dir) = folder(root, &meeting.id) else {
            continue;
        };
        let Ok(notes) = text(&dir.join("notes.md")) else {
            continue;
        };
        let tasks = notes.split("<details>").next().unwrap_or_default();
        if tasks.lines().any(|line| {
            let line = line.trim_start();
            line.starts_with("- [ ] ") || line.starts_with("* [ ] ")
        }) {
            open.push((meeting, notes));
        }
    }
    Ok(open)
}

pub fn import(root: &Path, source: &Path) -> Result<MeetingSummary, String> {
    // Checked before a folder exists, so a wrong pick leaves nothing behind.
    let extension = source
        .extension()
        .and_then(|s| s.to_str())
        .filter(|s| s.bytes().all(|b| b.is_ascii_alphanumeric()))
        .ok_or("Choose an audio or video file with an extension")?
        .to_owned();
    let created_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as u64;
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos()
        .to_string();
    let mut summary = MeetingSummary {
        id,
        title: source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported meeting")
            .chars()
            .take(120)
            .collect(),
        created_ms,
        status: "Finishing".into(),
        marks_ms: vec![],
        duration_ms: None,
        notes_ms: None,
        spellings: vec![],
    };
    let dir = root.join(&summary.id);
    fs::create_dir(&dir).map_err(|e| e.to_string())?;
    if let Err(e) = save(root, &summary) {
        // An empty folder would show up as an unreadable meeting.
        let _ = fs::remove_dir(&dir);
        return Err(format!("Couldn't create the meeting folder: {e}"));
    }
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let copied = dir.join(format!("source.{extension}"));
        fs::copy(source, &copied)?;
        let transcript = motet_core::transcribe::transcript_parts(
            &[(vec![copied.to_string_lossy().into_owned()], 0)],
            true,
        )?;
        fs::write(dir.join("transcript.txt"), &transcript)?;
        let notes_input = match motet_core::config::var("FIX_MODEL").filter(|v| !v.is_empty()) {
            Some(model) => {
                let fixed = motet_core::correct::transcript(&model, &transcript)?;
                fs::write(dir.join("transcript.fixed.txt"), &fixed)?;
                fixed
            }
            None => transcript,
        };
        let started = std::time::Instant::now();
        let spellings = used_spellings();
        motet_core::summarize::notes(&dir.join("notes").to_string_lossy(), &notes_input, "")?;
        summary.notes_ms = Some(started.elapsed().as_millis() as u64);
        summary.spellings = spellings;
        Ok(())
    })();
    summary.status = if result.is_ok() {
        "Ready"
    } else {
        "NeedsAttention"
    }
    .into();
    // The pipeline's error is the one people need; a failed status save is added, not swapped in.
    let saved = save(root, &summary);
    match (result, saved) {
        (Ok(()), Ok(())) => Ok(summary),
        (Ok(()), Err(e)) => Err(format!(
            "Notes for {} are written, but its status couldn't be saved: {e}",
            summary.title
        )),
        (Err(e), saved) => Err(format!(
            "Couldn't finish {}: {e}. Saved files remain in the notes folder.{}",
            summary.title,
            saved.err().map_or(String::new(), |s| format!(
                " (Its status couldn't be saved either: {s}.)"
            ))
        )),
    }
}

pub(crate) fn used_spellings() -> Vec<String> {
    motet_core::config::terms()
        .map(|terms| terms.split(", ").map(str::to_owned).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bytes_sum_meeting_files_but_not_links() {
        let root = temp_root();
        fs::create_dir(root.join("123")).unwrap();
        fs::write(
            root.join("123/meeting.json"),
            r#"{"id":"123","title":"A","created_ms":1,"status":"Ready"}"#,
        )
        .unwrap();
        fs::write(root.join("123/part-0000.mic.wav"), vec![0u8; 1000]).unwrap();
        let json = fs::metadata(root.join("123/meeting.json")).unwrap().len();
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc/hosts", root.join("123/link.wav")).unwrap();
        assert_eq!(bytes(&root).unwrap(), 1000 + json);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn open_items_lists_only_meetings_with_unchecked_tasks() {
        let root = temp_root();
        for (id, notes) in [
            ("1", "### Action items\n- [x] Done already\n"),
            (
                "2",
                "### Action items\n- [ ] You: Share the design [01:20]\n",
            ),
            ("3", "<details>\n- [ ] Inside the transcript\n</details>"),
        ] {
            fs::create_dir(root.join(id)).unwrap();
            fs::write(
                root.join(id).join("meeting.json"),
                format!(r#"{{"id":"{id}","title":"M{id}","created_ms":{id},"status":"Ready"}}"#),
            )
            .unwrap();
            fs::write(root.join(id).join("notes.md"), notes).unwrap();
        }
        let open = open_items(&root).unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].0.id, "2");
        assert!(open[0].1.contains("Share the design"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn search_matches_notes_and_transcript_without_exposing_paths() {
        let root = temp_root();
        fs::create_dir(root.join("123")).unwrap();
        fs::write(
            root.join("123/meeting.json"),
            r#"{"id":"123","title":"Design review","created_ms":1,"status":"Ready"}"#,
        )
        .unwrap();
        fs::write(
            root.join("123/transcript.txt"),
            "[00:04] Me: Publish the roadmap",
        )
        .unwrap();
        fs::write(
            root.join("123/my-notes.md"),
            "[00:05] [] Follow up Thursday",
        )
        .unwrap();
        assert_eq!(search(&root, " ROADMAP ").unwrap().len(), 1);
        assert_eq!(search(&root, "Thursday").unwrap().len(), 1);
        assert!(search(&root, "unrelated").unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_paths_outside_library() {
        let root = std::env::temp_dir();
        for id in ["", "..", "../secret", "/tmp", "123/456"] {
            assert!(folder(&root, id).is_err());
        }
    }

    fn temp_root() -> PathBuf {
        // Parallel tests can read the same clock tick; the counter keeps folders apart.
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let root = std::env::temp_dir().join(format!(
            "motet-library-{}-{}",
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        root
    }
    #[test]
    fn saved_meetings_roundtrip_and_list_newest_first() {
        let root = temp_root();
        for id in ["1", "2"] {
            fs::create_dir(root.join(id)).unwrap();
            save(
                &root,
                &MeetingSummary {
                    id: id.into(),
                    title: format!("Meeting {id}"),
                    created_ms: id.parse().unwrap(),
                    status: "Ready".into(),
                    marks_ms: vec![],
                    duration_ms: None,
                    notes_ms: None,
                    spellings: vec![],
                },
            )
            .unwrap();
        }
        fs::write(root.join("2/notes.md"), "### Action items").unwrap();
        fs::write(root.join("2/transcript.txt"), "[00:00] Hello").unwrap();
        assert_eq!(list(&root).unwrap()[0].id, "2");
        let meeting = read(&root, "2").unwrap();
        assert_eq!(meeting.notes_md, "### Action items");
        assert_eq!(meeting.transcript, "[00:00] Hello");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn list_skips_other_folders_and_flags_unreadable_meetings() {
        let root = temp_root();
        fs::create_dir(root.join("Archive")).unwrap();
        fs::write(root.join("notes.txt"), "not a meeting").unwrap();
        fs::create_dir(root.join("1700000000000000000")).unwrap(); // no meeting.json
        fs::create_dir(root.join("2")).unwrap();
        fs::write(root.join("2/meeting.json"), "{ broken").unwrap();
        let meetings = list(&root).unwrap();
        assert_eq!(meetings.len(), 2);
        assert!(meetings.iter().all(|m| m.status == "NeedsAttention"));
        assert_eq!(meetings[0].created_ms, 1_700_000_000_000);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_imports_need_attention_after_restart() {
        let root = temp_root();
        for (id, status) in [("1", "Finishing"), ("2", "Ready")] {
            fs::create_dir(root.join(id)).unwrap();
            let summary = MeetingSummary {
                id: id.into(),
                title: id.into(),
                created_ms: 1,
                status: status.into(),
                marks_ms: vec![],
                duration_ms: None,
                notes_ms: None,
                spellings: vec![],
            };
            save(&root, &summary).unwrap();
        }
        assert_eq!(recover_interrupted(&root).unwrap(), 1);
        assert_eq!(read(&root, "1").unwrap().summary.status, "NeedsAttention");
        assert_eq!(read(&root, "2").unwrap().summary.status, "Ready");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn import_without_extension_leaves_no_folder() {
        let root = temp_root();
        assert!(import(&root, Path::new("/tmp/recording")).is_err());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_import_keeps_its_error_and_needs_attention() {
        let root = temp_root();
        let missing = root.join("gone.m4a"); // copy fails: the pipeline error must surface
        let err = import(&root, &missing).unwrap_err();
        assert!(err.starts_with("Couldn't finish gone:"), "{err}");
        let meetings = list(&root).unwrap();
        assert_eq!(meetings.len(), 1);
        assert_eq!(meetings[0].status, "NeedsAttention");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_external_symlinks() {
        let root = temp_root();
        let outside = temp_root();
        fs::write(outside.join("secret"), "private").unwrap();
        fs::create_dir(root.join("1")).unwrap();
        std::os::unix::fs::symlink(outside.join("secret"), root.join("1/notes.md")).unwrap();
        assert!(text(&root.join("1/notes.md")).is_err());
        std::os::unix::fs::symlink(&outside, root.join("2")).unwrap();
        assert!(folder(&root, "2").is_err());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }
    #[test]
    #[ignore = "requires synthetic speech and installed local models"]
    fn real_import_preserves_source_and_writes_notes() {
        let source = PathBuf::from(std::env::var("MOTET_TEST_IMPORT").unwrap());
        let before = fs::read(&source).unwrap();
        let root = temp_root();
        let imported = import(&root, &source).unwrap();
        let meeting = read(&root, &imported.id).unwrap();
        assert_eq!(imported.status, "Ready");
        assert!(meeting.transcript.to_lowercase().contains("friday"));
        assert!(!meeting.notes_md.is_empty());
        assert_eq!(fs::read(&source).unwrap(), before);
        assert!(!source.with_extension("csv").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn export_bindings() {
        let config = ts_rs::Config::default().with_out_dir("../../ui/lib/bindings");
        Meeting::export_all(&config).unwrap();
        motet_core::events::Event::export_all(&config).unwrap();
    }
}

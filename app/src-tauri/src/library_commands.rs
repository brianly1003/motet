//! Library IPC and import orchestration.
use crate::{desktop::refresh_tray, library};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

pub(crate) struct Library {
    pub(crate) root: PathBuf,
    pub(crate) busy: Arc<AtomicBool>,
    pub(crate) importing: AtomicBool,
    pub(crate) allow_exit: Arc<AtomicBool>,
    pub(crate) quit_pending: AtomicBool,
}

#[tauri::command]
pub(crate) fn list_meetings(
    state: tauri::State<'_, Library>,
) -> Result<Vec<library::MeetingSummary>, String> {
    #[cfg(debug_assertions)]
    eprintln!("motet: main window connected");
    library::list(&state.root)
}
#[tauri::command]
pub(crate) fn library_bytes(state: tauri::State<'_, Library>) -> Result<u64, String> {
    library::bytes(&state.root)
}
#[tauri::command]
pub(crate) fn get_meeting(
    id: String,
    state: tauri::State<'_, Library>,
) -> Result<library::Meeting, String> {
    library::read(&state.root, &id)
}
#[tauri::command]
pub(crate) async fn open_action_items(
    app: tauri::AppHandle,
) -> Result<Vec<(library::MeetingSummary, String)>, String> {
    let root = app.state::<Library>().root.clone();
    tauri::async_runtime::spawn_blocking(move || library::open_items(&root))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) fn importing(state: tauri::State<'_, Library>) -> bool {
    state.importing.load(Ordering::SeqCst)
}

const MEDIA: &[&str] = &[
    "wav", "mp3", "m4a", "aac", "mp4", "mov", "flac", "aiff", "ogg", "opus", "webm",
];

#[tauri::command]
pub(crate) async fn import_audio(
    app: tauri::AppHandle,
) -> Result<Option<library::MeetingSummary>, String> {
    import_file(app, None).await
}

/// Imports a picked file, or one dropped on the library window. Drops arrive from the native
/// window event, so the web page never hands Rust a path.
pub(crate) async fn import_file(
    app: tauri::AppHandle,
    dropped: Option<PathBuf>,
) -> Result<Option<library::MeetingSummary>, String> {
    if let Some(path) = &dropped
        && !path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| MEDIA.contains(&e.to_ascii_lowercase().as_str()))
    {
        return Err("Drop an audio or video file (wav, mp3, m4a, mp4, mov…).".into());
    }
    let state = app.state::<Library>();
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("A meeting is already being processed".into());
    }
    state.importing.store(true, Ordering::SeqCst);
    refresh_tray(&app);
    let _ = app.emit("motet://library-changed", ());
    let worker = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let source = match dropped {
            Some(path) => path,
            None => {
                let Some(file) = worker
                    .dialog()
                    .file()
                    .add_filter("Audio or video", MEDIA)
                    .blocking_pick_file()
                else {
                    return Ok(None);
                };
                file.into_path().map_err(|e| e.to_string())?
            }
        };
        let root = &worker.state::<Library>().root;
        library::import(root, &source).map(Some)
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    if result.as_ref().is_ok_and(|r| r.is_some())
        && let Err(error) = app
            .state::<crate::settings::Store>()
            .get()
            .and_then(|s| crate::settings_storage::retain(&state.root, &s.keep_audio))
    {
        let _ = app.emit(
            "motet://notice",
            format!("Notes are ready, but audio retention failed: {error}"),
        );
    }
    state.importing.store(false, Ordering::SeqCst);
    state.busy.store(false, Ordering::SeqCst);
    refresh_tray(&app);
    let _ = app.emit("motet://library-changed", ());
    match &result {
        Ok(Some(meeting)) => crate::notifications::send(
            &app,
            &meeting.title,
            "Your meeting notes are ready. Open Motet to read them.",
        ),
        Err(_) => crate::notifications::send(
            &app,
            "Couldn’t write the notes",
            "Open Motet to review the problem. Your source file is unchanged.",
        ),
        _ => {}
    }
    if state.quit_pending.load(Ordering::SeqCst) {
        state.allow_exit.store(true, Ordering::SeqCst);
        app.exit(0);
    }
    result
}

#[tauri::command]
pub(crate) async fn search_meetings(
    app: tauri::AppHandle,
    query: String,
) -> Result<Vec<library::MeetingSummary>, String> {
    if query.len() > 1024 {
        return Err("Search is too long".into());
    }
    let root = app.state::<Library>().root.clone();
    tauri::async_runtime::spawn_blocking(move || library::search(&root, &query))
        .await
        .map_err(|e| e.to_string())?
}

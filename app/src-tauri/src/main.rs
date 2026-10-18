//! Desktop library shell over the local pipeline. No recording starts from app launch.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod library;

use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

struct Library {
    root: PathBuf,
    busy: AtomicBool,
    quit_pending: AtomicBool,
}

#[tauri::command]
fn list_meetings(state: tauri::State<'_, Library>) -> Result<Vec<library::MeetingSummary>, String> {
    #[cfg(debug_assertions)]
    eprintln!("motet: main window connected");
    library::list(&state.root)
}
#[tauri::command]
fn get_meeting(id: String, state: tauri::State<'_, Library>) -> Result<library::Meeting, String> {
    library::read(&state.root, &id)
}
#[tauri::command]
fn importing(state: tauri::State<'_, Library>) -> bool {
    state.busy.load(Ordering::SeqCst)
}

#[tauri::command]
async fn import_audio(app: tauri::AppHandle) -> Result<Option<library::MeetingSummary>, String> {
    let state = app.state::<Library>();
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("A meeting is already being processed".into());
    }
    let worker = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = worker
            .dialog()
            .file()
            .add_filter(
                "Audio or video",
                &[
                    "wav", "mp3", "m4a", "aac", "mp4", "mov", "flac", "aiff", "ogg", "opus", "webm",
                ],
            )
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let source = file.into_path().map_err(|e| e.to_string())?;
        let root = &worker.state::<Library>().root;
        library::import(root, &source).map(Some)
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    state.busy.store(false, Ordering::SeqCst);
    let _ = app.emit("motet://library-changed", ());
    if state.quit_pending.load(Ordering::SeqCst) {
        app.exit(0);
    }
    result
}

fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_meetings,
            get_meeting,
            import_audio,
            importing
        ])
        .setup(|app| {
            let root = app.path().document_dir()?.join("Motet").join("imports");
            std::fs::create_dir_all(&root)?;
            // Nothing imports yet at launch, so any Finishing meeting was cut off by a quit or crash.
            if let Err(e) = library::recover_interrupted(&root) {
                eprintln!("Motet: couldn't check for interrupted imports: {e}");
            }
            app.manage(Library {
                root,
                busy: AtomicBool::new(false),
                quit_pending: AtomicBool::new(false),
            });
            let menu = tauri::menu::MenuBuilder::new(app)
                .text("open", "Open Motet")
                .separator()
                .text("quit", "Quit Motet")
                .build()?;
            let mut rgba = vec![0_u8; 32 * 32 * 4];
            for y in 6..26 {
                for x in [9, 10, 11, 12, 19, 20, 21, 22] {
                    let i = (y * 32 + x) * 4;
                    rgba[i + 3] = 255;
                }
            }
            tauri::tray::TrayIconBuilder::with_id("motet")
                .icon(tauri::image::Image::new_owned(rgba, 32, 32))
                .icon_as_template(true)
                .tooltip("Motet · On this computer")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("could not build Motet")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                let state = app.state::<Library>();
                if state.busy.load(Ordering::SeqCst) {
                    state.quit_pending.store(true, Ordering::SeqCst);
                    api.prevent_exit();
                    show(app);
                    let _ = app.emit("motet://quit-pending", ());
                }
            }
        });
}

//! Tauri commands, windows and native menu adapter.
use crate::{
    controller::{Action, Controller, Host, Job},
    library, permissions, recording,
};
use motet_core::events::{AppState, Event, Readiness, SessionState};
use std::sync::{Arc, mpsc};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

struct Library {
    root: PathBuf,
    busy: Arc<AtomicBool>,
    importing: AtomicBool,
    allow_exit: Arc<AtomicBool>,
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
    state.importing.load(Ordering::SeqCst)
}

#[tauri::command]
async fn import_audio(app: tauri::AppHandle) -> Result<Option<library::MeetingSummary>, String> {
    let state = app.state::<Library>();
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("A meeting is already being processed".into());
    }
    state.importing.store(true, Ordering::SeqCst);
    refresh_tray(&app);
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

pub(crate) fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            list_meetings,
            get_meeting,
            import_audio,
            importing,
            get_state,
            start_recording,
            stop_recording,
            resume_recording,
            quit,
            dismiss_problem,
            set_companion_size,
            companion_ready,
            resume_remaining_ms,
            notification_permission
        ])
        .setup(|app| {
            let root = app.path().document_dir()?.join("Motet").join("imports");
            std::fs::create_dir_all(&root)?;
            // Nothing imports yet at launch, so any Finishing meeting was cut off by a quit or crash.
            if let Err(e) = library::recover_interrupted(&root) {
                eprintln!("Motet: couldn't check for interrupted imports: {e}");
            }
            let busy = Arc::new(AtomicBool::new(false));
            let allow_exit = Arc::new(AtomicBool::new(false));
            let (sender, receiver) = mpsc::channel();
            let jobs = recording::spawn(
                root.clone(),
                app.handle().clone(),
                sender.clone(),
                allow_exit.clone(),
            )?;
            let host = Desktop {
                app: app.handle().clone(),
                root: root.clone(),
                jobs,
            };
            app.manage(Controller::spawn(host, sender, receiver, busy.clone())?);
            app.manage(Library {
                root,
                busy,
                importing: AtomicBool::new(false),
                allow_exit,
                quit_pending: AtomicBool::new(false),
            });
            crate::companion::create(app.handle())?;
            crate::notifications::setup(app.handle())?;
            app.manage(crate::tray::State::default());
            let menu = crate::tray::initial(app.handle())?;
            tauri::tray::TrayIconBuilder::with_id("motet")
                .icon(crate::tray::icon(false))
                .icon_as_template(true)
                .tooltip("Motet · On this computer")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show(app),
                    "record" => dispatch(app.clone(), Action::Start),
                    "resume" => dispatch(app.clone(), Action::Resume),
                    "stop" => dispatch(app.clone(), Action::Stop),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let active = window
                    .app_handle()
                    .state::<Controller>()
                    .state()
                    .is_ok_and(|s| {
                        matches!(
                            s.state,
                            SessionState::Recording | SessionState::Stopped { .. }
                        )
                    });
                if !active
                    || (window.label() == "main"
                        && crate::companion::available(window.app_handle()))
                {
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("could not build Motet")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                let state = app.state::<Library>();
                if !state.allow_exit.load(Ordering::SeqCst) {
                    api.prevent_exit();
                    if state.importing.load(Ordering::SeqCst) {
                        state.quit_pending.store(true, Ordering::SeqCst);
                        show(app);
                        let _ = app.emit("motet://quit-pending", ());
                    } else {
                        dispatch(app.clone(), Action::RequestQuit);
                    }
                }
            }
        });
}

pub(crate) fn dispatch(app: tauri::AppHandle, action: Action) {
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = app.state::<Controller>().action(action) {
            let _ = app.emit("motet://notice", e);
        }
    });
}
async fn act(app: tauri::AppHandle, action: Action) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Controller>().action(action))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn get_state(controller: tauri::State<'_, Controller>) -> Result<AppState, String> {
    controller.state()
}
#[tauri::command]
async fn start_recording(app: tauri::AppHandle, notes_only: Option<bool>) -> Result<(), String> {
    if notes_only == Some(true) {
        return Err("Notes-only sessions are not implemented yet".into());
    }
    act(app, Action::Start).await
}
#[tauri::command]
async fn stop_recording(app: tauri::AppHandle) -> Result<(), String> {
    act(app, Action::Stop).await
}
#[tauri::command]
async fn resume_recording(app: tauri::AppHandle) -> Result<(), String> {
    act(app, Action::Resume).await
}
#[tauri::command]
async fn quit(app: tauri::AppHandle, stop_and_save: bool) -> Result<(), String> {
    act(app, Action::Quit(stop_and_save)).await
}
#[tauri::command]
async fn dismiss_problem(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    act(app, Action::Dismiss(id)).await
}

struct Desktop {
    app: tauri::AppHandle,
    root: PathBuf,
    jobs: mpsc::Sender<Job>,
}
impl Host for Desktop {
    fn readiness(&self) -> Result<Readiness, String> {
        let (microphone, system_audio) = permissions::audio()?;
        let disk = std::process::Command::new("df")
            .arg("-Pk")
            .arg(&self.root)
            .output()
            .map_err(|e| format!("Couldn't check free disk space: {e}"))?;
        if !disk.status.success() {
            return Err("Couldn't check free disk space".into());
        }
        let free_bytes = String::from_utf8_lossy(&disk.stdout)
            .lines()
            .nth(1)
            .and_then(|line| line.split_whitespace().nth(3))
            .and_then(|n| n.parse::<u64>().ok())
            .and_then(|n| n.checked_mul(1024))
            .ok_or("Couldn't read free disk space")?;
        let speech_model = motet_core::config::var("WHISPER_MODEL")
            .is_some_and(|p| std::fs::File::open(p).is_ok());
        Ok(Readiness {
            microphone,
            system_audio,
            notifications: permissions::notifications(false).unwrap_or(false),
            speech_model,
            free_bytes,
        })
    }
    fn event(&self, event: Event) -> Result<(), String> {
        crate::notifications::event(&self.app, &event);
        if let Event::StateChanged(state) = &event {
            crate::companion::update(&self.app, state)?;
            crate::tray::update(
                &self.app,
                &state.state,
                self.app.state::<Library>().importing.load(Ordering::SeqCst),
            )?;
            if let Some(tray) = self.app.tray_by_id("motet") {
                tray.set_tooltip(Some(if state.state == SessionState::Recording {
                    "Motet - Recording microphone and system audio; Stop in this menu"
                } else {
                    "Motet - On this computer"
                }))
                .map_err(|e| e.to_string())?;
            }

            if let Some(window) = self.app.get_webview_window("main") {
                window
                    .set_title(if state.state == SessionState::Recording {
                        "Motet - Recording microphone and system audio"
                    } else {
                        "Motet"
                    })
                    .map_err(|e| e.to_string())?;
                if state.quit_pending {
                    show(&self.app);
                }
            }
        }
        self.app
            .emit("motet://event", event)
            .map_err(|e| e.to_string())
    }
    fn announce(&self) -> Result<(), String> {
        crate::notifications::recording(&self.app);
        if crate::companion::available(&self.app) {
            return Ok(());
        }
        let window = self
            .app
            .get_webview_window("main")
            .ok_or("Open Motet before starting a recording")?;
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        window
            .set_title("Motet - Recording microphone and system audio")
            .map_err(|e| e.to_string())
    }
    fn job(&self, job: Job) -> Result<(), String> {
        self.jobs
            .send(job)
            .map_err(|_| "Recording worker stopped".into())
    }
    fn notice(&self, message: &str) {
        if message.starts_with("Recording under ") {
            crate::notifications::send(&self.app, "Recording discarded", message);
        }
        show(&self.app);
        let _ = self.app.emit("motet://notice", message);
    }
    fn exit(&self) {
        if self.jobs.send(Job::Exit).is_err() {
            // A disconnected queue means the worker has already stopped; no WAV is still open.
            self.app
                .state::<Library>()
                .allow_exit
                .store(true, Ordering::SeqCst);
            self.app.exit(0);
        }
    }
}

fn refresh_tray(app: &tauri::AppHandle) {
    let result = app.state::<Controller>().state().and_then(|state| {
        crate::tray::update(
            app,
            &state.state,
            app.state::<Library>().importing.load(Ordering::SeqCst),
        )
    });
    if let Err(error) = result {
        let _ = app.emit("motet://notice", error);
    }
}

#[tauri::command]
async fn set_companion_size(
    app: tauri::AppHandle,
    size: motet_core::events::CompanionSize,
) -> Result<(), String> {
    act(app, Action::Size(size)).await
}
#[tauri::command]
fn companion_ready(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "companion" {
        return Err("Only the companion can report readiness".into());
    }
    crate::companion::ready(window.app_handle());
    Ok(())
}
#[tauri::command]
async fn notification_permission(request: bool) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || permissions::notifications(request))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn resume_remaining_ms(controller: tauri::State<'_, Controller>) -> Result<u64, String> {
    controller.resume_remaining_ms()
}

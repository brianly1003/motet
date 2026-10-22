//! Global recording shortcuts use the same controller as the tray and webview.
use crate::{
    controller::{Action, Controller},
    desktop::dispatch,
};
use motetto_core::events::{CompanionSize, SessionState};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub fn install(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let keys = app.state::<crate::settings::Store>().get()?.shortcuts;
    register(app, &keys)?;
    Ok(())
}
fn register(
    app: &tauri::AppHandle,
    keys: &std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    for (key, accelerator) in keys {
        let key = key.clone();
        app.global_shortcut()
            .on_shortcut(accelerator.as_str(), move |app, _, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                let app = app.clone();
                let key = key.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    if let Err(error) = handle(&app, &key) {
                        let _ = app.emit("motetto://notice", error);
                    }
                });
            })
            .map_err(|e| format!("Shortcut {accelerator} is unavailable. Choose another: {e}"))?;
    }
    Ok(())
}
pub fn replace(
    app: &tauri::AppHandle,
    old: &std::collections::BTreeMap<String, String>,
    next: &std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())?;
    if let Err(error) = register(app, next) {
        let _ = app.global_shortcut().unregister_all();
        register(app, old).map_err(|e| format!("{error}. Couldn't restore shortcuts: {e}"))?;
        return Err(error);
    }
    Ok(())
}
fn handle(app: &tauri::AppHandle, key: &str) -> Result<(), String> {
    let controller = app.state::<Controller>();
    let state = controller.state()?;
    if key == "S" {
        return controller.action(match state.state {
            SessionState::Recording | SessionState::Paused => Action::Stop,
            SessionState::Stopped { .. } => Action::Resume,
            SessionState::Idle => Action::Start,
            _ => return Ok(()),
        });
    }
    if state.state != SessionState::Recording {
        return Ok(());
    }
    match key {
        "M" => {
            controller.mark()?;
        }
        "J" | "N" => open_notes(app, key == "J"),
        "Space" => controller.action(Action::Size(
            if state.companion.size == CompanionSize::Tucked {
                CompanionSize::Sidecar
            } else {
                CompanionSize::Tucked
            },
        ))?,
        "H" => controller.action(Action::Sharing(state.sharing.is_none()))?,
        "P" if state.sharing.is_some() => {
            controller.action(Action::Size(if state.companion.peeking {
                CompanionSize::Dot
            } else {
                CompanionSize::Sidecar
            }))?
        }
        _ => {}
    }
    Ok(())
}
pub fn open_notes(app: &tauri::AppHandle, jot: bool) {
    let sharing = app
        .state::<Controller>()
        .state()
        .is_ok_and(|state| state.sharing.is_some());
    if !jot || !sharing {
        dispatch(app.clone(), Action::Size(CompanionSize::Sidecar));
    }
    if let Some(window) = app.get_webview_window("companion") {
        let _ = window.set_focus();
    }
    let _ = app.emit("motetto://open-notes", if jot { "jot" } else { "notes" });
}

//! Global recording shortcuts use the same controller as the tray and webview.
use crate::{
    controller::{Action, Controller},
    desktop::dispatch,
};
use motet_core::events::{CompanionSize, SessionState};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub fn install(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    for key in ["S", "M", "J", "N", "Space", "H", "P"] {
        app.global_shortcut().on_shortcut(
            format!("Ctrl+Alt+{key}").as_str(),
            move |app, _, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                let app = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let result = handle(&app, key);
                    if let Err(error) = result {
                        let _ = app.emit("motet://notice", error);
                    }
                });
            },
        )?;
    }
    Ok(())
}
fn handle(app: &tauri::AppHandle, key: &str) -> Result<(), String> {
    let controller = app.state::<Controller>();
    let state = controller.state()?;
    if key == "S" {
        return controller.action(match state.state {
            SessionState::Recording => Action::Stop,
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
    let _ = app.emit("motet://open-notes", if jot { "jot" } else { "notes" });
}

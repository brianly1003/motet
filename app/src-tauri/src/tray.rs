//! Native recording actions follow session state; elapsed-time ticks do not rebuild the menu.
use motet_core::events::{AppState, SessionState};
use std::sync::Mutex;
use tauri::{
    Manager,
    menu::{Menu, MenuBuilder, MenuItem, MenuItemBuilder},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Record,
    Stop,
    Resume,
    None,
}
fn action(state: &SessionState, importing: bool) -> Action {
    match state {
        SessionState::Recording => Action::Stop,
        SessionState::Stopped { .. } => Action::Resume,
        SessionState::Idle if !importing => Action::Record,
        _ => Action::None,
    }
}
pub struct State(Mutex<(Action, Option<MenuItem<tauri::Wry>>, String)>);
impl Default for State {
    fn default() -> Self {
        Self(Mutex::new((Action::Record, None, String::new())))
    }
}
fn menu(
    app: &tauri::AppHandle,
    action: Action,
) -> tauri::Result<(Menu<tauri::Wry>, MenuItem<tauri::Wry>)> {
    let header = MenuItemBuilder::with_id(
        "status",
        match action {
            Action::Stop => "Recording",
            Action::Resume => "Stopped · notes start soon",
            Action::None => "Writing notes",
            Action::Record => "Motet · ready for calls",
        },
    )
    .enabled(false)
    .build(app)?;
    let mut builder = MenuBuilder::new(app).item(&header);
    for (id, label, shortcut) in match action {
        Action::Record => vec![("record", "Record now", "Ctrl+Alt+S")],
        Action::Stop => vec![
            ("stop", "Stop recording", "Ctrl+Alt+S"),
            ("mark", "Mark this moment", "Ctrl+Alt+M"),
            ("jot", "Jot a note…", "Ctrl+Alt+J"),
            ("notes", "Open My notes", "Ctrl+Alt+N"),
        ],
        Action::Resume => vec![("resume", "Resume recording", "Ctrl+Alt+S")],
        Action::None => vec![],
    } {
        builder = builder.item(
            &MenuItemBuilder::with_id(id, label)
                .accelerator(shortcut)
                .build(app)?,
        );
    }
    let menu = builder
        .separator()
        .item(
            &MenuItemBuilder::with_id("open", "Open Motet")
                .accelerator("CmdOrCtrl+O")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("settings", "Settings…")
                .accelerator("CmdOrCtrl+,")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("quit", "Quit Motet")
                .accelerator("CmdOrCtrl+Q")
                .build(app)?,
        )
        .build()?;
    Ok((menu, header))
}

pub fn initial(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let (menu, header) = menu(app, Action::Record)?;
    if let Ok(mut state) = app.state::<State>().0.lock() {
        state.1 = Some(header);
    }
    Ok(menu)
}
pub fn update(app: &tauri::AppHandle, state: &AppState, importing: bool) -> Result<(), String> {
    let managed = app.state::<State>();
    let mut previous = managed
        .0
        .lock()
        .map_err(|_| "Tray menu state is unavailable")?;
    let next = action(&state.state, importing);
    if previous.0 != next {
        let tray = app.tray_by_id("motet").ok_or("Tray menu is unavailable")?;
        let (menu, header) = menu(app, next).map_err(|e| e.to_string())?;
        previous.1 = Some(header);
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
        tray.set_icon(Some(icon(next == Action::Stop)))
            .map_err(|e| e.to_string())?;
        tray.set_icon_as_template(true).map_err(|e| e.to_string())?;
        previous.0 = next;
    }
    let seconds = state.elapsed_ms / 1000;
    let label = match state.state {
        SessionState::Recording => format!(
            "Recording · {} · {:02}:{:02}",
            state.app.as_deref().unwrap_or("Manual"),
            seconds / 60,
            seconds % 60
        ),
        SessionState::Stopped { .. } => "Stopped · notes start soon".into(),
        SessionState::Finishing { .. } => "Writing notes".into(),
        _ => "Motet · ready for calls".into(),
    };
    if previous.2 != label {
        if let Some(header) = &previous.1 {
            header.set_text(&label).map_err(|e| e.to_string())?;
        }
        previous.2 = label;
    }
    Ok(())
}
pub fn icon(recording: bool) -> tauri::image::Image<'static> {
    let _ = recording;
    tauri::image::Image::new_owned(include_bytes!("../icons/tray.rgba").to_vec(), 32, 32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_action_for_the_current_state_is_offered() {
        assert_eq!(action(&SessionState::Idle, false), Action::Record);
        assert_eq!(action(&SessionState::Recording, false), Action::Stop);
        assert_eq!(
            action(
                &SessionState::Stopped {
                    resume_until_ms: 10_000
                },
                false
            ),
            Action::Resume
        );
        assert_eq!(
            action(
                &SessionState::Finishing {
                    meeting_id: "1".into()
                },
                false
            ),
            Action::None
        );
        assert_eq!(action(&SessionState::Idle, true), Action::None);
        assert_eq!(
            action(
                &SessionState::Asking {
                    call_id: "1".into(),
                    app: "Call".into()
                },
                false
            ),
            Action::None
        );
    }
}

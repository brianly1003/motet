//! Native recording actions follow session state; elapsed-time ticks do not rebuild the menu.
use motet_core::events::SessionState;
use std::sync::Mutex;
use tauri::{
    Manager,
    menu::{Menu, MenuBuilder},
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
pub struct State(Mutex<Action>);
impl Default for State {
    fn default() -> Self {
        Self(Mutex::new(Action::Record))
    }
}
fn menu(app: &tauri::AppHandle, action: Action) -> tauri::Result<Menu<tauri::Wry>> {
    let builder = MenuBuilder::new(app).text("open", "Open Motet");
    let builder = match action {
        Action::Record => builder.text("record", "Record now"),
        Action::Stop => builder.text("stop", "Stop recording"),
        Action::Resume => builder.text("resume", "Resume recording"),
        Action::None => builder,
    };
    builder.separator().text("quit", "Quit Motet").build()
}
pub fn initial(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    menu(app, Action::Record)
}
pub fn update(app: &tauri::AppHandle, state: &SessionState, importing: bool) -> Result<(), String> {
    let managed = app.state::<State>();
    let mut previous = managed
        .0
        .lock()
        .map_err(|_| "Tray menu state is unavailable")?;
    let next = action(state, importing);
    if *previous != next {
        let tray = app.tray_by_id("motet").ok_or("Tray menu is unavailable")?;
        tray.set_menu(Some(menu(app, next).map_err(|e| e.to_string())?))
            .map_err(|e| e.to_string())?;
        tray.set_icon(Some(icon(next == Action::Stop)))
            .map_err(|e| e.to_string())?;
        tray.set_icon_as_template(next != Action::Stop)
            .map_err(|e| e.to_string())?;
        *previous = next;
    }
    Ok(())
}
pub fn icon(recording: bool) -> tauri::image::Image<'static> {
    let mut rgba = vec![0_u8; 32 * 32 * 4];
    for y in 0_i32..32 {
        for x in 0_i32..32 {
            let filled = if recording {
                (x - 16).pow(2) + (y - 16).pow(2) <= 100
            } else {
                (6..26).contains(&y) && ((9..13).contains(&x) || (19..23).contains(&x))
            };
            if filled {
                let i = ((y * 32 + x) * 4) as usize;
                rgba[i..i + 4].copy_from_slice(if recording {
                    &[208, 55, 43, 255]
                } else {
                    &[0, 0, 0, 255]
                });
            }
        }
    }
    tauri::image::Image::new_owned(rgba, 32, 32)
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

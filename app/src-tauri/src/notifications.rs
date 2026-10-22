//! Local OS notices with persistent in-app fallbacks and meeting-scoped actions.
use crate::{controller::Action as SessionAction, desktop};
use motetto_core::events::{AppState, Event, Problem, SessionState};
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use tauri_plugin_notification::{Action, ActionType, NotificationExt};

#[derive(Default)]
pub struct State(Mutex<Option<u64>>);

pub fn setup(app: &tauri::AppHandle) -> Result<(), String> {
    app.manage(State::default());
    app.notification()
        .register_action_types(vec![
            ActionType::builder("recording")
                .actions(vec![Action::builder("stop", "Stop").build()])
                .build(),
        ])
        .map_err(|e| e.to_string())?;
    let handle = app.clone();
    app.notification()
        .on_action(move |event| {
            if event.action_id() == "stop" {
                if let Some(id) = event
                    .notification()
                    .and_then(|n| n.extra().get("meeting_id"))
                    .and_then(|v| v.as_str())
                {
                    desktop::dispatch(handle.clone(), SessionAction::StopMeeting(id.into()));
                }
            } else if event.action_id() == "tap" {
                // The library remains a fallback when the platform cannot deliver action buttons.
                desktop::show(&handle);
                if let Some(id) = event
                    .notification()
                    .and_then(|n| n.extra().get("meeting_id"))
                    .and_then(|v| v.as_str())
                {
                    let _ = handle.emit("motetto://open-meeting", id);
                }
            }
        })
        .map_err(|e| e.to_string())
}

pub fn send(app: &tauri::AppHandle, title: &str, body: &str) {
    // Delivery is asynchronous and Focus can suppress it; never use success as recording consent.
    if !quiet(app, title, body) {
        desktop::show(app);
    }
}
/// A notification with no window fallback; `false` when the OS refused it.
pub fn quiet(app: &tauri::AppHandle, title: &str, body: &str) -> bool {
    let result = app.notification().builder().title(title).body(body).show();
    if let Err(error) = &result {
        eprintln!("Motetto notification: {error}");
    }
    result.is_ok()
}

pub fn event(app: &tauri::AppHandle, event: &Event) {
    match event {
        Event::NotesReady {
            meeting_id, title, ..
        } => {
            if let Err(error) = app
                .notification()
                .builder()
                .title(title)
                .body("Your meeting notes are ready. Open Motetto to read them.")
                .extra("meeting_id", meeting_id)
                .show()
            {
                eprintln!("Motetto notification: {error}");
                desktop::show(app);
            }
        }
        Event::StateChanged(state) => problem(app, state),
        _ => {}
    }
}
fn problem(app: &tauri::AppHandle, state: &AppState) {
    let managed = app.state::<State>();
    if let Ok(mut previous) = managed.0.lock() {
        let next = state.problem.as_ref().map(|p| p.id);
        // A start blocker while idle was already told by its own notice: the library for a
        // pressed Record, a quiet `cant_record` for a detected call (no window mid-call).
        let blocker = state.state == SessionState::Idle
            && state.problem.as_ref().is_some_and(|p| {
                matches!(
                    p.problem,
                    Problem::DiskFull
                        | Problem::PermissionMissing { .. }
                        | Problem::ModelMissing { .. }
                )
            });
        if next != *previous && next.is_some() && !blocker {
            send(
                app,
                "Motetto needs attention",
                "Open Motetto to review the problem and your saved recording.",
            );
            desktop::show(app);
        }
        *previous = next;
    }
}

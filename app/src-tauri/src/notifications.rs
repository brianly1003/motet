//! Local OS notices with persistent in-app fallbacks and meeting-scoped actions.
use crate::{
    controller::{Action as SessionAction, Controller},
    desktop,
};
use motet_core::events::{AppState, Event};
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
                    let _ = handle.emit("motet://open-meeting", id);
                }
            }
        })
        .map_err(|e| e.to_string())
}

pub fn send(app: &tauri::AppHandle, title: &str, body: &str) {
    // Delivery is asynchronous and Focus can suppress it; never use success as recording consent.
    if let Err(error) = app.notification().builder().title(title).body(body).show() {
        eprintln!("Motet notification: {error}");
        desktop::show(app);
    }
}

pub fn recording(app: &tauri::AppHandle) {
    if let Ok(state) = app.state::<Controller>().state() {
        let mut notice = app
            .notification()
            .builder()
            .title("Recording this call")
            .body("Recording microphone and system audio. Tell the people on the call.")
            .action_type_id("recording");
        if let Some(id) = state.meeting_id {
            notice = notice.extra("meeting_id", id);
        }
        if let Err(error) = notice.show() {
            eprintln!("Motet notification: {error}");
        }
    }
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
                .body("Your meeting notes are ready. Open Motet to read them.")
                .extra("meeting_id", meeting_id)
                .show()
            {
                eprintln!("Motet notification: {error}");
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
        if next != *previous && next.is_some() {
            send(
                app,
                "Motet needs attention",
                "Open Motet to review the problem and your saved recording.",
            );
            desktop::show(app);
        }
        *previous = next;
    }
}

//! One persistent recording window; Rust owns visibility and size.
use motet_core::events::{AppState, CompanionSize};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

#[derive(Default)]
pub struct State {
    ready: AtomicBool,
    previous: Mutex<Option<(bool, CompanionSize)>>,
}

pub fn create(app: &tauri::AppHandle) -> tauri::Result<()> {
    app.manage(State::default());
    let window = WebviewWindowBuilder::new(app, "companion", WebviewUrl::App("companion/".into()))
        .title("Motet recording controls")
        .inner_size(420.0, 46.0)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .content_protected(true)
        .focused(false)
        .visible(false)
        .on_page_load(|webview, _| {
            webview
                .app_handle()
                .state::<State>()
                .ready
                .store(false, Ordering::SeqCst);
            if webview
                .app_handle()
                .state::<crate::controller::Controller>()
                .state()
                .is_ok_and(|s| s.companion.visible)
            {
                crate::desktop::show(webview.app_handle());
            }
        })
        .build()?;
    crate::window_native::configure(&window)?;
    if let Some(monitor) = window.primary_monitor()? {
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        window.set_position(PhysicalPosition::new(
            area.position.x + area.size.width as i32 - (436.0 * scale) as i32,
            area.position.y + (16.0 * scale) as i32,
        ))?;
    }
    Ok(())
}

pub fn ready(app: &tauri::AppHandle) {
    app.state::<State>().ready.store(true, Ordering::SeqCst);
}

pub fn available(app: &tauri::AppHandle) -> bool {
    app.state::<State>().ready.load(Ordering::SeqCst)
        && app
            .get_webview_window("companion")
            .is_some_and(|w| w.is_visible().unwrap_or(false))
}

pub fn update(app: &tauri::AppHandle, state: &AppState) -> Result<(), String> {
    let managed = app.state::<State>();
    let mut previous = managed
        .previous
        .lock()
        .map_err(|_| "Companion state is unavailable")?;
    let next = (state.companion.visible, state.companion.size);
    if *previous == Some(next) {
        return Ok(());
    }
    let window = app
        .get_webview_window("companion")
        .ok_or("Recording controls are unavailable")?;
    let (width, height) = if next.1 == CompanionSize::Dot {
        (40.0, 40.0)
    } else {
        (420.0, 46.0)
    };
    let result = (|| -> tauri::Result<()> {
        let position = window.outer_position()?;
        let old = window.outer_size()?;
        let scale = window.scale_factor()?;
        window.set_size(LogicalSize::new(width, height))?;
        window.set_position(PhysicalPosition::new(
            position.x + old.width as i32 - (width * scale) as i32,
            position.y,
        ))?;
        if next.0 {
            window.show()?;
        } else {
            window.hide()?;
        }
        Ok(())
    })();
    result.map_err(|e| format!("Couldn't display recording controls: {e}"))?;
    *previous = Some(next);
    Ok(())
}

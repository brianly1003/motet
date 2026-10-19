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
    previous: Mutex<Option<(bool, CompanionSize, bool)>>,
}

pub fn create(app: &tauri::AppHandle) -> tauri::Result<()> {
    app.manage(State::default());
    let window = WebviewWindowBuilder::new(app, "companion", WebviewUrl::App("companion/".into()))
        .title("Motet recording controls")
        .inner_size(420.0, 72.0)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .content_protected(!motet_core::config::allow_screenshots())
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
    let next = (
        state.companion.visible,
        state.companion.size,
        state.quit_pending || state.state != motet_core::events::SessionState::Recording,
    );
    if *previous == Some(next) {
        return Ok(());
    }
    let window = app
        .get_webview_window("companion")
        .ok_or("Recording controls are unavailable")?;
    let (width, height) = dimensions(
        next.1,
        !matches!(state.state, motet_core::events::SessionState::Recording) || state.quit_pending,
    );
    let result = (|| -> tauri::Result<()> {
        resize(&window, width, height)?;
        if next.1 == CompanionSize::Tucked && !next.2 {
            snap_to_edge(&window)?;
        }
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

fn resize(window: &tauri::WebviewWindow, width: f64, height: f64) -> tauri::Result<()> {
    let position = window.outer_position()?;
    let old = window.outer_size()?;
    let scale = window.scale_factor()?;
    let mut height = height;
    let mut x = position.x + old.width as i32 - (width * scale) as i32;
    let mut y = position.y;
    if let Some(monitor) = window.current_monitor()? {
        let area = monitor.work_area();
        height = height.min(f64::from(area.size.height) / scale - 32.0);
        let left = area.position.x + (10.0 * scale) as i32;
        let top = area.position.y + (16.0 * scale) as i32;
        let right = area.position.x + area.size.width as i32 - ((width + 10.0) * scale) as i32;
        let bottom = area.position.y + area.size.height as i32 - ((height + 16.0) * scale) as i32;
        x = x.clamp(left, right.max(left));
        y = y.clamp(top, bottom.max(top));
    }
    if !height.is_finite() {
        height = 560.0; // no monitor reported: the previous fixed Sidecar height
    }
    window.set_size(LogicalSize::new(width, height))?;
    window.set_position(PhysicalPosition::new(x, y))
}

/// Tucked floats 10 px in from the nearer side edge, at the height it was left (DESIGN 4.3).
/// Rectangles are physical pixels: (x, y, width, height); `scale` converts the 10/16 px margins.
fn edge_position(
    area: (i32, i32, i32, i32),
    window: (i32, i32, i32, i32),
    scale: f64,
) -> (i32, i32) {
    let (ax, ay, aw, ah) = area;
    let (x, y, w, h) = window;
    let side = (10.0 * scale) as i32;
    let edge = (16.0 * scale) as i32;
    let left = x + w / 2 < ax + aw / 2;
    let x = if left { ax + side } else { ax + aw - w - side };
    let top = ay + edge;
    (x, y.clamp(top, (ay + ah - h - edge).max(top)))
}

fn snap_to_edge(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(());
    };
    let area = monitor.work_area();
    let (position, size) = (window.outer_position()?, window.outer_size()?);
    let (x, y) = edge_position(
        (
            area.position.x,
            area.position.y,
            area.size.width as i32,
            area.size.height as i32,
        ),
        (
            position.x,
            position.y,
            size.width as i32,
            size.height as i32,
        ),
        window.scale_factor()?,
    );
    window.set_position(PhysicalPosition::new(x, y))
}

/// Called by the Tucked capsule after the user drags it (the native drag has ended).
#[tauri::command]
pub fn snap_companion(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "companion" {
        return Err("Only recording controls can be moved this way".into());
    }
    let state = window.state::<crate::controller::Controller>().state()?;
    if state.companion.size != CompanionSize::Tucked {
        return Ok(());
    }
    snap_to_edge(&window).map_err(|e| e.to_string())
}

fn dimensions(size: CompanionSize, compact: bool) -> (f64, f64) {
    if compact {
        // Stopped and the quit question show a Pill with its caption row, as in the mockup.
        return (420.0, 72.0);
    }
    match size {
        CompanionSize::Dot => (40.0, 40.0),
        CompanionSize::Tucked => (24.0, 64.0),
        // Fills the work area 16 px from top and bottom, as in the mockup (DESIGN 4.3: 40-100 %);
        // resize() clamps it. no splitter or saved height yet (SPEC sidecar_height_pct).
        CompanionSize::Sidecar => (340.0, f64::INFINITY),
        CompanionSize::Pill => (420.0, 72.0),
    }
}
#[tauri::command]
pub fn companion_menu(window: tauri::WebviewWindow, open: bool) -> Result<(), String> {
    if window.label() != "companion" {
        return Err("Only recording controls can resize this window".into());
    }
    let state = window.state::<crate::controller::Controller>().state()?;
    let (width, height) = dimensions(
        state.companion.size,
        state.quit_pending || state.state != motet_core::events::SessionState::Recording,
    );
    resize(
        &window,
        if open { width.max(420.0) } else { width },
        if open { height.max(300.0) } else { height },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pin_companion(app: tauri::AppHandle, pinned: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<crate::controller::Controller>()
            .action(crate::controller::Action::Pin(pinned))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn sharing_mode(app: tauri::AppHandle, active: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<crate::controller::Controller>()
            .action(crate::controller::Action::Sharing(active))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::edge_position;

    #[test]
    fn tucked_snaps_to_the_nearer_side_edge_and_stays_on_screen() {
        // A 1440 x 900 work area under a 25 px menu bar, at 2x: the capsule is 48 x 128 px.
        let area = (0, 50, 2880, 1750);
        // Dropped left of centre: 10 px (20 physical) in from the left, height kept.
        assert_eq!(edge_position(area, (900, 600, 48, 128), 2.0), (20, 600));
        // Right of centre: its right side 10 px in from the right edge.
        assert_eq!(
            edge_position(area, (2000, 600, 48, 128), 2.0),
            (2880 - 48 - 20, 600)
        );
        // Dragged above the menu bar or below the Dock: kept 16 px inside the work area.
        assert_eq!(edge_position(area, (2000, 0, 48, 128), 2.0).1, 50 + 32);
        assert_eq!(
            edge_position(area, (2000, 1790, 48, 128), 2.0).1,
            50 + 1750 - 128 - 32
        );
        // A second display to the left (negative x) uses its own edges.
        assert_eq!(
            edge_position((-1920, 0, 1920, 1080), (-300, 400, 24, 64), 1.0),
            (-24 - 10, 400)
        );
    }
}

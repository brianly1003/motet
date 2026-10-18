//! macOS companion placement across full-screen Spaces.
#[cfg(target_os = "macos")]
pub fn configure(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use objc2::{msg_send, runtime::AnyObject};
    let native = window.ns_window()? as *mut AnyObject;
    // SAFETY: setup calls this on AppKit's main thread; ns_window is the live NSWindow owned
    // by Tauri. Preserve its existing flags and add NSWindowCollectionBehaviorFullScreenAuxiliary.
    unsafe {
        let behavior: usize = msg_send![native, collectionBehavior];
        let _: () = msg_send![native, setCollectionBehavior: behavior | (1 << 8)];
    }
    Ok(())
}
#[cfg(not(target_os = "macos"))]
pub fn configure(_window: &tauri::WebviewWindow) -> tauri::Result<()> {
    Ok(())
}

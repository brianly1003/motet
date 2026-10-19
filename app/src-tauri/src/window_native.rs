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

/// Keep the logo a native template image while the recording dot and timer use recording red.
#[cfg(target_os = "macos")]
pub fn tray_title(tray: &tauri::tray::TrayIcon, title: String) -> tauri::Result<()> {
    use objc2::{class, msg_send, runtime::AnyObject};
    use objc2_foundation::NSString;
    tray.with_inner_tray_icon(move |inner| {
        let Some(item) = inner.ns_status_item() else {
            return;
        };
        let Some(main_thread) = objc2::MainThreadMarker::new() else {
            return;
        };
        let Some(button) = item.button(main_thread) else {
            return;
        };
        let title = NSString::from_str(&title);
        let key = NSString::from_str("NSColor");
        // SAFETY: with_inner_tray_icon runs on AppKit's main thread. The live status button
        // retains its attributed title; release our owned string after assigning it.
        unsafe {
            let color: *mut AnyObject = msg_send![class!(NSColor), systemRedColor];
            let attributes: *mut AnyObject =
                msg_send![class!(NSDictionary), dictionaryWithObject: color, forKey: &*key];
            let value: *mut AnyObject = msg_send![class!(NSAttributedString), alloc];
            let value: *mut AnyObject =
                msg_send![value, initWithString: &*title, attributes: attributes];
            let _: () = msg_send![&*button, setAttributedTitle: value];
            let _: () = msg_send![value, release];
        }
    })
}
#[cfg(not(target_os = "macos"))]
pub fn tray_title(tray: &tauri::tray::TrayIcon, title: String) -> tauri::Result<()> {
    tray.set_title(Some(title))
}

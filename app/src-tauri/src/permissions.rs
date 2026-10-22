//! Native permission queries; unsafe Objective-C calls stay behind this boundary.
/// `request` shows the macOS prompts for undecided access; `false` only queries.
#[cfg(target_os = "macos")]
pub fn audio(request: bool) -> Result<(bool, bool), String> {
    use objc2::{
        msg_send,
        runtime::{AnyClass, AnyObject, Bool},
    };
    #[link(name = "AVFoundation", kind = "framework")]
    unsafe extern "C" {
        static AVMediaTypeAudio: *const AnyObject;
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }
    let device =
        AnyClass::get(c"AVCaptureDevice").ok_or("Microphone permission API is unavailable")?;
    // SAFETY: AVCaptureDevice class methods and the AVMediaTypeAudio constant are supplied
    // by the linked AVFoundation framework; NSInteger is isize and BOOL uses objc2::Bool.
    unsafe {
        let status: isize = msg_send![device, authorizationStatusForMediaType: AVMediaTypeAudio];
        if request && status == 0 {
            let callback = block2::RcBlock::new(|_: Bool| {});
            let _: () = msg_send![device, requestAccessForMediaType: AVMediaTypeAudio, completionHandler: &*callback];
        }
        let screen = CGPreflightScreenCaptureAccess();
        if request && !screen {
            CGRequestScreenCaptureAccess();
        }
        // Prompting is not permission. A later user start/resume performs fresh checks.
        Ok((status == 3, screen))
    }
}
#[cfg(not(target_os = "macos"))]
pub fn audio(_request: bool) -> Result<(bool, bool), String> {
    Err("Desktop recording permission checks are currently implemented for macOS only".into())
}

/// The plugin's desktop permission method always returns Granted; query macOS itself.
#[cfg(target_os = "macos")]
pub fn notifications(request: bool) -> Result<bool, String> {
    use objc2::{
        msg_send,
        runtime::{AnyClass, AnyObject, Bool},
    };
    use objc2_foundation::NSBundle;
    use std::{sync::mpsc, time::Duration};
    #[link(name = "UserNotifications", kind = "framework")]
    unsafe extern "C" {}
    // UNUserNotificationCenter requires an application bundle, not a raw cargo binary.
    if !NSBundle::mainBundle()
        .bundlePath()
        .to_string()
        .ends_with(".app")
    {
        return Err("Notification permission requires the packaged Motetto.app. Recording controls remain visible in development.".into());
    }
    let class =
        AnyClass::get(c"UNUserNotificationCenter").ok_or("Notifications are unavailable")?;
    let (tx, rx) = mpsc::channel();
    // SAFETY: UserNotifications owns the center and retains the completion blocks. Callback
    // objects are used only for the callback's duration; enum/options values are NSUInteger.
    unsafe {
        let center: *mut AnyObject = msg_send![class, currentNotificationCenter];
        if request {
            let callback = block2::RcBlock::new(move |granted: Bool, error: *mut AnyObject| {
                let _ = tx.send(if error.is_null() { Ok(granted.as_bool()) } else { Err("macOS couldn't authorize notifications. Check System Settings > Notifications > Motetto.".to_string()) });
            });
            let _: () = msg_send![center, requestAuthorizationWithOptions: 6_usize, completionHandler: &*callback];
        } else {
            let callback = block2::RcBlock::new(move |settings: *mut AnyObject| {
                let status: isize = msg_send![settings, authorizationStatus];
                let _ = tx.send(Ok(matches!(status, 2..=4)));
            });
            let _: () = msg_send![center, getNotificationSettingsWithCompletionHandler: &*callback];
        }
    }
    let result = rx
        .recv_timeout(Duration::from_secs(if request { 60 } else { 3 }))
        .map_err(|_| "Notification permission check timed out. Retry in Motetto.".to_string())?;
    // An ad-hoc signed bundle can fail the request even when System Settings allows Motetto,
    // so trust the current setting before reporting the error.
    match result {
        Err(error) if request => match notifications(false) {
            Ok(true) => Ok(true),
            _ => Err(error),
        },
        result => result,
    }
}
#[cfg(not(target_os = "macos"))]
pub fn notifications(_request: bool) -> Result<bool, String> {
    Ok(false)
}

/// SPEC 4.3: the OS pane where the user turns `kind` back on.
fn pane(kind: motetto_core::events::PermissionKind) -> &'static str {
    use motetto_core::events::PermissionKind::*;
    if cfg!(windows) {
        match kind {
            Microphone => "ms-settings:privacy-microphone",
            Notifications => "ms-settings:notifications",
            SystemAudio => "ms-settings:privacy", // Windows never blocks loopback capture.
        }
    } else {
        match kind {
            Microphone => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
            }
            SystemAudio => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
            }
            Notifications => "x-apple.systempreferences:com.apple.Notifications-Settings.extension",
        }
    }
}

#[tauri::command]
pub fn open_permission_settings(kind: motetto_core::events::PermissionKind) -> Result<(), String> {
    let opener = if cfg!(windows) { "explorer" } else { "open" };
    std::process::Command::new(opener)
        .arg(pane(kind))
        .status()
        .map(|_| ())
        .map_err(|e| format!("Couldn't open settings: {e}"))
}

#[cfg(test)]
mod tests {
    use motetto_core::events::PermissionKind;
    #[test]
    fn each_permission_opens_its_own_pane() {
        let panes = [
            PermissionKind::Microphone,
            PermissionKind::SystemAudio,
            PermissionKind::Notifications,
        ]
        .map(super::pane);
        assert!(panes[0] != panes[1] && panes[1] != panes[2] && panes[0] != panes[2]);
        assert!(panes.iter().all(|p| p.starts_with(if cfg!(windows) {
            "ms-settings:"
        } else {
            "x-apple.systempreferences:"
        })));
    }
}

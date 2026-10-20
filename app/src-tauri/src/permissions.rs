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
        return Err("Notification permission requires the packaged Motet.app. Recording controls remain visible in development.".into());
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
                let _ = tx.send(if error.is_null() { Ok(granted.as_bool()) } else { Err("macOS couldn't authorize notifications. Check System Settings > Notifications > Motet.".to_string()) });
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
        .map_err(|_| "Notification permission check timed out. Retry in Motet.".to_string())?;
    // An ad-hoc signed bundle can fail the request even when System Settings allows Motet,
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

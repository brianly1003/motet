//! macOS microphones: the input devices Core Audio knows, how they connect, their rate and input
//! volume, and setting that volume. All `unsafe` for the microphone picker lives here.

use crate::mics::{Mic, Transport};
use core_foundation::{
    base::TCFType,
    string::{CFString, CFStringRef},
};
use std::{ffi::c_void, mem::size_of, ptr};

const fn fourcc(code: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*code)
}

const SYSTEM_OBJECT: u32 = 1; // kAudioObjectSystemObject
const SCOPE_GLOBAL: u32 = fourcc(b"glob");
const SCOPE_INPUT: u32 = fourcc(b"inpt");
const DEVICES: u32 = fourcc(b"dev#");
const DEFAULT_INPUT: u32 = fourcc(b"dIn ");
const UID: u32 = fourcc(b"uid ");
const NAME: u32 = fourcc(b"lnam");
const TRANSPORT: u32 = fourcc(b"tran");
const STREAMS: u32 = fourcc(b"stm#");
const RATE: u32 = fourcc(b"nsrt");
const VOLUME: u32 = fourcc(b"volm");

#[repr(C)]
struct PropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectHasProperty(object: u32, address: *const PropertyAddress) -> u8;
    fn AudioObjectIsPropertySettable(
        object: u32,
        address: *const PropertyAddress,
        settable: *mut u8,
    ) -> i32;
    fn AudioObjectGetPropertyDataSize(
        object: u32,
        address: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
    ) -> i32;
    fn AudioObjectGetPropertyData(
        object: u32,
        address: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
        data: *mut c_void,
    ) -> i32;
    fn AudioObjectSetPropertyData(
        object: u32,
        address: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: u32,
        data: *const c_void,
    ) -> i32;
}

fn at(selector: u32, scope: u32, element: u32) -> PropertyAddress {
    PropertyAddress {
        selector,
        scope,
        element,
    }
}

fn check(call: &str, selector: u32, status: i32) -> Result<(), String> {
    let code = String::from_utf8_lossy(&selector.to_be_bytes()).into_owned();
    match status {
        0 => Ok(()),
        _ => Err(format!("{call}('{code}') failed with OSStatus {status}")),
    }
}

/// A fixed-size property (`T` is a plain number or a CF reference).
fn get<T: Copy + Default>(object: u32, addr: &PropertyAddress) -> Result<T, String> {
    let mut value = T::default();
    let mut size = size_of::<T>() as u32;
    // SAFETY: `value` is a live `T` and `size` is its exact byte size, so Core Audio writes at
    // most `size` bytes into it; the address outlives the call.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            addr,
            0,
            ptr::null(),
            &mut size,
            (&mut value as *mut T).cast(),
        )
    };
    check("AudioObjectGetPropertyData", addr.selector, status)?;
    Ok(value)
}

fn size_of_property(object: u32, addr: &PropertyAddress) -> Result<u32, String> {
    let mut size = 0u32;
    // SAFETY: the address and `size` outlive the call; no qualifier is passed.
    let status = unsafe { AudioObjectGetPropertyDataSize(object, addr, 0, ptr::null(), &mut size) };
    check("AudioObjectGetPropertyDataSize", addr.selector, status)?;
    Ok(size)
}

fn string(object: u32, selector: u32) -> Result<String, String> {
    let raw: usize = get(object, &at(selector, SCOPE_GLOBAL, 0))?;
    if raw == 0 {
        return Err("Core Audio returned no string".into());
    }
    // SAFETY: these properties return a +1 `CFStringRef` (Copy rule); wrapping under the create
    // rule releases it exactly once.
    let s = unsafe { CFString::wrap_under_create_rule(raw as CFStringRef) };
    Ok(s.to_string())
}

fn devices() -> Result<Vec<u32>, String> {
    let addr = at(DEVICES, SCOPE_GLOBAL, 0);
    let mut ids = vec![0u32; size_of_property(SYSTEM_OBJECT, &addr)? as usize / size_of::<u32>()];
    let mut size = (ids.len() * size_of::<u32>()) as u32;
    if size == 0 {
        return Ok(ids);
    }
    // SAFETY: `ids` holds exactly `size` bytes of `u32`s; Core Audio writes at most `size`
    // bytes and reports how many it wrote.
    let status = unsafe {
        AudioObjectGetPropertyData(
            SYSTEM_OBJECT,
            &addr,
            0,
            ptr::null(),
            &mut size,
            ids.as_mut_ptr().cast(),
        )
    };
    check("AudioObjectGetPropertyData", DEVICES, status)?;
    // A device can go away between the two calls.
    ids.truncate(size as usize / size_of::<u32>());
    Ok(ids)
}

fn transport(code: u32) -> Transport {
    match &code.to_be_bytes() {
        b"bltn" => Transport::BuiltIn,
        b"blue" | b"blea" => Transport::Bluetooth,
        b"usb " => Transport::Usb,
        b"virt" | b"grup" | b"auto" => Transport::Virtual,
        _ => Transport::Other,
    }
}

/// The input volume's element: the whole device (0) if it has one, else the first channel; None
/// when the device sets its own level (most Bluetooth headsets).
fn volume_element(device: u32) -> Option<u32> {
    [0, 1].into_iter().find(|&element| {
        let addr = at(VOLUME, SCOPE_INPUT, element);
        let mut settable = 0u8;
        // SAFETY: the address and `settable` outlive both calls; neither keeps a pointer.
        unsafe {
            AudioObjectHasProperty(device, &addr) != 0
                && AudioObjectIsPropertySettable(device, &addr, &mut settable) == 0
                && settable != 0
        }
    })
}

fn mic(device: u32, default: u32) -> Option<Mic> {
    // Output-only devices have no input streams.
    if size_of_property(device, &at(STREAMS, SCOPE_INPUT, 0)).ok()? == 0 {
        return None;
    }
    let volume = volume_element(device)
        .and_then(|element| get::<f32>(device, &at(VOLUME, SCOPE_INPUT, element)).ok());
    Some(Mic {
        id: string(device, UID).ok()?,
        name: string(device, NAME).ok()?,
        transport: transport(get(device, &at(TRANSPORT, SCOPE_GLOBAL, 0)).unwrap_or(0)),
        rate_hz: get::<f64>(device, &at(RATE, SCOPE_GLOBAL, 0)).unwrap_or(0.0) as u32,
        volume,
        is_default: device == default,
    })
}

/// Every input device, in Core Audio's order. Its `id` is the device UID, which is what
/// ScreenCaptureKit takes as the microphone to capture.
pub(crate) fn list() -> Result<Vec<Mic>, String> {
    let default: u32 = get(SYSTEM_OBJECT, &at(DEFAULT_INPUT, SCOPE_GLOBAL, 0)).unwrap_or(0);
    Ok(devices()?
        .into_iter()
        .filter_map(|device| mic(device, default))
        .collect())
}

/// Sets the input volume (0.0-1.0) of the device whose UID is `id`, on every channel when the
/// device has no whole-device volume.
pub(crate) fn set_volume(id: &str, volume: f32) -> Result<(), String> {
    let device = devices()?
        .into_iter()
        .find(|&device| string(device, UID).is_ok_and(|uid| uid == id))
        .ok_or("That microphone isn't connected.")?;
    let element = volume_element(device).ok_or("This microphone sets its own level.")?;
    let elements = if element == 0 { vec![0] } else { vec![1, 2] };
    let value = volume.clamp(0.0, 1.0);
    for element in elements {
        let addr = at(VOLUME, SCOPE_INPUT, element);
        // SAFETY: `value` is a live f32 and the size passed is its exact size; Core Audio copies
        // it during the call. A mono device has no channel 2 and refuses it, which is fine.
        let status = unsafe {
            AudioObjectSetPropertyData(
                device,
                &addr,
                0,
                ptr::null(),
                size_of::<f32>() as u32,
                (&value as *const f32).cast(),
            )
        };
        if element != 2 {
            check("AudioObjectSetPropertyData", VOLUME, status)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transports_map_from_core_audio_codes() {
        assert_eq!(transport(fourcc(b"bltn")), Transport::BuiltIn);
        assert_eq!(transport(fourcc(b"blue")), Transport::Bluetooth);
        assert_eq!(transport(fourcc(b"blea")), Transport::Bluetooth);
        assert_eq!(transport(fourcc(b"usb ")), Transport::Usb);
        assert_eq!(transport(fourcc(b"virt")), Transport::Virtual);
        assert_eq!(transport(0), Transport::Other);
    }

    #[test]
    #[ignore = "needs this Mac's audio devices"]
    fn lists_input_devices() {
        let mics = list().unwrap();
        println!("{mics:#?}");
        assert!(mics.iter().any(|m| m.transport == Transport::BuiltIn));
    }
}

//! 마우스 위치와 Caps Lock 상태 — CoreGraphics FFI.

use std::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

type CGEventRef = *mut c_void;
type CGEventSourceRef = *mut c_void;
type CGEventFlags = u64;

/// kCGEventFlagMaskAlphaShift — Caps Lock.
const FLAG_ALPHA_SHIFT: CGEventFlags = 0x0001_0000;
/// kCGEventSourceStateCombinedSessionState.
const COMBINED_SESSION_STATE: i32 = 0;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreate(source: CGEventSourceRef) -> CGEventRef;
    fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
    fn CGEventSourceFlagsState(state_id: i32) -> CGEventFlags;
    fn CFRelease(cf: *const c_void);
}

/// 전역 커서 위치 (화면 좌표, top-left 원점).
pub fn cursor_location() -> Option<CGPoint> {
    unsafe {
        let event = CGEventCreate(std::ptr::null_mut());
        if event.is_null() {
            return None;
        }
        let point = CGEventGetLocation(event);
        CFRelease(event);
        Some(point)
    }
}

/// Caps Lock이 켜져 있는가 (상태 조회 — 키 입력 관찰 아님).
pub fn caps_lock_on() -> bool {
    unsafe { CGEventSourceFlagsState(COMBINED_SESSION_STATE) & FLAG_ALPHA_SHIFT != 0 }
}

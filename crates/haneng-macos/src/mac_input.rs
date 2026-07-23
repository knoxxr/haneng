//! Caps Lock 상태 — CoreGraphics FFI (상태 조회, 키 입력 관찰 아님).

type CGEventFlags = u64;

/// kCGEventFlagMaskAlphaShift — Caps Lock.
const FLAG_ALPHA_SHIFT: CGEventFlags = 0x0001_0000;
/// kCGEventSourceStateCombinedSessionState.
const COMBINED_SESSION_STATE: i32 = 0;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceFlagsState(state_id: i32) -> CGEventFlags;
}

/// Caps Lock이 켜져 있는가.
pub fn caps_lock_on() -> bool {
    unsafe { CGEventSourceFlagsState(COMBINED_SESSION_STATE) & FLAG_ALPHA_SHIFT != 0 }
}

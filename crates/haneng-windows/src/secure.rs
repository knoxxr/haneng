//! 비밀번호 필드 감지 (PLAN.md N3).
//!
//! Phase 1: Win32 Edit 컨트롤의 ES_PASSWORD 스타일만 검사한다.
//! 브라우저·현대 UI 프레임워크의 비밀번호 필드는 창 스타일로 드러나지
//! 않으므로 Phase 2에서 UI Automation(IsPassword)으로 확장한다.

use std::mem::{size_of, zeroed};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowLongW, GetWindowThreadProcessId,
    ES_PASSWORD, GUITHREADINFO, GWL_STYLE,
};

pub fn password_field_focused() -> bool {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return false;
        }
        let thread = GetWindowThreadProcessId(foreground, std::ptr::null_mut());
        let mut info: GUITHREADINFO = zeroed();
        info.cbSize = size_of::<GUITHREADINFO>() as u32;
        if GetGUIThreadInfo(thread, &mut info) == 0 || info.hwndFocus.is_null() {
            return false;
        }
        let style = GetWindowLongW(info.hwndFocus, GWL_STYLE) as u32;
        if style & (ES_PASSWORD as u32) == 0 {
            return false;
        }
        // ES_PASSWORD(0x20) 비트는 Edit 계열이 아니면 다른 스타일 의미일 수 있다.
        let mut class = [0u16; 64];
        let len = GetClassNameW(info.hwndFocus, class.as_mut_ptr(), class.len() as i32);
        let name = String::from_utf16_lossy(&class[..len.max(0) as usize]).to_ascii_lowercase();
        name.contains("edit")
    }
}

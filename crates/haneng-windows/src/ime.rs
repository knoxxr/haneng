//! 한글 IME 상태 질의.
//!
//! 포커스 컨트롤의 기본 IME 윈도우에 WM_IME_CONTROL/IMC_GETOPENSTATUS를
//! 보내 한글 모드 여부를 읽는다. 한글 IME가 없는 환경에서는 None.

use std::mem::{size_of, zeroed};
use windows_sys::Win32::UI::Input::Ime::ImmGetDefaultIMEWnd;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, SendMessageTimeoutW,
    GUITHREADINFO, SMTO_ABORTIFHUNG, WM_IME_CONTROL,
};

/// IME 열림 상태(한글 IME에서 = 한글 모드 여부).
const IMC_GETOPENSTATUS: usize = 0x0005;

/// 포커스 컨트롤의 IME 열림 상태를 질의한다. `None` = 응답 없음
/// (IME 창이 없거나 타임아웃) — 호출자는 키 관찰 추적으로 폴백한다.
///
/// SendMessageW 대신 타임아웃 버전을 쓴다: 대상 프로세스가 멈춰 있으면
/// 배지 갱신·변환 스레드까지 함께 멈추기 때문.
pub fn query_korean_mode() -> Option<bool> {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return None;
        }
        // 포커스 컨트롤 기준이 창 기준보다 정확하다 (스레드가 같으면 동일).
        let thread = GetWindowThreadProcessId(foreground, std::ptr::null_mut());
        let mut info: GUITHREADINFO = zeroed();
        info.cbSize = size_of::<GUITHREADINFO>() as u32;
        let target = if GetGUIThreadInfo(thread, &mut info) != 0 && !info.hwndFocus.is_null() {
            info.hwndFocus
        } else {
            foreground
        };
        let ime = ImmGetDefaultIMEWnd(target);
        if ime.is_null() {
            return None;
        }
        let mut result: usize = 0;
        let ok = SendMessageTimeoutW(
            ime,
            WM_IME_CONTROL,
            IMC_GETOPENSTATUS,
            0,
            SMTO_ABORTIFHUNG,
            50,
            &mut result,
        );
        if ok == 0 {
            return None;
        }
        Some(result != 0)
    }
}

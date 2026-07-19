//! 한글 IME 변환 모드 조회·전환.
//!
//! 포커스 앱의 기본 IME 윈도우에 WM_IME_CONTROL을 보내 변환 모드의
//! IME_CMODE_NATIVE 비트(한글)를 읽고 쓴다. 한글 IME가 없는(영문 자판만
//! 쓰는) 환경에서는 IME 윈도우가 없어 항상 영문 모드로 취급된다.

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::Ime::ImmGetDefaultIMEWnd;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, SendMessageW, WM_IME_CONTROL,
};

const IMC_GETCONVERSIONMODE: usize = 0x0001;
/// IME_CMODE_NATIVE — 한글 입력 모드 비트.
const CMODE_NATIVE: isize = 0x0001;

fn ime_window() -> HWND {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return std::ptr::null_mut();
        }
        ImmGetDefaultIMEWnd(foreground)
    }
}

pub fn korean_mode() -> bool {
    let ime = ime_window();
    if ime.is_null() {
        return false;
    }
    unsafe { SendMessageW(ime, WM_IME_CONTROL, IMC_GETCONVERSIONMODE, 0) & CMODE_NATIVE != 0 }
}

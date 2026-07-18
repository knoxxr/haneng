//! SendInput 기반 주입 — 백스페이스 n회 + 유니코드 텍스트 타이핑.
//!
//! 모든 주입 이벤트의 dwExtraInfo에 마커를 심어 우리 훅이 자기 이벤트를
//! 다시 소비하지 않게 한다.

use std::mem::size_of;
use std::thread::sleep;
use std::time::Duration;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    VK_BACK,
};

/// "HAEN" — 주입 이벤트 식별자 (KBDLLHOOKSTRUCT.dwExtraInfo로 확인).
pub const INJECT_MARKER: usize = 0x4841_454E;

/// 연속 합성 이벤트를 앱이 놓치지 않도록 이벤트 사이에 두는 간격.
const EVENT_GAP: Duration = Duration::from_millis(2);

fn key_input(vk: u16, scan: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: INJECT_MARKER,
            },
        },
    }
}

fn send(input: &INPUT) {
    unsafe {
        SendInput(1, input, size_of::<INPUT>() as i32);
    }
    sleep(EVENT_GAP);
}

/// 백스페이스 `backspaces`회 후 `text`를 타이핑한다.
pub fn replace_text(backspaces: usize, text: &str) {
    for _ in 0..backspaces {
        send(&key_input(VK_BACK, 0, 0));
        send(&key_input(VK_BACK, 0, KEYEVENTF_KEYUP));
    }
    for unit in text.encode_utf16() {
        send(&key_input(0, unit, KEYEVENTF_UNICODE));
        send(&key_input(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
}

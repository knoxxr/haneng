//! SendInput 기반 주입 — 백스페이스 n회 + 유니코드 텍스트 타이핑.
//!
//! 모든 주입 이벤트의 dwExtraInfo에 마커를 심어 우리 훅이 자기 이벤트를
//! 다시 소비하지 않게 한다.

use std::mem::size_of;
use std::thread::sleep;
use std::time::Duration;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
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

/// 유니코드 텍스트 타이핑 (IME를 우회해 그대로 삽입된다).
pub fn type_text(text: &str) {
    for unit in text.encode_utf16() {
        send(&key_input(0, unit, KEYEVENTF_UNICODE));
        send(&key_input(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
}

const VK_SHIFT: u16 = 0x10;
const VK_CONTROL: u16 = 0x11;
const VK_HANGUL: u16 = 0x15;
pub const VK_LEFT: u16 = 0x25;
pub const VK_RIGHT: u16 = 0x27;

/// 단일 키 탭 (down+up).
pub fn tap_key(vk: u16) {
    send(&key_input(vk, 0, 0));
    send(&key_input(vk, 0, KEYEVENTF_KEYUP));
}

/// Ctrl+Shift+Left — 커서 바로 앞의 단어를 선택한다.
/// 백스페이스 개수 계산 없이 선택 위에 타이핑하는 치환의 기반:
/// 몇 번을 실행해도 선택 범위 밖 텍스트는 지워지지 않는다.
/// 호출 전 커서가 단어 끝 글자 바로 뒤에 있어야 한다 (공백 뒤가 아니라)
/// — 공백 뒤에서 부르면 앱에 따라 선택이 줄바꿈을 넘어갈 수 있다.
pub fn select_previous_word() {
    for (vk, flags) in [
        (VK_CONTROL, 0),
        (VK_SHIFT, 0),
        (VK_LEFT, 0),
        (VK_LEFT, KEYEVENTF_KEYUP),
        (VK_SHIFT, KEYEVENTF_KEYUP),
        (VK_CONTROL, KEYEVENTF_KEYUP),
    ] {
        send(&key_input(vk, 0, flags));
    }
}

/// 한/영 키를 흉내 내 IME 모드를 전환한다 — Win11 신형 한글 IME에서도
/// 동작하는 유일하게 신뢰할 수 있는 전환 방법.
pub fn press_hangul_toggle() {
    send(&key_input(VK_HANGUL, 0, 0));
    send(&key_input(VK_HANGUL, 0, KEYEVENTF_KEYUP));
}

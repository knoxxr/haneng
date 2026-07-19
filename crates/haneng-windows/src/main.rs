//! hanengw — Windows 상주 데몬.
//!
//! WH_KEYBOARD_LL 저수준 훅으로 키를 관찰해 마지막 단어의 물리 키를
//! 버퍼링한다. Windows는 **띄어쓰기 기준 수동 변환만** 지원한다:
//!
//! - Ctrl+Shift+Space → 공백으로 확정된 마지막 단어를 반대 모드 문자로
//!   치환 + 한/영 전환. 같은 단어에서 다시 누르면 되돌아온다.
//! - 치환은 백스페이스 개수 계산 없이 **Ctrl+Shift+Left 선택 위에
//!   타이핑**한다 — 몇 번을 눌러도 선택 밖 텍스트는 지워지지 않는다.
//! - 자동 교정은 Windows에서 비활성 (Win11 신형 한글 IME가 모드 질의
//!   `WM_IME_CONTROL`에 응답하지 않아 화면 상태를 신뢰할 수 없다).
//!   IME 모드는 한/영 키(VK_HANGUL) 관찰로 추적하고, 전환도 한/영 키
//!   주입으로 한다.
//!
//! 관리자 권한 앱에는 일반 권한 훅이 닿지 않는다(알려진 Windows 제약).

// 릴리스 빌드는 콘솔 없이 트레이 상주만 한다 — 콘솔 창이 뜨면 CLI처럼
// 보이고, 사용자가 그 창을 닫는 순간 데몬이 종료된다.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod keymap;

#[cfg(windows)]
mod appinfo;
#[cfg(windows)]
mod ime;
#[cfg(windows)]
mod inject;
#[cfg(windows)]
mod secure;
#[cfg(windows)]
mod tray;

#[cfg(not(windows))]
fn main() {
    eprintln!("hanengw는 Windows 전용입니다.");
    std::process::exit(1);
}

#[cfg(windows)]
fn main() {
    win::run();
}

#[cfg(windows)]
mod win {
    use crate::inject::{self, INJECT_MARKER};
    use crate::keymap::{classify, VK_HANGUL_KEY};
    use crate::{appinfo, ime, secure};
    use haneng_core::{config, english_to_hangul_with, InjectionLock, Target, WordBuffer};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{LazyLock, Mutex};
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_SPACE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, KBDLLHOOKSTRUCT, MSG,
        WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_RBUTTONDOWN,
        WM_SYSKEYDOWN,
    };

    pub static ENABLED: AtomicBool = AtomicBool::new(true);
    static BUFFER: Mutex<WordBuffer> = Mutex::new(WordBuffer::new());
    /// 변환은 한 번에 하나 — 핫키 연타로 주입이 겹치면 텍스트가 깨진다.
    static CONVERTING: InjectionLock = InjectionLock::new();

    /// 추적 중인 IME 모드. 시작 시 레거시 IME 질의로 초기화하고(신형 IME는
    /// 응답하지 않아 영문 가정), 이후 한/영 키 관찰로 따라간다.
    static KOREAN_MODE: AtomicBool = AtomicBool::new(false);

    static CONFIG: LazyLock<config::Config> = LazyLock::new(config::load_config);

    /// 전면 앱이 disabled_apps에 해당하는가.
    fn foreground_app_disabled() -> bool {
        !CONFIG.disabled_apps.is_empty()
            && appinfo::foreground_exe_name().is_some_and(|name| CONFIG.app_disabled(&name))
    }

    fn key_down(vk: u16) -> bool {
        unsafe { GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0 }
    }

    pub fn run() {
        LazyLock::force(&CONFIG);
        KOREAN_MODE.store(ime::korean_mode(), Ordering::Relaxed);
        // 트레이 아이콘은 메시지 루프가 도는 이 스레드에서 만들어야 하며,
        // 루프가 끝날 때까지 살아 있어야 한다.
        let _tray = crate::tray::install(&ENABLED);
        unsafe {
            let keyboard =
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), std::ptr::null_mut(), 0);
            let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), std::ptr::null_mut(), 0);
            if keyboard.is_null() || mouse.is_null() {
                eprintln!("키보드/마우스 훅 설치 실패");
                std::process::exit(1);
            }
            eprintln!("hanengw 실행 중 — Ctrl+Shift+Space로 마지막 단어를 한↔영 변환합니다.");
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                DispatchMessageW(&msg);
            }
        }
    }

    unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
            let message = wparam as u32;
            if kb.dwExtraInfo != INJECT_MARKER
                && (message == WM_KEYDOWN || message == WM_SYSKEYDOWN)
                && ENABLED.load(Ordering::Relaxed)
                && on_key_down(kb.vkCode as u16)
            {
                // 핫키는 여기서 소비한다 — 앱까지 전달되면 공백 삽입이나
                // Shift+Space 한/영 토글 등 부작용을 일으킨다.
                return 1;
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
    }

    unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let message = wparam as u32;
            if matches!(message, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
                // 클릭은 포커스/커서 이동일 수 있다 → 추적 포기.
                BUFFER.lock().unwrap().clear();
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
    }

    /// true = 이 키(핫키)를 소비해야 한다.
    fn on_key_down(vk: u16) -> bool {
        // 한/영 키 관찰 → 모드 추적. (주입 이벤트는 마커로 걸러졌다)
        if vk == VK_HANGUL_KEY {
            KOREAN_MODE.fetch_xor(true, Ordering::Relaxed);
            return false;
        }
        let ctrl = key_down(VK_CONTROL);
        let shift = key_down(VK_SHIFT);
        let alt = key_down(VK_MENU);
        let win = key_down(VK_LWIN) || key_down(VK_RWIN);
        // 모디파이어 키 자체의 KeyDown은 아무 상태도 바꾸지 않는다.
        let Some(class) = classify(vk, shift) else {
            return false;
        };
        if vk == VK_SPACE && ctrl && shift && !alt && !win {
            trigger_manual_conversion();
            return true;
        }
        if ctrl || alt || win {
            // 단축키 입력은 타이핑이 아니다.
            BUFFER.lock().unwrap().clear();
            return false;
        }
        BUFFER.lock().unwrap().feed(class);
        false
    }

    /// Ctrl/Shift가 물리적으로 떼어질 때까지 대기. 시간 안에 떼어지지
    /// 않으면 false — 모디파이어가 눌린 채 주입하면 앱이 주입 문자를
    /// Ctrl+문자 단축키로 해석하므로 강행하지 않는다.
    fn wait_modifiers_released() -> bool {
        for _ in 0..150 {
            if !key_down(VK_CONTROL) && !key_down(VK_SHIFT) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// Ctrl+Shift+Space: 공백으로 확정된 마지막 단어를 반대 모드로 치환.
    ///
    /// 삭제 대신 Ctrl+Shift+Left 선택 위에 타이핑하므로 화면 글자 수를
    /// 알 필요가 없고, 반복해서 눌러도 단어 하나 범위를 벗어나지 않는다.
    /// 조합 중(preedit)인 단어는 건드리지 않는다 — 공백을 먼저 쳐야 한다.
    fn trigger_manual_conversion() {
        std::thread::spawn(|| {
            // 진행 중인 변환이 있으면 이 누름은 버린다 (연타 시 주입 중첩 방지).
            let Some(_guard) = CONVERTING.try_acquire() else {
                return;
            };
            if secure::password_field_focused() || foreground_app_disabled() {
                return;
            }
            let korean_mode = KOREAN_MODE.load(Ordering::Relaxed);
            let replacement = {
                let mut buf = BUFFER.lock().unwrap();
                let Some(Target::Committed(keys, _boundary)) = buf.target() else {
                    return;
                };
                buf.mark_converted();
                if korean_mode {
                    // 화면: 조합된 한글 → 원래 친 영문 키를 그대로.
                    keys
                } else {
                    // 화면: 영문 그대로 → 한글 조합 결과를.
                    english_to_hangul_with(CONFIG.layout, &keys)
                }
            };

            if !wait_modifiers_released() {
                return; // 모디파이어를 계속 누르고 있음 — 안전하게 포기.
            }
            // 공백·줄바꿈 불가침 치환: 커서를 경계 공백 앞으로 옮긴 뒤
            // 단어 글자만 선택해 그 위에 타이핑한다. 공백을 선택에 넣으면
            // 단어가 줄 첫머리일 때 앱에 따라 선택이 이전 줄의 줄바꿈까지
            // 넘어가 줄이 합쳐진다.
            inject::tap_key(inject::VK_LEFT);
            inject::select_previous_word();
            std::thread::sleep(Duration::from_millis(20));
            inject::type_text(&replacement);
            inject::tap_key(inject::VK_RIGHT); // 기존 공백 뒤로 복귀.
                                               // 이어서 올바른 모드로 계속 타이핑할 수 있도록 한/영 전환.
            inject::press_hangul_toggle();
            KOREAN_MODE.fetch_xor(true, Ordering::Relaxed);
        });
    }
}

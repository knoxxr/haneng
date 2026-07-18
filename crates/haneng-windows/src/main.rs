//! hanengw — Windows 상주 데몬.
//!
//! WH_KEYBOARD_LL 저수준 훅으로 키를 관찰해 마지막 단어의 물리 키를
//! 버퍼링한다.
//!
//! - **수동 변환 (Phase 1)**: Ctrl+Shift+Space → 마지막 단어를 반대 모드
//!   문자로 치환 + IME 모드 전환. 같은 단어에서 다시 누르면 되돌아온다.
//! - **자동 교정 (Phase 2)**: 단어 경계(공백·문장부호)에서 잘못된 모드로
//!   판정되면 자동 치환. 직후 백스페이스 1회로 되돌리면 그 단어를 예외
//!   사전에 학습해 다시 건드리지 않는다.
//!
//! 관리자 권한 앱에는 일반 권한 훅이 닿지 않는다(알려진 Windows 제약).

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
    use crate::keymap::classify;
    use crate::{appinfo, ime, secure};
    use haneng_core::{
        build_replace_plan, config, AutoCorrector, Detector, KeyClass, Target, WordBuffer,
    };
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    pub static AUTO: AtomicBool = AtomicBool::new(true);
    static BUFFER: Mutex<WordBuffer> = Mutex::new(WordBuffer::new());

    /// 사용자 키 입력마다 증가 — 자동 교정 태스크가 "그 사이 추가 입력이
    /// 없었는가"를 확인하는 세대 카운터 (주입 중 race 방지).
    static GENERATION: AtomicU64 = AtomicU64::new(0);

    static CONFIG: LazyLock<config::Config> = LazyLock::new(config::load_config);

    static CORRECTOR: LazyLock<Mutex<AutoCorrector>> = LazyLock::new(|| {
        let mut detector = Detector::with_layout(CONFIG.sensitivity, CONFIG.layout);
        for word in config::load_exceptions() {
            detector.record_undo(&word);
        }
        Mutex::new(AutoCorrector::new(detector))
    });

    /// 전면 앱이 disabled_apps에 해당하는가.
    fn foreground_app_disabled() -> bool {
        !CONFIG.disabled_apps.is_empty()
            && appinfo::foreground_exe_name().is_some_and(|name| CONFIG.app_disabled(&name))
    }

    /// 세벌식에서는 숫자·문장부호 키도 자모를 낸다 — 자판 기준으로 재분류.
    fn reclassify_for_layout(class: KeyClass) -> KeyClass {
        if let KeyClass::Boundary(c) = class {
            if haneng_core::layout::is_word_key(CONFIG.layout, c) {
                return KeyClass::Letter(c);
            }
        }
        class
    }

    /// 자동 교정 직후의 되돌리기 대기 상태.
    struct UndoRecord {
        remaining_backspaces: usize,
        revert: String,
        restore_korean_mode: bool,
        exception_word: String,
    }

    static PENDING_UNDO: Mutex<Option<UndoRecord>> = Mutex::new(None);

    fn key_down(vk: u16) -> bool {
        unsafe { GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0 }
    }

    pub fn run() {
        AUTO.store(CONFIG.auto, Ordering::Relaxed);
        LazyLock::force(&CORRECTOR);
        // 트레이 아이콘은 메시지 루프가 도는 이 스레드에서 만들어야 하며,
        // 루프가 끝날 때까지 살아 있어야 한다.
        let _tray = crate::tray::install(&ENABLED, &AUTO);
        unsafe {
            let keyboard =
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), std::ptr::null_mut(), 0);
            let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), std::ptr::null_mut(), 0);
            if keyboard.is_null() || mouse.is_null() {
                eprintln!("키보드/마우스 훅 설치 실패");
                std::process::exit(1);
            }
            eprintln!(
                "hanengw 실행 중 — 단어 경계 자동 교정 + Ctrl+Shift+Space 수동 변환. \
                 자동 교정 직후 백스페이스 1회로 되돌립니다."
            );
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
            {
                on_key_down(kb.vkCode as u16);
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
                *PENDING_UNDO.lock().unwrap() = None;
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
    }

    fn on_key_down(vk: u16) {
        let ctrl = key_down(VK_CONTROL);
        let shift = key_down(VK_SHIFT);
        let alt = key_down(VK_MENU);
        let win = key_down(VK_LWIN) || key_down(VK_RWIN);
        // 모디파이어 키 자체의 KeyDown은 아무 상태도 바꾸지 않는다 —
        // 여기서 Clear하면 Shift 타이핑과 Ctrl+Shift+Space 핫키가 깨진다.
        let Some(class) = classify(vk, shift).map(reclassify_for_layout) else {
            return;
        };
        GENERATION.fetch_add(1, Ordering::Relaxed);
        if vk == VK_SPACE && ctrl && shift && !alt && !win {
            *PENDING_UNDO.lock().unwrap() = None;
            trigger_manual_conversion();
            return;
        }
        if ctrl || alt || win {
            // 단축키 입력은 타이핑이 아니다.
            BUFFER.lock().unwrap().clear();
            *PENDING_UNDO.lock().unwrap() = None;
            return;
        }

        // 자동 교정 직후의 백스페이스 1회 = 되돌리기.
        if class == KeyClass::Backspace {
            if let Some(record) = PENDING_UNDO.lock().unwrap().take() {
                BUFFER.lock().unwrap().clear();
                trigger_undo(record);
                return;
            }
        }
        // 그 외 어떤 키든 되돌리기 기회는 소멸.
        *PENDING_UNDO.lock().unwrap() = None;

        let mut buf = BUFFER.lock().unwrap();
        buf.feed(class);
        if let KeyClass::Boundary(_) = class {
            if AUTO.load(Ordering::Relaxed) {
                if let Some(Target::Committed(keys, boundary)) = buf.target() {
                    drop(buf);
                    trigger_auto_correction(keys, boundary);
                }
            }
        }
    }

    /// 단어 경계 자동 교정. 경계 문자가 앱에 실제로 그려질 시간을 준 뒤,
    /// 그 사이 추가 입력이 없었을 때만 치환한다.
    fn trigger_auto_correction(keys: String, boundary: char) {
        let generation = GENERATION.load(Ordering::Relaxed);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(60));
            if GENERATION.load(Ordering::Relaxed) != generation {
                return; // 사용자가 계속 타이핑 중 — 건드리지 않는다.
            }
            if secure::password_field_focused() || foreground_app_disabled() {
                return;
            }
            let korean_mode = ime::korean_mode();
            let Some(decision) =
                CORRECTOR
                    .lock()
                    .unwrap()
                    .on_word_committed(korean_mode, &keys, boundary)
            else {
                return;
            };
            if GENERATION.load(Ordering::Relaxed) != generation {
                return;
            }
            inject::replace_text(decision.backspaces, &decision.replacement);
            ime::set_korean_mode(decision.to_korean_mode);
            *PENDING_UNDO.lock().unwrap() = Some(UndoRecord {
                remaining_backspaces: decision.replacement.chars().count() - 1,
                revert: decision.revert,
                restore_korean_mode: korean_mode,
                exception_word: decision.screen_word,
            });
        });
    }

    /// 자동 교정 되돌리기: 사용자 백스페이스가 경계를 지운 뒤 호출된다.
    fn trigger_undo(record: UndoRecord) {
        std::thread::spawn(move || {
            inject::replace_text(record.remaining_backspaces, &record.revert);
            ime::set_korean_mode(record.restore_korean_mode);
            CORRECTOR
                .lock()
                .unwrap()
                .detector_mut()
                .record_undo(&record.exception_word);
            if let Err(e) = config::append_exception(&record.exception_word) {
                eprintln!("예외 사전 저장 실패: {e}");
            }
        });
    }

    /// Ctrl+Shift+Space 수동 변환. LL 훅 콜백은 빨리 반환해야 하므로
    /// 별도 스레드에서 실행.
    fn trigger_manual_conversion() {
        std::thread::spawn(|| {
            if secure::password_field_focused() || foreground_app_disabled() {
                return;
            }
            let korean_mode = ime::korean_mode();
            let plan = {
                let mut buf = BUFFER.lock().unwrap();
                let Some(target) = buf.target() else { return };
                let plan = build_replace_plan(CONFIG.layout, korean_mode, &target);
                buf.mark_converted();
                plan
            };

            // Ctrl/Shift가 눌린 채 VK_BACK을 주입하면 앱이 Ctrl+Backspace
            // (단어 삭제)로 해석한다 → 모디파이어를 뗄 때까지 대기 (최대 1초).
            for _ in 0..100 {
                if !key_down(VK_CONTROL) && !key_down(VK_SHIFT) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            inject::replace_text(plan.backspaces, &plan.replacement);
            // 이어서 올바른 모드로 계속 타이핑할 수 있도록 IME 모드 전환.
            ime::set_korean_mode(!korean_mode);
        });
    }
}

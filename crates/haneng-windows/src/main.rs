//! hanengw — 한/영 상태 표시기 (Windows 상주 데몬).
//!
//! 마우스가 텍스트 입력(I-빔 커서) 위에 있으면 커서 옆에 현재 IME
//! 모드("한"/"A") 배지를 표시한다. 그게 전부다:
//!
//! - **키보드 후킹 없음** — 모드는 포커스 창 IME에 실시간 질의
//!   (IMC_GETOPENSTATUS)로 읽고, 마우스 이동만 관찰한다.
//! - **텍스트 조작 없음** — 아무것도 지우거나 입력하지 않는다.
//!   (변환 기능은 v0.4.0에서 제거 — 이력은 git 태그 v0.3.2 참고)
//! - 설정: `%APPDATA%\haneng\config.txt` — `hover_indicator = off`,
//!   `initial_mode = korean|english`(질의 무응답 환경의 기본값),
//!   `ime_query = off`.

// 릴리스 빌드는 콘솔 없이 트레이 상주만 한다.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
mod ime;
#[cfg(windows)]
mod indicator;
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
    use crate::indicator::Mode;
    use crate::{ime, indicator};
    use haneng_core::config;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::LazyLock;
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, MSG, WH_MOUSE_LL,
        WM_MOUSEMOVE,
    };

    /// 트레이 토글: 배지 표시 켜기/끄기.
    pub static ENABLED: AtomicBool = AtomicBool::new(true);

    /// 마지막으로 알아낸 한/영 모드 (질의 무응답일 때의 폴백 표시값).
    static LAST_KNOWN_MODE: AtomicBool = AtomicBool::new(false);

    static CONFIG: LazyLock<config::Config> = LazyLock::new(config::load_config);

    /// 현재 한/영 모드: 포커스 창 IME에 실시간 질의, 무응답이면 마지막 값.
    fn current_korean_mode() -> bool {
        if CONFIG.extra("ime_query") != Some("off") {
            if let Some(korean) = ime::query_korean_mode() {
                LAST_KNOWN_MODE.store(korean, Ordering::Relaxed);
                return korean;
            }
        }
        LAST_KNOWN_MODE.load(Ordering::Relaxed)
    }

    /// 배지에 표시할 상태. 영문이면 Caps Lock 토글 상태로 대/소문자를
    /// 구분한다 (GetKeyState의 토글 비트 — 키 입력 관찰이 아니라 상태 조회).
    fn current_mode() -> Mode {
        if current_korean_mode() {
            Mode::Korean
        } else if unsafe { GetKeyState(VK_CAPITAL as i32) } & 1 != 0 {
            Mode::EnglishUpper
        } else {
            Mode::EnglishLower
        }
    }

    pub fn run() {
        LazyLock::force(&CONFIG);
        let initial =
            ime::query_korean_mode().unwrap_or(CONFIG.extra("initial_mode") == Some("korean"));
        LAST_KNOWN_MODE.store(initial, Ordering::Relaxed);
        ENABLED.store(
            CONFIG.extra("hover_indicator") != Some("off"),
            Ordering::Relaxed,
        );

        indicator::init(current_mode);
        // 트레이 아이콘은 메시지 루프가 도는 이 스레드에서 만들어야 하며,
        // 루프가 끝날 때까지 살아 있어야 한다.
        let _tray = crate::tray::install(&ENABLED);

        unsafe {
            let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), std::ptr::null_mut(), 0);
            if mouse.is_null() {
                eprintln!("마우스 훅 설치 실패");
                std::process::exit(1);
            }
            eprintln!("hanengw 실행 중 — 입력창 위에 마우스를 올리면 한/영 상태를 표시합니다.");
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                DispatchMessageW(&msg);
            }
        }
    }

    unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 && wparam as u32 == WM_MOUSEMOVE && ENABLED.load(Ordering::Relaxed) {
            indicator::on_mouse_move();
        }
        CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
    }
}

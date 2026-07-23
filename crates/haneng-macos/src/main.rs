//! hanengd — 한/영 상태 표시기 (macOS 상주 데몬).
//!
//! 마우스가 텍스트 입력 위에 있으면 커서 옆에 현재 상태 배지를 표시한다:
//! 파랑 "한"(한글) / 회색 "a"(영문 소문자) / 주황 "A"(영문 + Caps Lock).
//!
//! - 텍스트 입력 판별: Accessibility API (요소 role). **손쉬운 사용 권한** 필요.
//! - 한/영 판별: 현재 키보드 입력 소스(TIS) 조회.
//! - **키 입력을 관찰하거나 텍스트를 조작하지 않는다** — 커서 위치·모드만 읽는다.
//! - 메뉴바 아이콘: 배지 토글 · 설정 · 종료.

#[cfg(target_os = "macos")]
mod ax;
#[cfg(target_os = "macos")]
mod badge;
#[cfg(target_os = "macos")]
mod mac_input;
#[cfg(target_os = "macos")]
mod tis;
#[cfg(target_os = "macos")]
mod tray;

/// 배지에 표시할 입력 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    EnglishLower,
    EnglishUpper,
    Korean,
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("hanengd는 macOS 전용입니다.");
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
fn main() {
    macos::run();
}

#[cfg(target_os = "macos")]
mod macos {
    use crate::badge::Badge;
    use crate::{ax, mac_input, tis, Mode};
    use haneng_core::config;
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};
    use tao::event::{Event, StartCause};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};

    pub static ENABLED: AtomicBool = AtomicBool::new(true);

    /// 커서·모드 확인 주기.
    const POLL: Duration = Duration::from_millis(120);

    /// 마지막으로 알아낸 한/영 값 (입력 소스 조회 실패 시 폴백).
    static LAST_KOREAN: AtomicBool = AtomicBool::new(false);

    fn current_mode() -> Mode {
        let korean =
            tis::current_source_is_korean().unwrap_or_else(|| LAST_KOREAN.load(Ordering::Relaxed));
        LAST_KOREAN.store(korean, Ordering::Relaxed);
        if korean {
            Mode::Korean
        } else if mac_input::caps_lock_on() {
            Mode::EnglishUpper
        } else {
            Mode::EnglishLower
        }
    }

    pub fn run() {
        // 데몬은 하나만 — 이미 실행 중이면 종료.
        if !haneng_core::single_instance::acquire("hanengd") {
            eprintln!("hanengd가 이미 실행 중입니다.");
            return;
        }
        let cfg = config::load_config();
        ENABLED.store(
            cfg.extra("hover_indicator") != Some("off"),
            Ordering::Relaxed,
        );
        LAST_KOREAN.store(
            cfg.extra("initial_mode") == Some("korean"),
            Ordering::Relaxed,
        );
        // 시야를 가리지 않도록 배지를 반투명하게 (기본 80%).
        let opacity = cfg.badge_opacity_percent() as f64 / 100.0;

        // 손쉬운 사용 권한 요청 (없으면 텍스트 감지 불가 → 배지가 안 뜬다).
        if !ax::accessibility_trusted(true) {
            eprintln!(
                "손쉬운 사용 권한이 필요합니다 — 시스템 설정 → 개인정보 보호 및 보안 →\n\
                 손쉬운 사용에 hanengd(또는 실행 중인 터미널)를 추가하세요."
            );
        }

        let mut event_loop = EventLoopBuilder::<()>::with_user_event().build();
        // 메뉴바 전용 앱 — Dock 아이콘 없음.
        event_loop.set_activation_policy(ActivationPolicy::Accessory);

        // 배지·트레이 모두 루프가 끝날 때까지 살아 있어야 한다.
        let badge: RefCell<Option<Badge>> = RefCell::new(None);
        let tray: RefCell<Option<tray_icon::TrayIcon>> = RefCell::new(None);

        eprintln!("hanengd 실행 중 — 입력창 위에 마우스를 올리면 한/영 상태를 표시합니다.");

        event_loop.run(move |event, _target, control_flow| match event {
            Event::NewEvents(StartCause::Init) => {
                let mtm =
                    objc2::MainThreadMarker::new().expect("event loop runs on the main thread");
                *badge.borrow_mut() = Some(Badge::new(mtm, opacity));
                *tray.borrow_mut() = Some(crate::tray::install(&ENABLED));
                *control_flow = ControlFlow::WaitUntil(Instant::now() + POLL);
            }
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                tick(&badge);
                *control_flow = ControlFlow::WaitUntil(Instant::now() + POLL);
            }
            _ => {}
        });
    }

    fn tick(badge: &RefCell<Option<Badge>>) {
        let mut guard = badge.borrow_mut();
        let Some(badge) = guard.as_mut() else { return };

        if !ENABLED.load(Ordering::Relaxed) {
            badge.hide();
            return;
        }
        let Some(pos) = mac_input::cursor_location() else {
            badge.hide();
            return;
        };
        // 트리거는 마우스가 텍스트 입력 위(hover)일 때. 위치는 마우스가 아니라
        // 그 입력의 카렛. 카렛을 못 읽으면(포커스 없음 등) 숨긴다.
        if ax::text_input_at(pos.x, pos.y) {
            let mode = current_mode();
            // 카렛을 읽을 수 있으면 카렛에, 없으면(브라우저 등) 마우스 옆에.
            if let Some(rect) = ax::caret_bounds_at(pos.x, pos.y) {
                badge.show_at_caret(rect.origin.x, rect.origin.y, rect.size.height, mode);
            } else {
                badge.show_at_mouse(pos.x, pos.y, mode);
            }
        } else {
            badge.hide();
        }
    }
}

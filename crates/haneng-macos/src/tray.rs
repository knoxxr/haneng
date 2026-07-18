//! 메뉴바(트레이) UI — 전체 활성화 / 자동 교정 토글과 종료.
//!
//! macOS에서 트레이 아이콘은 메인 스레드에서, NSApp 이벤트 루프가 시작된
//! 뒤에 만들어야 한다 → tao 루프의 첫 `NewEvents(Init)`에서 생성한다.
//! tao의 이벤트 루프가 메인 CFRunLoop을 구동하므로 main.rs에서 걸어둔
//! CGEventTap 소스도 같은 루프에서 함께 돌아간다.
//!
//! 메뉴 항목(muda)은 Send가 아니라서 MenuEvent 핸들러(임의 스레드 호출
//! 가능)에 캡처할 수 없다 — 핸들러는 프록시로 이벤트를 메인 루프에
//! 넘기기만 하고, 실제 처리는 루프 클로저(메인 스레드)에서 한다.

use std::sync::atomic::{AtomicBool, Ordering};

use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

/// 데몬 실행 파일 옆의 haneng-settings를 띄운다 (없으면 조용히 무시하고 로그만).
fn open_settings() {
    let Some(path) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("haneng-settings")))
    else {
        return;
    };
    if let Err(e) = std::process::Command::new(&path).spawn() {
        eprintln!("설정 창 실행 실패 ({}): {e}", path.display());
    }
}

pub fn run_with_tray(enabled: &'static AtomicBool, auto: &'static AtomicBool) -> ! {
    let event_loop = EventLoopBuilder::<MenuEvent>::with_user_event().build();

    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = proxy.send_event(event);
    }));

    let toggle_enabled = CheckMenuItem::new(
        "한/영 교정 활성화",
        true,
        enabled.load(Ordering::Relaxed),
        None,
    );
    let toggle_auto = CheckMenuItem::new(
        "단어 경계 자동 교정",
        true,
        auto.load(Ordering::Relaxed),
        None,
    );
    let settings = MenuItem::new("설정...", true, None);
    let quit = MenuItem::new("종료", true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &toggle_enabled,
        &toggle_auto,
        &PredefinedMenuItem::separator(),
        &settings,
        &quit,
    ])
    .expect("append tray menu items");

    // Init 이벤트에서 생성해 루프가 끝날 때까지 보유해야 한다 (drop되면
    // 메뉴바에서 사라진다).
    let mut tray: Option<TrayIcon> = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) if tray.is_none() => {
                tray = Some(
                    TrayIconBuilder::new()
                        .with_menu(Box::new(menu.clone()))
                        .with_title("한A") // 아이콘 에셋 없이 텍스트 상태 항목으로 표시
                        .with_tooltip("haneng — 한/영 오타 교정 (⌘⇧Space)")
                        .build()
                        .expect("create tray icon"),
                );
            }
            Event::UserEvent(menu_event) => {
                if menu_event.id == *toggle_enabled.id() {
                    let now = !enabled.load(Ordering::Relaxed);
                    enabled.store(now, Ordering::Relaxed);
                    toggle_enabled.set_checked(now);
                } else if menu_event.id == *toggle_auto.id() {
                    let now = !auto.load(Ordering::Relaxed);
                    auto.store(now, Ordering::Relaxed);
                    toggle_auto.set_checked(now);
                } else if menu_event.id == *settings.id() {
                    open_settings();
                } else if menu_event.id == *quit.id() {
                    std::process::exit(0);
                }
            }
            _ => {}
        }
    })
}

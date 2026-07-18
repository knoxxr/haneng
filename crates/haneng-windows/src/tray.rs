//! 시스템 트레이 UI — 전체 활성화 / 자동 교정 토글과 종료.
//!
//! tray-icon은 이 스레드에 숨은 창을 만들고, 메뉴 이벤트는 우리
//! GetMessageW/DispatchMessageW 루프가 그 창의 메시지를 처리할 때 같은
//! 스레드에서 핸들러로 전달된다. muda 메뉴 항목은 Send가 아니라 핸들러
//! 클로저에 캡처할 수 없으므로, 체크 상태 갱신용 항목들은 thread_local에
//! 두고 핸들러(같은 스레드 호출 보장)에서 꺼내 쓴다.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

thread_local! {
    static CHECK_ITEMS: RefCell<Option<(CheckMenuItem, CheckMenuItem)>> =
        const { RefCell::new(None) };
}

/// 단색 16×16 아이콘 (에셋 없이 생성).
fn solid_icon() -> Icon {
    const SIZE: usize = 16;
    let mut rgba = Vec::with_capacity(SIZE * SIZE * 4);
    for _ in 0..SIZE * SIZE {
        rgba.extend_from_slice(&[0x2B, 0x6C, 0xB0, 0xFF]);
    }
    Icon::from_rgba(rgba, SIZE as u32, SIZE as u32).expect("valid rgba icon")
}

/// 데몬 실행 파일 옆의 haneng-settings.exe를 띄운다.
fn open_settings() {
    let Some(path) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("haneng-settings.exe")))
    else {
        return;
    };
    if let Err(e) = std::process::Command::new(&path).spawn() {
        eprintln!("설정 창 실행 실패 ({}): {e}", path.display());
    }
}

/// 트레이 설치. 반환된 TrayIcon은 메시지 루프가 도는 동안 보유해야 한다.
pub fn install(enabled: &'static AtomicBool, auto: &'static AtomicBool) -> TrayIcon {
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

    let ids: (MenuId, MenuId, MenuId, MenuId) = (
        toggle_enabled.id().clone(),
        toggle_auto.id().clone(),
        settings.id().clone(),
        quit.id().clone(),
    );
    CHECK_ITEMS.with(|cell| *cell.borrow_mut() = Some((toggle_enabled, toggle_auto)));

    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == ids.0 {
            let now = !enabled.load(Ordering::Relaxed);
            enabled.store(now, Ordering::Relaxed);
            CHECK_ITEMS.with(|cell| {
                if let Some((item, _)) = cell.borrow().as_ref() {
                    item.set_checked(now);
                }
            });
        } else if event.id == ids.1 {
            let now = !auto.load(Ordering::Relaxed);
            auto.store(now, Ordering::Relaxed);
            CHECK_ITEMS.with(|cell| {
                if let Some((_, item)) = cell.borrow().as_ref() {
                    item.set_checked(now);
                }
            });
        } else if event.id == ids.2 {
            open_settings();
        } else if event.id == ids.3 {
            std::process::exit(0);
        }
    }));

    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(solid_icon())
        .with_tooltip("haneng — 한/영 오타 교정 (Ctrl+Shift+Space)")
        .build()
        .expect("create tray icon")
}

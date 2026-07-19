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
    static CHECK_ITEMS: RefCell<Option<CheckMenuItem>> = const { RefCell::new(None) };
}

/// 앱 아이콘 (scripts/gen-icon.py가 생성한 원시 RGBA).
fn app_icon() -> Icon {
    const SIZE: u32 = 32;
    let rgba = include_bytes!("../../../assets/tray-32.rgba").to_vec();
    Icon::from_rgba(rgba, SIZE, SIZE).expect("valid rgba icon")
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
pub fn install(enabled: &'static AtomicBool) -> TrayIcon {
    let toggle_enabled = CheckMenuItem::new(
        "한/영 배지 표시",
        true,
        enabled.load(Ordering::Relaxed),
        None,
    );
    let settings = MenuItem::new("설정...", true, None);
    let quit = MenuItem::new("종료", true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &toggle_enabled,
        &PredefinedMenuItem::separator(),
        &settings,
        &quit,
    ])
    .expect("append tray menu items");

    let ids: (MenuId, MenuId, MenuId) = (
        toggle_enabled.id().clone(),
        settings.id().clone(),
        quit.id().clone(),
    );
    CHECK_ITEMS.with(|cell| *cell.borrow_mut() = Some(toggle_enabled));

    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == ids.0 {
            let now = !enabled.load(Ordering::Relaxed);
            enabled.store(now, Ordering::Relaxed);
            CHECK_ITEMS.with(|cell| {
                if let Some(item) = cell.borrow().as_ref() {
                    item.set_checked(now);
                }
            });
        } else if event.id == ids.1 {
            open_settings();
        } else if event.id == ids.2 {
            std::process::exit(0);
        }
    }));

    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(app_icon())
        .with_tooltip("haneng — 한/영 상태 표시기")
        .build()
        .expect("create tray icon")
}

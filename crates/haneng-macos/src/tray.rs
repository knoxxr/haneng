//! 메뉴바(트레이) UI — 배지 표시 토글 · 설정 · 종료.
//!
//! muda 메뉴 항목은 Send가 아니라 MenuEvent 핸들러에 캡처할 수 없다 →
//! thread_local에 두고 같은 스레드(메인)에서 처리한다. tray-icon은 tao
//! 이벤트 루프가 도는 메인 스레드에서 만들어야 한다.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

thread_local! {
    static TOGGLE_ITEM: RefCell<Option<CheckMenuItem>> = const { RefCell::new(None) };
}

/// 앱 아이콘 (scripts/gen-icon.py가 생성한 원시 RGBA 64×64).
fn app_icon() -> Icon {
    let rgba = include_bytes!("../../../assets/icon-64.rgba").to_vec();
    Icon::from_rgba(rgba, 64, 64).expect("valid rgba icon")
}

/// 데몬 실행 파일 옆의 haneng-settings를 띄운다.
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

pub fn install(enabled: &'static AtomicBool) -> TrayIcon {
    let toggle = CheckMenuItem::new(
        "한/영 배지 표시",
        true,
        enabled.load(Ordering::Relaxed),
        None,
    );
    let settings = MenuItem::new("설정...", true, None);
    let quit = MenuItem::new("종료", true, None);
    let menu = Menu::new();
    menu.append_items(&[&toggle, &PredefinedMenuItem::separator(), &settings, &quit])
        .expect("append tray menu items");

    let ids: (MenuId, MenuId, MenuId) = (
        toggle.id().clone(),
        settings.id().clone(),
        quit.id().clone(),
    );
    TOGGLE_ITEM.with(|cell| *cell.borrow_mut() = Some(toggle));

    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == ids.0 {
            let now = !enabled.load(Ordering::Relaxed);
            enabled.store(now, Ordering::Relaxed);
            TOGGLE_ITEM.with(|cell| {
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

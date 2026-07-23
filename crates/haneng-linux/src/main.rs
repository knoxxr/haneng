//! hanengl — 한/영 상태 표시기 (Linux X11 상주 데몬). **실험적 — 실기기 미검증.**
//!
//! 마우스가 텍스트 입력(I-빔 커서) 위에 있으면 커서 옆에 상태 배지를
//! 표시한다: 파랑 "한" / 회색 "a" / 주황 "A"(Caps Lock).
//!
//! X11 제약과 그 대응:
//! - 텍스트 입력 판별: XFixes 커서 이름("xterm"/"text" 등)으로 I-빔 감지.
//! - Caps Lock: 포인터 질의의 Lock 마스크.
//! - **한/영 모드**: X11에는 IME 상태를 묻는 표준 API가 없다. 그래서 한/영
//!   토글 키(기본 Hangul=130, 오른쪽 Alt=108; `linux_toggle_keycodes`로
//!   재정의)를 XRecord로 관찰해 모드를 *추적*한다. macOS/Windows와 달리
//!   키 이벤트를 관찰하지만, 토글 여부만 보고 내용은 보지 않는다. 시작
//!   모드는 `initial_mode = korean|english`로 지정.
//! - Wayland 미지원. 배지는 불투명 사각형(합성기 독립).
//! - **배지 위치**: Windows/macOS는 입력 카렛 위치에 배지를 붙이지만, X11에는
//!   임의 앱의 카렛 위치를 묻는 표준 API가 없다. 그래서 Linux는 (이 데몬의
//!   실험적 특성과 함께) 마우스 커서 기준 위치를 유지한다.

#[cfg(target_os = "linux")]
mod render;

/// 배지에 표시할 입력 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    EnglishLower,
    EnglishUpper,
    Korean,
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("hanengl은 Linux(X11) 전용입니다.");
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
fn main() {
    if let Err(e) = linux::run() {
        eprintln!("hanengl 종료: {e}");
        std::process::exit(1);
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use crate::render::{self, SIZE};
    use crate::Mode;
    use haneng_core::config;
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::Duration;
    use x11rb::connection::Connection;
    use x11rb::protocol::record::{self, ConnectionExt as _};
    use x11rb::protocol::xfixes::ConnectionExt as _;
    use x11rb::protocol::xproto::{
        AtomEnum, ConfigureWindowAux, ConnectionExt as _, CreateGCAux, CreateWindowAux, EventMask,
        ImageFormat, KeyButMask, PropMode, WindowClass,
    };
    use x11rb::wrapper::ConnectionExt as _;

    static KOREAN: AtomicBool = AtomicBool::new(false);

    const POLL: Duration = Duration::from_millis(120);
    const OFFSET: i32 = 18;
    /// 텍스트 입력을 뜻하는 커서 이름(테마 무관 통용).
    const TEXT_CURSORS: &[&str] = &["xterm", "text", "ibeam", "cursor-text"];

    fn current_mode(caps: bool) -> Mode {
        if KOREAN.load(Ordering::Relaxed) {
            Mode::Korean
        } else if caps {
            Mode::EnglishUpper
        } else {
            Mode::EnglishLower
        }
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        let cfg = config::load_config();
        KOREAN.store(
            cfg.extra("initial_mode") == Some("korean"),
            Ordering::Relaxed,
        );
        let show = cfg.extra("hover_indicator") != Some("off");
        let toggles: Vec<u8> = cfg
            .extra("linux_toggle_keycodes")
            .map(|v| v.split(',').filter_map(|s| s.trim().parse().ok()).collect())
            .filter(|v: &Vec<u8>| !v.is_empty())
            .unwrap_or_else(|| vec![130, 108]);

        let (conn, screen_num) = x11rb::connect(None)?;
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;
        let depth = screen.root_depth;

        // XFixes 사용 전 버전 협상 필수.
        conn.xfixes_query_version(5, 0)?.reply()?;

        // 배지용 override-redirect 창.
        let win = conn.generate_id()?;
        conn.create_window(
            depth,
            win,
            root,
            0,
            0,
            SIZE as u16,
            SIZE as u16,
            0,
            WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &CreateWindowAux::new()
                .override_redirect(1)
                .event_mask(EventMask::EXPOSURE),
        )?;
        let gc = conn.generate_id()?;
        conn.create_gc(gc, win, &CreateGCAux::new())?;

        // 반투명: 합성기(compositor)가 있으면 _NET_WM_WINDOW_OPACITY를 존중한다.
        // 없으면 불투명(무시)으로 graceful degrade.
        if let Ok(reply) = conn.intern_atom(false, b"_NET_WM_WINDOW_OPACITY")?.reply() {
            let opacity = (cfg.badge_opacity_percent() as u64 * 0xFFFF_FFFF / 100) as u32;
            let _ = conn.change_property32(
                PropMode::REPLACE,
                win,
                reply.atom,
                AtomEnum::CARDINAL,
                &[opacity],
            );
        }
        conn.flush()?;

        // 한/영 토글 키 관찰 스레드 (모드 추적).
        spawn_mode_tracker(toggles);

        let font = render::load_font_cached();
        let images: Vec<Vec<u8>> = [Mode::EnglishLower, Mode::EnglishUpper, Mode::Korean]
            .iter()
            .map(|&m| render::render(m, font.as_ref()))
            .collect();
        let image_for = |m: Mode| -> &[u8] {
            match m {
                Mode::EnglishLower => &images[0],
                Mode::EnglishUpper => &images[1],
                Mode::Korean => &images[2],
            }
        };

        eprintln!("hanengl 실행 중 — 입력창 위에 마우스를 올리면 한/영 상태를 표시합니다.");

        let mut visible = false;
        let mut shown_mode: Option<Mode> = None;
        loop {
            thread::sleep(POLL);
            if !show {
                continue;
            }
            let pointer = conn.query_pointer(root)?.reply()?;
            let caps = u16::from(pointer.mask) & u16::from(KeyButMask::LOCK) != 0;

            let over_text = cursor_is_text(&conn).unwrap_or(false);
            if !over_text {
                if visible {
                    conn.unmap_window(win)?;
                    conn.flush()?;
                    visible = false;
                    shown_mode = None;
                }
                continue;
            }

            let mode = current_mode(caps);
            conn.configure_window(
                win,
                &ConfigureWindowAux::new()
                    .x(pointer.root_x as i32 + OFFSET)
                    .y(pointer.root_y as i32 + OFFSET),
            )?;
            if !visible {
                conn.map_window(win)?;
                visible = true;
            }
            if shown_mode != Some(mode) {
                conn.put_image(
                    ImageFormat::Z_PIXMAP,
                    win,
                    gc,
                    SIZE as u16,
                    SIZE as u16,
                    0,
                    0,
                    0,
                    depth,
                    image_for(mode),
                )?;
                shown_mode = Some(mode);
            }
            conn.flush()?;
        }
    }

    /// 현재 커서 이름이 텍스트 입력용인가 (XFixes).
    fn cursor_is_text(conn: &impl Connection) -> Result<bool, Box<dyn Error>> {
        let reply = conn.xfixes_get_cursor_image_and_name()?.reply()?;
        if reply.cursor_atom == x11rb::NONE {
            return Ok(false);
        }
        let name = conn.get_atom_name(reply.cursor_atom)?.reply()?;
        let name = String::from_utf8_lossy(&name.name).to_ascii_lowercase();
        Ok(TEXT_CURSORS.iter().any(|t| name.contains(t)))
    }

    /// XRecord로 한/영 토글 키를 관찰해 KOREAN을 뒤집는다.
    fn spawn_mode_tracker(toggles: Vec<u8>) {
        thread::spawn(move || {
            if let Err(e) = track(&toggles) {
                eprintln!("모드 추적 중단(한/영 표시가 고정될 수 있음): {e}");
            }
        });
    }

    fn track(toggles: &[u8]) -> Result<(), Box<dyn Error>> {
        let (ctrl, _) = x11rb::connect(None)?;
        let (data, _) = x11rb::connect(None)?;
        let context = ctrl.generate_id()?;
        let range = record::Range {
            device_events: record::Range8 { first: 2, last: 2 }, // KeyPress
            ..Default::default()
        };
        ctrl.record_create_context(context, 0, &[record::CS::ALL_CLIENTS.into()], &[range])?
            .check()?;
        ctrl.flush()?;
        let mut cookie = data.record_enable_context(context)?;
        loop {
            let reply = cookie.next().ok_or("XRecord 스트림 종료")??;
            for raw in reply.data.chunks_exact(32) {
                if raw[0] & 0x7F == 2 && toggles.contains(&raw[1]) {
                    KOREAN.fetch_xor(true, Ordering::Relaxed);
                }
            }
        }
    }
}

//! hanengl — Linux X11 상주 데몬.
//!
//! XRecord로 키를 관찰해 마지막 단어의 물리 키를 버퍼링하고, XTest로
//! 치환을 주입한다. 파이프라인(수동 핫키 Ctrl+Shift+Space, 단어 경계 자동
//! 교정, 백스페이스 1회 undo + 예외 학습)은 macOS/Windows 데몬과 동일.
//!
//! X11 고유 제약:
//! - **IME 모드 조회 API가 없다** — ibus/fcitx의 한/영 상태는 D-Bus로
//!   노출되지 않는다. 대신 한/영 토글 키(기본: Hangul=130, 오른쪽 Alt=108;
//!   `linux_toggle_keycodes`로 재정의)를 관찰해 모드를 *추적*하고, 시작
//!   시점은 영문 모드로 가정한다. 어긋나면 토글 키를 한 번 눌러 동기화.
//! - **비밀번호 필드 감지가 없다** — AT-SPI 연동 전까지의 공백(Phase 3
//!   후속). 민감한 환경에서는 트레이/설정으로 끄는 것을 안내해야 한다.
//! - Wayland에서는 전역 관찰이 불가능하다 — Fcitx5/IBus 플러그인 트랙이
//!   담당한다(PLAN.md).

// Linux 밖에서는 호출부가 컴파일되지 않지만 테스트를 위해 유지한다.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod keymap;

#[cfg(target_os = "linux")]
mod inject;

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
    use crate::inject::{self, INJECTING};
    use crate::keymap::{
        classify, CONTROL_MASK, DEFAULT_TOGGLE_KEYCODES, MOD1_MASK, MOD4_MASK, SHIFT_MASK,
        SPACE_KEYCODE,
    };
    use haneng_core::{
        build_replace_plan, config, AutoCorrector, Detector, InjectionLock, KeyClass, Target,
        WordBuffer,
    };
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, LazyLock, Mutex, OnceLock};
    use std::time::Duration;
    use x11rb::connection::Connection;
    use x11rb::protocol::record::{self, ConnectionExt as _};
    use x11rb::rust_connection::RustConnection;

    pub static ENABLED: AtomicBool = AtomicBool::new(true);
    pub static AUTO: AtomicBool = AtomicBool::new(true);
    /// 추적 중인 IME 모드 (시작은 영문 가정 — X11에는 조회 API가 없다).
    static KOREAN_MODE: AtomicBool = AtomicBool::new(false);
    static BUFFER: Mutex<WordBuffer> = Mutex::new(WordBuffer::new());
    static GENERATION: AtomicU64 = AtomicU64::new(0);
    /// 주입은 한 번에 하나 — 핫키 연타 등으로 겹치면 텍스트가 깨진다.
    static CONVERTING: InjectionLock = InjectionLock::new();
    /// 주입용 연결 (관찰용 연결과 분리).
    static INJECT_CONN: OnceLock<Arc<RustConnection>> = OnceLock::new();
    static TOGGLE_KEYCODES: OnceLock<Vec<u8>> = OnceLock::new();

    static CONFIG: LazyLock<config::Config> = LazyLock::new(config::load_config);

    static CORRECTOR: LazyLock<Mutex<AutoCorrector>> = LazyLock::new(|| {
        let mut detector = Detector::with_layout(CONFIG.sensitivity, CONFIG.layout);
        for word in config::load_exceptions() {
            detector.record_undo(&word);
        }
        Mutex::new(AutoCorrector::new(detector))
    });

    /// 활성 창의 WM_CLASS가 disabled_apps에 해당하는가.
    fn active_app_disabled() -> bool {
        if CONFIG.disabled_apps.is_empty() {
            return false;
        }
        active_window_class().is_some_and(|class| CONFIG.app_disabled(&class))
    }

    /// _NET_ACTIVE_WINDOW → WM_CLASS ("instance\0class\0").
    fn active_window_class() -> Option<String> {
        use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};
        let conn = INJECT_CONN.get()?;
        let root = conn.setup().roots.first()?.root;
        let net_active = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")
            .ok()?
            .reply()
            .ok()?
            .atom;
        let active = conn
            .get_property(false, root, net_active, AtomEnum::WINDOW, 0, 1)
            .ok()?
            .reply()
            .ok()?;
        let window = active.value32()?.next()?;
        let class = conn
            .get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 64)
            .ok()?
            .reply()
            .ok()?;
        Some(String::from_utf8_lossy(&class.value).replace('\0', " "))
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

    struct UndoRecord {
        remaining_backspaces: usize,
        revert: String,
        restore_korean_mode: bool,
        exception_word: String,
    }

    static PENDING_UNDO: Mutex<Option<UndoRecord>> = Mutex::new(None);

    pub fn run() -> Result<(), Box<dyn Error>> {
        AUTO.store(CONFIG.auto, Ordering::Relaxed);
        LazyLock::force(&CORRECTOR);
        let toggles: Vec<u8> = CONFIG
            .extra("linux_toggle_keycodes")
            .map(|v| v.split(',').filter_map(|s| s.trim().parse().ok()).collect())
            .filter(|v: &Vec<u8>| !v.is_empty())
            .unwrap_or_else(|| DEFAULT_TOGGLE_KEYCODES.to_vec());
        TOGGLE_KEYCODES.set(toggles).expect("set once");

        let (inject_conn, _) = RustConnection::connect(None)?;
        INJECT_CONN
            .set(Arc::new(inject_conn))
            .map_err(|_| "already initialized")?;

        // XRecord는 제어/데이터 연결을 분리해야 한다.
        let (ctrl_conn, _) = RustConnection::connect(None)?;
        let (data_conn, _) = RustConnection::connect(None)?;

        let context = ctrl_conn.generate_id()?;
        let range = record::Range {
            device_events: record::Range8 { first: 2, last: 4 }, // KeyPress..ButtonPress
            ..Default::default()
        };
        ctrl_conn
            .record_create_context(context, 0, &[record::CS::ALL_CLIENTS.into()], &[range])?
            .check()?;
        ctrl_conn.flush()?;

        eprintln!(
            "hanengl 실행 중 — 단어 경계 자동 교정 + Ctrl+Shift+Space 수동 변환. \
             시작 모드는 영문으로 가정합니다 (한/영 키 관찰로 추적)."
        );

        let mut cookie = data_conn.record_enable_context(context)?;
        loop {
            let reply = cookie.next().ok_or("XRecord 스트림 종료")??;
            // reply.data에는 32바이트 xEvent가 이어 붙는다.
            for raw in reply.data.chunks_exact(32) {
                handle_raw_event(raw);
            }
        }
    }

    /// xEvent 원시 바이트: [0]=타입, [1]=키코드, [28..30]=modifier state.
    fn handle_raw_event(raw: &[u8]) {
        if INJECTING.load(Ordering::SeqCst) || !ENABLED.load(Ordering::Relaxed) {
            return;
        }
        let event_type = raw[0] & 0x7F;
        match event_type {
            2 => {
                // KeyPress
                let keycode = raw[1];
                let state = u16::from_ne_bytes([raw[28], raw[29]]);
                on_key_press(keycode, state);
            }
            4 => {
                // ButtonPress: 클릭은 포커스/커서 이동일 수 있다 → 추적 포기.
                BUFFER.lock().unwrap().clear();
                *PENDING_UNDO.lock().unwrap() = None;
            }
            _ => {}
        }
    }

    fn on_key_press(keycode: u8, state: u16) {
        // 한/영 토글 키 관찰 → 모드 추적.
        if TOGGLE_KEYCODES.get().is_some_and(|t| t.contains(&keycode)) {
            KOREAN_MODE.fetch_xor(true, Ordering::Relaxed);
            return;
        }
        let shift = state & SHIFT_MASK != 0;
        let ctrl = state & CONTROL_MASK != 0;
        let alt = state & MOD1_MASK != 0;
        let superkey = state & MOD4_MASK != 0;
        // 모디파이어 키 자체의 KeyPress는 아무 상태도 바꾸지 않는다.
        let Some(class) = classify(keycode, shift).map(reclassify_for_layout) else {
            return;
        };
        GENERATION.fetch_add(1, Ordering::Relaxed);
        if keycode == SPACE_KEYCODE && ctrl && shift && !alt && !superkey {
            *PENDING_UNDO.lock().unwrap() = None;
            trigger_manual_conversion();
            return;
        }
        if ctrl || alt || superkey {
            BUFFER.lock().unwrap().clear();
            *PENDING_UNDO.lock().unwrap() = None;
            return;
        }

        if class == KeyClass::Backspace {
            if let Some(record) = PENDING_UNDO.lock().unwrap().take() {
                BUFFER.lock().unwrap().clear();
                trigger_undo(record);
                return;
            }
        }
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

    /// 토글 키를 눌러 IME 모드를 바꾸고 추적 상태를 갱신한다.
    fn switch_mode(to_korean: bool) {
        if KOREAN_MODE.load(Ordering::Relaxed) == to_korean {
            return;
        }
        let conn = INJECT_CONN.get().expect("initialized in run()");
        let toggle = TOGGLE_KEYCODES.get().expect("initialized in run()")[0];
        if inject::press_toggle_key(conn, toggle).is_ok() {
            KOREAN_MODE.store(to_korean, Ordering::Relaxed);
        }
    }

    fn trigger_auto_correction(keys: String, boundary: char) {
        let generation = GENERATION.load(Ordering::Relaxed);
        std::thread::spawn(move || {
            let Some(_guard) = CONVERTING.try_acquire() else {
                return; // 다른 주입 진행 중 — 이 교정은 버린다.
            };
            std::thread::sleep(Duration::from_millis(60));
            if GENERATION.load(Ordering::Relaxed) != generation || active_app_disabled() {
                return;
            }
            let korean_mode = KOREAN_MODE.load(Ordering::Relaxed);
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
            let conn = INJECT_CONN.get().expect("initialized in run()");
            if let Err(e) = inject::replace_text(conn, decision.backspaces, &decision.replacement) {
                eprintln!("이벤트 주입 실패: {e}");
                return;
            }
            switch_mode(decision.to_korean_mode);
            *PENDING_UNDO.lock().unwrap() = Some(UndoRecord {
                remaining_backspaces: decision.replacement.chars().count() - 1,
                revert: decision.revert,
                restore_korean_mode: korean_mode,
                exception_word: decision.screen_word,
            });
        });
    }

    fn trigger_undo(record: UndoRecord) {
        std::thread::spawn(move || {
            let Some(_guard) = CONVERTING.try_acquire() else {
                return;
            };
            let conn = INJECT_CONN.get().expect("initialized in run()");
            if let Err(e) = inject::replace_text(conn, record.remaining_backspaces, &record.revert)
            {
                eprintln!("되돌리기 주입 실패: {e}");
                return;
            }
            switch_mode(record.restore_korean_mode);
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

    fn trigger_manual_conversion() {
        std::thread::spawn(|| {
            let Some(_guard) = CONVERTING.try_acquire() else {
                return; // 진행 중인 변환이 있으면 이 누름은 버린다.
            };
            if active_app_disabled() {
                return;
            }
            let korean_mode = KOREAN_MODE.load(Ordering::Relaxed);
            let plan = {
                let mut buf = BUFFER.lock().unwrap();
                let Some(target) = buf.target() else { return };
                let plan = build_replace_plan(CONFIG.layout, korean_mode, &target);
                buf.mark_converted();
                plan
            };
            let conn = INJECT_CONN.get().expect("initialized in run()");
            if !wait_modifiers_released(conn) {
                return; // 모디파이어를 계속 누르고 있음 — 안전하게 포기.
            }
            if let Err(e) = inject::replace_text(conn, plan.backspaces, &plan.replacement) {
                eprintln!("이벤트 주입 실패: {e}");
                return;
            }
            switch_mode(!korean_mode);
        });
    }

    /// Ctrl/Shift가 물리적으로 떼어질 때까지 대기 (X 서버 modifier state
    /// 질의). 눌린 채 백스페이스를 주입하면 앱이 Ctrl+Backspace(단어 삭제)
    /// 로 해석하므로, 시간 안에 떼어지지 않으면 false를 돌려 포기시킨다.
    fn wait_modifiers_released(conn: &RustConnection) -> bool {
        use x11rb::protocol::xproto::ConnectionExt as _;
        let Some(root) = conn.setup().roots.first().map(|s| s.root) else {
            return false;
        };
        for _ in 0..150 {
            let held = conn
                .query_pointer(root)
                .ok()
                .and_then(|c| c.reply().ok())
                .map(|r| u16::from(r.mask) & (SHIFT_MASK | CONTROL_MASK) != 0)
                .unwrap_or(false);
            if !held {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }
}

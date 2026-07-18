//! hanengd — macOS 상주 데몬.
//!
//! CGEventTap(ListenOnly)으로 키를 관찰해 마지막 단어의 물리 키를 버퍼링한다.
//!
//! - **수동 변환 (Phase 1)**: ⌘⇧Space → 마지막 단어를 반대 모드 문자로 치환
//!   + 입력 소스 전환. 같은 단어에서 다시 누르면 되돌아온다.
//! - **자동 교정 (Phase 2)**: 단어 경계(공백·문장부호)에서 잘못된 모드로
//!   판정되면 자동 치환. 직후 백스페이스 1회로 되돌리면 그 단어를 예외
//!   사전에 학습해 다시 건드리지 않는다.
//!
//! 실행 전 요구 권한: 시스템 설정 → 개인정보 보호 및 보안 → 손쉬운 사용 +
//! 입력 모니터링에 이 바이너리(또는 실행한 터미널)를 추가.

#[cfg(target_os = "macos")]
mod inject;
#[cfg(target_os = "macos")]
mod keymap;
#[cfg(target_os = "macos")]
mod tis;
#[cfg(target_os = "macos")]
mod tray;

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
    use crate::inject::{self, INJECT_MARKER};
    use crate::keymap::classify;
    use crate::tis;
    use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
    use core_graphics::event::{
        CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions,
        CGEventTapPlacement, CGEventType, CallbackResult, EventField,
    };
    use haneng_core::{
        build_replace_plan, config, AutoCorrector, Detector, KeyClass, Target, WordBuffer,
    };
    use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
    use std::sync::{Arc, LazyLock, Mutex};
    use std::time::Duration;

    /// 트레이 토글로 제어되는 전역 스위치.
    pub static ENABLED: AtomicBool = AtomicBool::new(true);
    pub static AUTO: AtomicBool = AtomicBool::new(true);

    /// 사용자 키 입력마다 증가 — 자동 교정 태스크가 "그 사이 추가 입력이
    /// 없었는가"를 확인하는 세대 카운터 (주입 중 race 방지).
    static GENERATION: AtomicU64 = AtomicU64::new(0);

    /// 마지막 키 이벤트의 대상 프로세스 PID (앱별 예외 판정용).
    static LAST_TARGET_PID: AtomicI64 = AtomicI64::new(0);

    static CONFIG: LazyLock<config::Config> = LazyLock::new(config::load_config);

    static CORRECTOR: LazyLock<Mutex<AutoCorrector>> = LazyLock::new(|| {
        let mut detector = Detector::with_layout(CONFIG.sensitivity, CONFIG.layout);
        for word in config::load_exceptions() {
            detector.record_undo(&word);
        }
        Mutex::new(AutoCorrector::new(detector))
    });

    extern "C" {
        // libproc (libSystem에 포함) — PID의 실행 파일 경로.
        fn proc_pidpath(pid: i32, buffer: *mut u8, buffersize: u32) -> i32;
    }

    /// 마지막 키 이벤트를 받은 앱이 disabled_apps에 해당하는가.
    fn target_app_disabled() -> bool {
        if CONFIG.disabled_apps.is_empty() {
            return false;
        }
        let pid = LAST_TARGET_PID.load(Ordering::Relaxed);
        if pid <= 0 {
            return false;
        }
        let mut buf = [0u8; 4096];
        let len = unsafe { proc_pidpath(pid as i32, buf.as_mut_ptr(), buf.len() as u32) };
        if len <= 0 {
            return false;
        }
        let path = String::from_utf8_lossy(&buf[..len as usize]);
        let name = path.rsplit('/').next().unwrap_or(&path);
        CONFIG.app_disabled(name)
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
        /// 사용자 백스페이스가 경계를 이미 지웠으므로 남은 삭제 횟수.
        remaining_backspaces: usize,
        /// 재주입할 원문 (원래 단어 + 경계).
        revert: String,
        /// 복원할 IME 모드 (교정 전 모드).
        restore_korean_mode: bool,
        /// 예외 사전에 학습할 단어.
        exception_word: String,
    }

    static PENDING_UNDO: Mutex<Option<UndoRecord>> = Mutex::new(None);

    const SPACE_KEYCODE: i64 = 49;

    fn is_hotkey(keycode: i64, flags: CGEventFlags) -> bool {
        keycode == SPACE_KEYCODE
            && flags.contains(CGEventFlags::CGEventFlagCommand | CGEventFlags::CGEventFlagShift)
            && !flags
                .intersects(CGEventFlags::CGEventFlagControl | CGEventFlags::CGEventFlagAlternate)
    }

    pub fn run() {
        let no_tray = std::env::args().any(|a| a == "--no-tray");
        AUTO.store(CONFIG.auto, Ordering::Relaxed);
        LazyLock::force(&CORRECTOR);

        let state: Arc<Mutex<WordBuffer>> = Arc::new(Mutex::new(WordBuffer::new()));
        let tap_state = state.clone();

        let tap = CGEventTap::new(
            CGEventTapLocation::Session,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            vec![
                CGEventType::KeyDown,
                CGEventType::LeftMouseDown,
                CGEventType::RightMouseDown,
                CGEventType::OtherMouseDown,
            ],
            move |_proxy, event_type, event| {
                handle_event(&tap_state, event_type, event);
                CallbackResult::Keep
            },
        )
        .unwrap_or_else(|_| {
            eprintln!(
                "이벤트 탭 생성 실패 — 권한이 필요합니다.\n\
                 시스템 설정 → 개인정보 보호 및 보안 → 손쉬운 사용 / 입력 모니터링에\n\
                 hanengd(또는 실행 중인 터미널)를 추가한 뒤 다시 실행하세요."
            );
            std::process::exit(1);
        });

        let source = tap
            .mach_port()
            .create_runloop_source(0)
            .expect("runloop source");
        // 트레이(NSApp) 이벤트 루프도 메인 CFRunLoop을 구동하므로,
        // 탭 소스는 어느 경로로 가든 메인 런루프에 걸어두면 된다.
        let run_loop = CFRunLoop::get_current();
        unsafe {
            run_loop.add_source(&source, kCFRunLoopCommonModes);
        }
        tap.enable();
        eprintln!(
            "hanengd 실행 중 — 단어 경계 자동 교정 + ⌘⇧Space 수동 변환. \
             자동 교정 직후 백스페이스 1회로 되돌립니다."
        );

        if no_tray {
            CFRunLoop::run_current();
        } else {
            crate::tray::run_with_tray(&ENABLED, &AUTO);
        }
    }

    fn handle_event(state: &Arc<Mutex<WordBuffer>>, event_type: CGEventType, event: &CGEvent) {
        if !ENABLED.load(Ordering::Relaxed) {
            return;
        }
        if event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA) == INJECT_MARKER {
            return;
        }
        match event_type {
            CGEventType::LeftMouseDown
            | CGEventType::RightMouseDown
            | CGEventType::OtherMouseDown => {
                // 클릭은 포커스/커서 이동일 수 있다 → 추적 포기.
                state.lock().unwrap().clear();
                *PENDING_UNDO.lock().unwrap() = None;
            }
            CGEventType::KeyDown => {
                GENERATION.fetch_add(1, Ordering::Relaxed);
                LAST_TARGET_PID.store(
                    event.get_integer_value_field(EventField::EVENT_TARGET_UNIX_PROCESS_ID),
                    Ordering::Relaxed,
                );
                let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                let flags = event.get_flags();
                if is_hotkey(keycode, flags) {
                    *PENDING_UNDO.lock().unwrap() = None;
                    trigger_manual_conversion(state.clone());
                    return;
                }
                if flags.intersects(
                    CGEventFlags::CGEventFlagCommand
                        | CGEventFlags::CGEventFlagControl
                        | CGEventFlags::CGEventFlagAlternate,
                ) {
                    // 단축키 입력은 타이핑이 아니다.
                    state.lock().unwrap().clear();
                    *PENDING_UNDO.lock().unwrap() = None;
                    return;
                }
                let shift = flags.contains(CGEventFlags::CGEventFlagShift);
                let class = reclassify_for_layout(classify(keycode, shift));

                // 자동 교정 직후의 백스페이스 1회 = 되돌리기.
                if class == KeyClass::Backspace {
                    if let Some(record) = PENDING_UNDO.lock().unwrap().take() {
                        state.lock().unwrap().clear();
                        trigger_undo(record);
                        return;
                    }
                }
                // 그 외 어떤 키든 되돌리기 기회는 소멸.
                *PENDING_UNDO.lock().unwrap() = None;

                let mut buf = state.lock().unwrap();
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
            _ => {}
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
            if tis::secure_input_active() || target_app_disabled() {
                return;
            }
            let Some(korean_mode) = tis::current_source_is_korean() else {
                return;
            };
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
            if inject::replace_text(decision.backspaces, &decision.replacement).is_err() {
                eprintln!("이벤트 주입 실패");
                return;
            }
            tis::select_input_source(decision.to_korean_mode);
            *PENDING_UNDO.lock().unwrap() = Some(UndoRecord {
                remaining_backspaces: decision.replacement.chars().count() - 1,
                revert: decision.revert,
                restore_korean_mode: korean_mode,
                exception_word: decision.screen_word,
            });
            // 버퍼의 (keys, boundary)는 그대로 유효: 수동 핫키 토글도
            // 전환된 모드 기준으로 올바른 계획을 만든다.
        });
    }

    /// 자동 교정 되돌리기: 사용자 백스페이스가 경계를 지운 뒤 호출된다.
    fn trigger_undo(record: UndoRecord) {
        std::thread::spawn(move || {
            if inject::replace_text(record.remaining_backspaces, &record.revert).is_err() {
                eprintln!("되돌리기 주입 실패");
                return;
            }
            tis::select_input_source(record.restore_korean_mode);
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

    /// ⌘⇧Space 수동 변환. 탭 콜백을 막지 않도록 별도 스레드에서 실행.
    fn trigger_manual_conversion(state: Arc<Mutex<WordBuffer>>) {
        std::thread::spawn(move || {
            if tis::secure_input_active() || target_app_disabled() {
                return;
            }
            let Some(korean_mode) = tis::current_source_is_korean() else {
                return;
            };

            let plan = {
                let mut buf = state.lock().unwrap();
                let Some(target) = buf.target() else { return };
                let plan = build_replace_plan(CONFIG.layout, korean_mode, &target);
                buf.mark_converted();
                plan
            };

            // 사용자가 핫키 모디파이어에서 손을 뗄 시간.
            std::thread::sleep(Duration::from_millis(30));
            if inject::replace_text(plan.backspaces, &plan.replacement).is_err() {
                eprintln!("이벤트 주입 실패");
                return;
            }
            // 이어서 올바른 모드로 계속 타이핑할 수 있도록 입력 소스 전환.
            tis::select_input_source(!korean_mode);
        });
    }
}

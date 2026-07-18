//! 합성 키 이벤트 주입 — 백스페이스 n회 + 유니코드 텍스트 타이핑.
//!
//! 모든 주입 이벤트에는 사용자 데이터 마커를 심어 우리 탭이 자기 이벤트를
//! 다시 소비하지 않게 한다 (PLAN.md의 race 방지 원칙).

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, EventField};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use std::thread::sleep;
use std::time::Duration;

/// "HAEN" — 주입 이벤트 식별자.
pub const INJECT_MARKER: i64 = 0x4841_454E;

const BACKSPACE_KEYCODE: u16 = 51;
/// 연속 합성 이벤트를 앱이 놓치지 않도록 이벤트 사이에 두는 간격.
const EVENT_GAP: Duration = Duration::from_millis(2);
/// CGEventKeyboardSetUnicodeString이 이벤트 하나에 안전하게 싣는 UTF-16 단위 수.
const TEXT_CHUNK_UTF16: usize = 20;

fn post_key(source: &CGEventSource, keycode: u16, text: Option<&[u16]>) -> Result<(), ()> {
    for down in [true, false] {
        let event = CGEvent::new_keyboard_event(source.clone(), keycode, down)?;
        // 사용자가 아직 핫키 모디파이어를 누르고 있어도 주입 이벤트에
        // 섞이지 않도록 플래그를 비운다.
        event.set_flags(CGEventFlags::CGEventFlagNull);
        if let (Some(text), true) = (text, down) {
            event.set_string_from_utf16_unchecked(text);
        }
        event.set_integer_value_field(EventField::EVENT_SOURCE_USER_DATA, INJECT_MARKER);
        event.post(CGEventTapLocation::HID);
        sleep(EVENT_GAP);
    }
    Ok(())
}

/// 백스페이스 `backspaces`회 후 `text`를 타이핑한다.
pub fn replace_text(backspaces: usize, text: &str) -> Result<(), ()> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)?;
    for _ in 0..backspaces {
        post_key(&source, BACKSPACE_KEYCODE, None)?;
    }
    let units: Vec<u16> = text.encode_utf16().collect();
    for chunk in units.chunks(TEXT_CHUNK_UTF16) {
        post_key(&source, 0, Some(chunk))?;
    }
    Ok(())
}

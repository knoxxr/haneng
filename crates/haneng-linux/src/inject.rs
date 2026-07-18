//! XTest 기반 주입 — 백스페이스 n회 + 유니코드 텍스트 타이핑.
//!
//! X11에는 이벤트에 마커를 심을 방법이 없어서, 주입하는 동안
//! `INJECTING` 플래그를 올려 XRecord 관찰 쪽이 자기 이벤트를 무시하게
//! 한다 (그 사이 사용자 실입력도 함께 무시되지만, 주입 중 사용자 입력은
//! 어차피 결과를 깨뜨리는 race라 보수적으로 버리는 편이 안전하다).
//!
//! 임의 유니코드 타이핑은 xdotool과 같은 방식: 비어 있는 스페어 키코드에
//! 문자의 keysym(U+xxxx → 0x0100_0000 + 코드포인트)을 임시 매핑하고
//! 눌렀다가, 끝나면 매핑을 되돌린다.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

pub static INJECTING: AtomicBool = AtomicBool::new(false);

const KEY_PRESS: u8 = 2;
const KEY_RELEASE: u8 = 3;
/// 연속 합성 이벤트 간격 + 키맵 변경 전파 시간.
const EVENT_GAP: Duration = Duration::from_millis(2);

fn fake_key(conn: &RustConnection, keycode: u8) -> Result<(), Box<dyn std::error::Error>> {
    for kind in [KEY_PRESS, KEY_RELEASE] {
        conn.xtest_fake_input(kind, keycode, 0, x11rb::NONE, 0, 0, 0)?;
        conn.flush()?;
        sleep(EVENT_GAP);
    }
    Ok(())
}

/// 모든 keysym이 비어 있는 스페어 키코드를 찾는다.
fn find_spare_keycode(conn: &RustConnection) -> Result<u8, Box<dyn std::error::Error>> {
    let setup = conn.setup();
    let (min, max) = (setup.min_keycode, setup.max_keycode);
    let mapping = conn.get_keyboard_mapping(min, max - min + 1)?.reply()?;
    let per = mapping.keysyms_per_keycode as usize;
    for (i, chunk) in mapping.keysyms.chunks(per).enumerate() {
        if chunk.iter().all(|&s| s == 0) {
            return Ok(min + i as u8);
        }
    }
    Err("스페어 키코드 없음".into())
}

fn remap(
    conn: &RustConnection,
    keycode: u8,
    keysym: u32,
    per: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let syms = vec![keysym; per as usize];
    conn.change_keyboard_mapping(1, keycode, per, &syms)?;
    conn.flush()?;
    // 매핑 변경이 앱까지 전파될 시간.
    sleep(Duration::from_millis(5));
    Ok(())
}

/// 백스페이스 `backspaces`회 후 `text`를 타이핑한다.
pub fn replace_text(
    conn: &RustConnection,
    backspaces: usize,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    INJECTING.store(true, Ordering::SeqCst);
    let result = replace_text_inner(conn, backspaces, text);
    INJECTING.store(false, Ordering::SeqCst);
    result
}

fn replace_text_inner(
    conn: &RustConnection,
    backspaces: usize,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for _ in 0..backspaces {
        fake_key(conn, crate::keymap::BACKSPACE_KEYCODE)?;
    }
    if text.is_empty() {
        return Ok(());
    }
    let spare = find_spare_keycode(conn)?;
    // 그룹1의 (기본, Shift) 두 슬롯이면 충분하다.
    let per = 2u8;
    for c in text.chars() {
        let keysym = if (c as u32) < 0x100 {
            c as u32
        } else {
            0x0100_0000 + c as u32
        };
        remap(conn, spare, keysym, per)?;
        fake_key(conn, spare)?;
    }
    // 스페어 키코드 원상 복구 (빈 매핑).
    remap(conn, spare, 0, per)?;
    Ok(())
}

/// 한/영 토글 키를 흉내 내 IME 모드를 전환한다.
pub fn press_toggle_key(
    conn: &RustConnection,
    keycode: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    INJECTING.store(true, Ordering::SeqCst);
    let result = fake_key(conn, keycode);
    INJECTING.store(false, Ordering::SeqCst);
    result
}

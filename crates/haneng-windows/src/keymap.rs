//! Windows 가상 키코드(VK_*) → 물리 키 분류.
//!
//! Windows는 **띄어쓰기 기준 수동 변환만** 지원한다: 공백만 단어 경계이고,
//! 그 밖의 인쇄 가능 문자(숫자·문장부호 포함)는 전부 단어의 일부다.
//! 치환은 백스페이스 개수 계산 없이 Ctrl+Shift+Left 선택 위에 타이핑하므로
//! 단어에 어떤 문자가 섞여 있어도 안전하다.

pub use haneng_core::KeyClass;

/// 한/영 전환 키. LL 훅에서 이 키를 관찰해 IME 모드를 추적한다
/// (Win11 신형 한글 IME는 WM_IME_CONTROL 모드 질의에 응답하지 않는다).
// Windows 밖에서는 호출부(훅)가 컴파일되지 않지만 테스트를 위해 유지한다.
#[cfg_attr(not(windows), allow(dead_code))]
pub const VK_HANGUL_KEY: u16 = 0x15;

/// `None` = 버퍼에 영향을 주지 않는 키 (모디파이어, 한/영·한자 키).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn classify(vk: u16, shift: bool) -> Option<KeyClass> {
    use KeyClass::*;
    let sym = |plain: char, shifted: char| Letter(if shift { shifted } else { plain });
    // VK_SHIFT/CONTROL/MENU(+L/R 변형), CapsLock, Win, 한/영(0x15), 한자(0x19).
    if matches!(vk, 0x10..=0x12 | 0x14 | 0x15 | 0x19 | 0x5B | 0x5C | 0xA0..=0xA5) {
        return None;
    }
    Some(match vk {
        0x20 => Boundary(' '),
        0x08 => Backspace,
        // A-Z
        0x41..=0x5A => {
            let c = (b'a' + (vk - 0x41) as u8) as char;
            Letter(if shift { c.to_ascii_uppercase() } else { c })
        }
        // 숫자열 (US 배열 시프트 기호) — 공백이 아니므로 단어의 일부.
        0x30 => sym('0', ')'),
        0x31 => sym('1', '!'),
        0x32 => sym('2', '@'),
        0x33 => sym('3', '#'),
        0x34 => sym('4', '$'),
        0x35 => sym('5', '%'),
        0x36 => sym('6', '^'),
        0x37 => sym('7', '&'),
        0x38 => sym('8', '*'),
        0x39 => sym('9', '('),
        // OEM 문장부호
        0xBA => sym(';', ':'),
        0xBB => sym('=', '+'),
        0xBC => sym(',', '<'),
        0xBD => sym('-', '_'),
        0xBE => sym('.', '>'),
        0xBF => sym('/', '?'),
        0xC0 => sym('`', '~'),
        0xDB => sym('[', '{'),
        0xDC => sym('\\', '|'),
        0xDD => sym(']', '}'),
        0xDE => sym('\'', '"'),
        // 키패드
        0x60..=0x69 => Letter((b'0' + (vk - 0x60) as u8) as char),
        0x6A => Letter('*'),
        0x6B => Letter('+'),
        0x6D => Letter('-'),
        0x6E => Letter('.'),
        0x6F => Letter('/'),
        // 엔터·커서 이동·기타: 추적 포기.
        _ => Clear,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_is_the_only_boundary() {
        assert_eq!(classify(0x20, false), Some(KeyClass::Boundary(' ')));
        // 숫자·문장부호는 단어의 일부다.
        assert_eq!(classify(0x31, true), Some(KeyClass::Letter('!')));
        assert_eq!(classify(0xBE, false), Some(KeyClass::Letter('.')));
        assert_eq!(classify(0x41, false), Some(KeyClass::Letter('a')));
        assert_eq!(classify(0x08, false), Some(KeyClass::Backspace));
        assert_eq!(classify(0x0D, false), Some(KeyClass::Clear)); // Enter
    }

    #[test]
    fn modifiers_and_ime_keys_are_ignored() {
        for vk in [0x10, 0x11, 0x12, 0x14, 0x15, 0x19, 0x5B, 0xA0, 0xA5] {
            assert_eq!(classify(vk, false), None, "vk={vk:#x}");
        }
    }
}

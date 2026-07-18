//! Windows 가상 키코드(VK_*) → 물리 키 분류.
//!
//! macOS의 keymap과 마찬가지로 활성 IME와 무관하게 "어떤 QWERTY 키를
//! 눌렀는가"만 분류한다. 순수 매핑이라 어떤 플랫폼에서도 테스트 가능.

pub use haneng_core::KeyClass;

/// `None` = 버퍼에 영향을 주지 않는 키 (모디파이어 자체의 KeyDown 등).
/// 모디파이어를 `Clear`로 처리하면 Shift를 누르는 순간 단어 추적이 끊긴다.
// Windows 밖에서는 호출부(훅)가 컴파일되지 않지만 테스트를 위해 유지한다.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn classify(vk: u16, shift: bool) -> Option<KeyClass> {
    use KeyClass::*;
    let sym = |plain: char, shifted: char| Boundary(if shift { shifted } else { plain });
    // VK_SHIFT/CONTROL/MENU(+L/R 변형), CapsLock, Win 키.
    if matches!(vk, 0x10..=0x12 | 0x14 | 0x5B | 0x5C | 0xA0..=0xA5) {
        return None;
    }
    Some(match vk {
        // A-Z
        0x41..=0x5A => {
            let c = (b'a' + (vk - 0x41) as u8) as char;
            Letter(if shift { c.to_ascii_uppercase() } else { c })
        }
        // 숫자열 (US 배열 시프트 기호)
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
        0x20 => Boundary(' '),
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
        0x60..=0x69 => Boundary((b'0' + (vk - 0x60) as u8) as char),
        0x6A => Boundary('*'),
        0x6B => Boundary('+'),
        0x6D => Boundary('-'),
        0x6E => Boundary('.'),
        0x6F => Boundary('/'),

        0x08 => Backspace,
        // 엔터는 제출(전송)일 수 있어 경계로 삼지 않는다. 나머지 미지의 키도
        // 커서를 움직일 수 있으므로 보수적으로 추적을 버린다.
        _ => Clear,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_shift() {
        assert_eq!(classify(0x41, false), Some(KeyClass::Letter('a')));
        assert_eq!(classify(0x5A, true), Some(KeyClass::Letter('Z')));
    }

    #[test]
    fn boundaries_and_control_keys() {
        assert_eq!(classify(0x20, false), Some(KeyClass::Boundary(' ')));
        assert_eq!(classify(0x31, true), Some(KeyClass::Boundary('!')));
        assert_eq!(classify(0xBE, false), Some(KeyClass::Boundary('.')));
        assert_eq!(classify(0x08, false), Some(KeyClass::Backspace));
        assert_eq!(classify(0x0D, false), Some(KeyClass::Clear)); // Enter
        assert_eq!(classify(0x25, false), Some(KeyClass::Clear)); // ←
    }

    #[test]
    fn modifier_keydowns_are_ignored() {
        for vk in [0x10, 0x11, 0x12, 0x14, 0x5B, 0xA0, 0xA1, 0xA2, 0xA5] {
            assert_eq!(classify(vk, false), None, "vk={vk:#x}");
        }
    }
}

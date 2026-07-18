//! X11 키코드(evdev + 8, 표준 QWERTY xkb 배열) → 물리 키 분류.
//!
//! X11에서는 모디파이어(Shift·Ctrl 등)도 KeyPress로 오므로 `None`으로
//! 무시한다 — Clear로 처리하면 Shift 타이핑과 핫키가 깨진다.

pub use haneng_core::KeyClass;

/// 한/영 전환으로 흔히 쓰는 키코드 (Hangul, 오른쪽 Alt).
/// `config.txt`의 `linux_toggle_keycodes = 130,108`로 재정의 가능.
pub const DEFAULT_TOGGLE_KEYCODES: [u8; 2] = [130, 108];

pub const SPACE_KEYCODE: u8 = 65;
pub const BACKSPACE_KEYCODE: u8 = 22;

// X11 modifier state 비트.
pub const SHIFT_MASK: u16 = 1;
pub const CONTROL_MASK: u16 = 4;
pub const MOD1_MASK: u16 = 8; // Alt
pub const MOD4_MASK: u16 = 64; // Super

/// `None` = 버퍼에 영향을 주지 않는 키 (모디파이어, 한/영 토글 등).
pub fn classify(keycode: u8, shift: bool) -> Option<KeyClass> {
    use KeyClass::*;
    let letter = |c: char| Letter(if shift { c.to_ascii_uppercase() } else { c });
    let sym = |plain: char, shifted: char| Boundary(if shift { shifted } else { plain });
    // Shift(50,62), Ctrl(37,105), Alt(64,108), Super(133,134), CapsLock(66),
    // 한/영·한자(130,131)는 무시.
    if matches!(
        keycode,
        50 | 62 | 37 | 105 | 64 | 108 | 133 | 134 | 66 | 130 | 131
    ) {
        return None;
    }
    Some(match keycode {
        24 => letter('q'),
        25 => letter('w'),
        26 => letter('e'),
        27 => letter('r'),
        28 => letter('t'),
        29 => letter('y'),
        30 => letter('u'),
        31 => letter('i'),
        32 => letter('o'),
        33 => letter('p'),
        38 => letter('a'),
        39 => letter('s'),
        40 => letter('d'),
        41 => letter('f'),
        42 => letter('g'),
        43 => letter('h'),
        44 => letter('j'),
        45 => letter('k'),
        46 => letter('l'),
        52 => letter('z'),
        53 => letter('x'),
        54 => letter('c'),
        55 => letter('v'),
        56 => letter('b'),
        57 => letter('n'),
        58 => letter('m'),

        65 => Boundary(' '),
        10 => sym('1', '!'),
        11 => sym('2', '@'),
        12 => sym('3', '#'),
        13 => sym('4', '$'),
        14 => sym('5', '%'),
        15 => sym('6', '^'),
        16 => sym('7', '&'),
        17 => sym('8', '*'),
        18 => sym('9', '('),
        19 => sym('0', ')'),
        20 => sym('-', '_'),
        21 => sym('=', '+'),
        34 => sym('[', '{'),
        35 => sym(']', '}'),
        47 => sym(';', ':'),
        48 => sym('\'', '"'),
        49 => sym('`', '~'),
        51 => sym('\\', '|'),
        59 => sym(',', '<'),
        60 => sym('.', '>'),
        61 => sym('/', '?'),

        22 => Backspace,
        // 엔터(36)는 제출일 수 있어 경계로 삼지 않는다. 나머지 미지의 키도
        // 커서를 움직일 수 있으므로 보수적으로 추적을 버린다.
        _ => Clear,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_boundaries() {
        assert_eq!(classify(38, false), Some(KeyClass::Letter('a')));
        assert_eq!(classify(24, true), Some(KeyClass::Letter('Q')));
        assert_eq!(classify(65, false), Some(KeyClass::Boundary(' ')));
        assert_eq!(classify(60, false), Some(KeyClass::Boundary('.')));
        assert_eq!(classify(22, false), Some(KeyClass::Backspace));
        assert_eq!(classify(36, false), Some(KeyClass::Clear)); // Enter
        assert_eq!(classify(113, false), Some(KeyClass::Clear)); // ←
    }

    #[test]
    fn modifiers_and_toggles_are_ignored() {
        for kc in [50, 62, 37, 105, 64, 108, 66, 130, 131, 133] {
            assert_eq!(classify(kc, false), None, "keycode={kc}");
        }
    }
}

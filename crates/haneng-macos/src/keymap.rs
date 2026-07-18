//! macOS ANSI 가상 키코드(kVK_ANSI_*) → 물리 키 분류.
//!
//! 활성 IME와 무관하게 "어떤 QWERTY 키를 눌렀는가"를 추적하는 것이 목적이다.
//! 한글 모드였다면 화면에 무엇이 조합됐는지는 코어의 `Composer`가 재현한다.

pub use haneng_core::KeyClass;

pub fn classify(keycode: i64, shift: bool) -> KeyClass {
    use KeyClass::*;
    let letter = |c: char| Letter(if shift { c.to_ascii_uppercase() } else { c });
    let sym = |plain: char, shifted: char| Boundary(if shift { shifted } else { plain });
    match keycode {
        0 => letter('a'),
        1 => letter('s'),
        2 => letter('d'),
        3 => letter('f'),
        4 => letter('h'),
        5 => letter('g'),
        6 => letter('z'),
        7 => letter('x'),
        8 => letter('c'),
        9 => letter('v'),
        11 => letter('b'),
        12 => letter('q'),
        13 => letter('w'),
        14 => letter('e'),
        15 => letter('r'),
        16 => letter('y'),
        17 => letter('t'),
        31 => letter('o'),
        32 => letter('u'),
        34 => letter('i'),
        35 => letter('p'),
        37 => letter('l'),
        38 => letter('j'),
        40 => letter('k'),
        45 => letter('n'),
        46 => letter('m'),

        49 => Boundary(' '),
        18 => sym('1', '!'),
        19 => sym('2', '@'),
        20 => sym('3', '#'),
        21 => sym('4', '$'),
        23 => sym('5', '%'),
        22 => sym('6', '^'),
        26 => sym('7', '&'),
        28 => sym('8', '*'),
        25 => sym('9', '('),
        29 => sym('0', ')'),
        27 => sym('-', '_'),
        24 => sym('=', '+'),
        33 => sym('[', '{'),
        30 => sym(']', '}'),
        42 => sym('\\', '|'),
        41 => sym(';', ':'),
        39 => sym('\'', '"'),
        43 => sym(',', '<'),
        47 => sym('.', '>'),
        44 => sym('/', '?'),
        50 => sym('`', '~'),
        // 키패드
        82 => Boundary('0'),
        83 => Boundary('1'),
        84 => Boundary('2'),
        85 => Boundary('3'),
        86 => Boundary('4'),
        87 => Boundary('5'),
        88 => Boundary('6'),
        89 => Boundary('7'),
        91 => Boundary('8'),
        92 => Boundary('9'),
        65 => Boundary('.'),
        67 => Boundary('*'),
        69 => Boundary('+'),
        75 => Boundary('/'),
        78 => Boundary('-'),

        51 => Backspace,
        // 엔터는 제출(전송)일 수 있어 경계로 삼지 않는다. 나머지 미지의 키도
        // 커서를 움직일 수 있으므로 보수적으로 추적을 버린다.
        _ => Clear,
    }
}

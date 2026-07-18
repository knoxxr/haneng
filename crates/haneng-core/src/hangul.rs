//! 한글 음절/자모 데이터와 조합·분해 프리미티브.
//!
//! 유니코드 완성형 음절(U+AC00..U+D7A3)과 호환 자모(U+3131..U+3163)만 다룬다.
//! 옛한글은 범위 밖.

pub const SYLLABLE_BASE: u32 = 0xAC00;
pub const SYLLABLE_LAST: u32 = 0xD7A3;

/// 초성 19자 (유니코드 초성 인덱스 순서).
pub const CHOSEONG: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ',
    'ㅌ', 'ㅍ', 'ㅎ',
];

/// 중성 21자 (유니코드 중성 인덱스 순서).
pub const JUNGSEONG: [char; 21] = [
    'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ',
    'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
];

/// 종성 27자. 배열 인덱스 + 1 = 유니코드 종성 코드 (0 = 종성 없음).
pub const JONGSEONG: [char; 27] = [
    'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ', 'ㅀ', 'ㅁ',
    'ㅂ', 'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];

/// 두 중성이 결합해 만드는 겹중성: (첫째, 둘째, 결과).
const COMPOUND_JUNG: [(char, char, char); 7] = [
    ('ㅗ', 'ㅏ', 'ㅘ'),
    ('ㅗ', 'ㅐ', 'ㅙ'),
    ('ㅗ', 'ㅣ', 'ㅚ'),
    ('ㅜ', 'ㅓ', 'ㅝ'),
    ('ㅜ', 'ㅔ', 'ㅞ'),
    ('ㅜ', 'ㅣ', 'ㅟ'),
    ('ㅡ', 'ㅣ', 'ㅢ'),
];

/// 두 종성이 결합해 만드는 겹받침: (첫째, 둘째, 결과).
const COMPOUND_JONG: [(char, char, char); 11] = [
    ('ㄱ', 'ㅅ', 'ㄳ'),
    ('ㄴ', 'ㅈ', 'ㄵ'),
    ('ㄴ', 'ㅎ', 'ㄶ'),
    ('ㄹ', 'ㄱ', 'ㄺ'),
    ('ㄹ', 'ㅁ', 'ㄻ'),
    ('ㄹ', 'ㅂ', 'ㄼ'),
    ('ㄹ', 'ㅅ', 'ㄽ'),
    ('ㄹ', 'ㅌ', 'ㄾ'),
    ('ㄹ', 'ㅍ', 'ㄿ'),
    ('ㄹ', 'ㅎ', 'ㅀ'),
    ('ㅂ', 'ㅅ', 'ㅄ'),
];

pub fn cho_index(c: char) -> Option<usize> {
    CHOSEONG.iter().position(|&x| x == c)
}

pub fn jung_index(c: char) -> Option<usize> {
    JUNGSEONG.iter().position(|&x| x == c)
}

/// 종성 배열 내 인덱스 (유니코드 종성 코드 - 1).
pub fn jong_index(c: char) -> Option<usize> {
    JONGSEONG.iter().position(|&x| x == c)
}

pub fn is_vowel_jamo(c: char) -> bool {
    jung_index(c).is_some()
}

/// U+3131..U+3163 호환 자모 (겹자모 포함).
pub fn is_compat_jamo(c: char) -> bool {
    ('\u{3131}'..='\u{3163}').contains(&c)
}

pub fn is_syllable(c: char) -> bool {
    (SYLLABLE_BASE..=SYLLABLE_LAST).contains(&(c as u32))
}

/// 같은 초성 두 번으로 만드는 쌍자음 (세벌식 조합 규칙).
const COMPOUND_CHO: [(char, char); 5] = [
    ('ㄱ', 'ㄲ'),
    ('ㄷ', 'ㄸ'),
    ('ㅂ', 'ㅃ'),
    ('ㅅ', 'ㅆ'),
    ('ㅈ', 'ㅉ'),
];

pub fn combine_cho(a: char, b: char) -> Option<char> {
    if a != b {
        return None;
    }
    COMPOUND_CHO
        .iter()
        .find(|&&(base, _)| base == a)
        .map(|&(_, double)| double)
}

pub fn split_cho(c: char) -> Option<(char, char)> {
    COMPOUND_CHO
        .iter()
        .find(|&&(_, double)| double == c)
        .map(|&(base, _)| (base, base))
}

pub fn combine_jung(a: char, b: char) -> Option<char> {
    COMPOUND_JUNG
        .iter()
        .find(|&&(x, y, _)| x == a && y == b)
        .map(|&(_, _, z)| z)
}

pub fn split_jung(c: char) -> Option<(char, char)> {
    COMPOUND_JUNG
        .iter()
        .find(|&&(_, _, z)| z == c)
        .map(|&(x, y, _)| (x, y))
}

pub fn combine_jong(a: char, b: char) -> Option<char> {
    COMPOUND_JONG
        .iter()
        .find(|&&(x, y, _)| x == a && y == b)
        .map(|&(_, _, z)| z)
}

pub fn split_jong(c: char) -> Option<(char, char)> {
    COMPOUND_JONG
        .iter()
        .find(|&&(_, _, z)| z == c)
        .map(|&(x, y, _)| (x, y))
}

/// 초성/중성 인덱스와 종성 코드(0 = 없음)로 완성형 음절 하나를 만든다.
pub fn compose_syllable(cho: usize, jung: usize, jong_code: usize) -> char {
    debug_assert!(cho < 19 && jung < 21 && jong_code < 28);
    let code = SYLLABLE_BASE + (cho as u32 * 21 + jung as u32) * 28 + jong_code as u32;
    char::from_u32(code).expect("valid hangul syllable code point")
}

/// 완성형 음절을 (초성 인덱스, 중성 인덱스, 종성 코드)로 분해한다.
pub fn decompose_syllable(c: char) -> Option<(usize, usize, usize)> {
    if !is_syllable(c) {
        return None;
    }
    let offset = c as u32 - SYLLABLE_BASE;
    let jong = (offset % 28) as usize;
    let jung = ((offset / 28) % 21) as usize;
    let cho = (offset / 28 / 21) as usize;
    Some((cho, jung, jong))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_decompose_roundtrip() {
        for (c, expected) in [('한', (18, 0, 4)), ('글', (0, 18, 8)), ('가', (0, 0, 0))] {
            assert_eq!(decompose_syllable(c), Some(expected));
            let (cho, jung, jong) = expected;
            assert_eq!(compose_syllable(cho, jung, jong), c);
        }
    }

    #[test]
    fn compound_tables() {
        assert_eq!(combine_jung('ㅗ', 'ㅏ'), Some('ㅘ'));
        assert_eq!(split_jung('ㅢ'), Some(('ㅡ', 'ㅣ')));
        assert_eq!(combine_jong('ㄹ', 'ㄱ'), Some('ㄺ'));
        assert_eq!(split_jong('ㅄ'), Some(('ㅂ', 'ㅅ')));
        assert_eq!(combine_jung('ㅏ', 'ㅗ'), None);
        assert_eq!(combine_jong('ㅂ', 'ㄹ'), None);
    }
}

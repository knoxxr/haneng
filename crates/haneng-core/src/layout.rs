//! 자판 배열 — 두벌식(KS X 5002)·세벌식(390/최종) ↔ QWERTY 키 매핑.
//!
//! 두벌식: 쌍자음·일부 겹모음은 Shift(대문자)에 배정, 자음은 초성/종성
//! 구분이 없어 오토마타가 위치를 결정한다(`KeyJamo::Dual`).
//! 세벌식: libhangul 자판 데이터에서 생성한 테이블(`sebeolsik_data.rs`)을
//! 쓰며, 모든 자모가 위치 명시적이다. 숫자·문장부호 키도 자모를 낼 수 있다.

use crate::sebeolsik_data::{SEBEOLSIK_390, SEBEOLSIK_FINAL};

/// 지원 자판 배열.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    #[default]
    Dubeolsik,
    Sebeolsik390,
    SebeolsikFinal,
}

/// 키 하나가 내는 자모 (위치 정보 포함).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyJamo {
    /// 두벌식 자음: 초성/종성은 오토마타가 결정.
    Dual(char),
    Cho(char),
    Jung(char),
    Jong(char),
    /// 이 자판에서 자모가 아닌 문자를 내는 키 (그대로 출력).
    Other(char),
}

fn sebeolsik_table(layout: Layout) -> &'static [(char, u8, char); 94] {
    match layout {
        Layout::Sebeolsik390 => &SEBEOLSIK_390,
        Layout::SebeolsikFinal => &SEBEOLSIK_FINAL,
        Layout::Dubeolsik => unreachable!("dubeolsik has no generated table"),
    }
}

/// 자판에서 키 하나가 내는 자모. 자판에 없는 키(공백 등)는 None.
pub fn key_jamo(layout: Layout, key: char) -> Option<KeyJamo> {
    match layout {
        Layout::Dubeolsik => {
            let jamo = key_to_jamo(key)?;
            Some(if crate::hangul::is_vowel_jamo(jamo) {
                KeyJamo::Jung(jamo)
            } else {
                KeyJamo::Dual(jamo)
            })
        }
        _ => {
            let table = sebeolsik_table(layout);
            let &(_, kind, jamo) = table.iter().find(|&&(k, _, _)| k == key)?;
            Some(match kind {
                0 => KeyJamo::Cho(jamo),
                1 => KeyJamo::Jung(jamo),
                2 => KeyJamo::Jong(jamo),
                _ => KeyJamo::Other(jamo),
            })
        }
    }
}

/// 이 키가 이 자판에서 단어를 이루는가 (자모를 내는가).
/// 어댑터가 Letter/Boundary 분류에 쓴다 — 세벌식에서는 숫자·문장부호도
/// 자모일 수 있다.
pub fn is_word_key(layout: Layout, key: char) -> bool {
    matches!(
        key_jamo(layout, key),
        Some(KeyJamo::Dual(_) | KeyJamo::Cho(_) | KeyJamo::Jung(_) | KeyJamo::Jong(_))
    )
}

/// 특정 위치의 자모를 내는 키를 찾는다 (겹자모는 직접 키가 없으면 None —
/// 호출부가 분해해서 다시 찾는다). kind: 0=초성, 1=중성, 2=종성.
pub fn key_for_jamo(layout: Layout, kind: u8, jamo: char) -> Option<char> {
    match layout {
        Layout::Dubeolsik => jamo_to_key(jamo),
        _ => sebeolsik_table(layout)
            .iter()
            .find(|&&(_, k, j)| k == kind && j == jamo)
            .map(|&(key, _, _)| key),
    }
}

/// QWERTY 키 하나를 두벌식 자모로. 한글 자판에 없는 키(숫자·기호)는 None.
pub fn key_to_jamo(key: char) -> Option<char> {
    let jamo = match key {
        // Shift 배정 자모
        'Q' => 'ㅃ',
        'W' => 'ㅉ',
        'E' => 'ㄸ',
        'R' => 'ㄲ',
        'T' => 'ㅆ',
        'O' => 'ㅒ',
        'P' => 'ㅖ',
        _ => match key.to_ascii_lowercase() {
            'q' => 'ㅂ',
            'w' => 'ㅈ',
            'e' => 'ㄷ',
            'r' => 'ㄱ',
            't' => 'ㅅ',
            'y' => 'ㅛ',
            'u' => 'ㅕ',
            'i' => 'ㅑ',
            'o' => 'ㅐ',
            'p' => 'ㅔ',
            'a' => 'ㅁ',
            's' => 'ㄴ',
            'd' => 'ㅇ',
            'f' => 'ㄹ',
            'g' => 'ㅎ',
            'h' => 'ㅗ',
            'j' => 'ㅓ',
            'k' => 'ㅏ',
            'l' => 'ㅣ',
            'z' => 'ㅋ',
            'x' => 'ㅌ',
            'c' => 'ㅊ',
            'v' => 'ㅍ',
            'b' => 'ㅠ',
            'n' => 'ㅜ',
            'm' => 'ㅡ',
            _ => return None,
        },
    };
    Some(jamo)
}

/// 홑자모 하나를 QWERTY 키로. 겹자모(ㄳ, ㅘ 등)는 호출 전에 분해해야 한다.
pub fn jamo_to_key(jamo: char) -> Option<char> {
    let key = match jamo {
        'ㅂ' => 'q',
        'ㅃ' => 'Q',
        'ㅈ' => 'w',
        'ㅉ' => 'W',
        'ㄷ' => 'e',
        'ㄸ' => 'E',
        'ㄱ' => 'r',
        'ㄲ' => 'R',
        'ㅅ' => 't',
        'ㅆ' => 'T',
        'ㅛ' => 'y',
        'ㅕ' => 'u',
        'ㅑ' => 'i',
        'ㅐ' => 'o',
        'ㅒ' => 'O',
        'ㅔ' => 'p',
        'ㅖ' => 'P',
        'ㅁ' => 'a',
        'ㄴ' => 's',
        'ㅇ' => 'd',
        'ㄹ' => 'f',
        'ㅎ' => 'g',
        'ㅗ' => 'h',
        'ㅓ' => 'j',
        'ㅏ' => 'k',
        'ㅣ' => 'l',
        'ㅋ' => 'z',
        'ㅌ' => 'x',
        'ㅊ' => 'c',
        'ㅍ' => 'v',
        'ㅠ' => 'b',
        'ㅜ' => 'n',
        'ㅡ' => 'm',
        _ => return None,
    };
    Some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_33_jamo_roundtrip() {
        // 두벌식 33자모 전부: 키 → 자모 → 키 왕복 일치.
        for key in ('a'..='z').chain(['Q', 'W', 'E', 'R', 'T', 'O', 'P']) {
            let jamo = key_to_jamo(key).unwrap();
            let back = jamo_to_key(jamo).unwrap();
            let expected = match key {
                'Q' | 'W' | 'E' | 'R' | 'T' | 'O' | 'P' => key,
                _ => key.to_ascii_lowercase(),
            };
            assert_eq!(back, expected, "key {key} → {jamo} → {back}");
        }
    }

    #[test]
    fn uppercase_without_shift_jamo_falls_back() {
        assert_eq!(key_to_jamo('A'), Some('ㅁ'));
        assert_eq!(key_to_jamo('Y'), Some('ㅛ'));
    }

    #[test]
    fn non_letters_are_none() {
        assert_eq!(key_to_jamo('1'), None);
        assert_eq!(key_to_jamo(' '), None);
        assert_eq!(jamo_to_key('ㄳ'), None); // 겹자모는 분해 후 매핑
    }
}

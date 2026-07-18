//! 한글 → 영문 키 시퀀스 역변환.
//!
//! 한글 모드로 잘못 입력된 텍스트(예: `"ㅗ디ㅣㅐ"`)를 원래 치려던 영문
//! (`"hello"`)으로 되돌린다. 완성형 음절은 초·중·종성으로, 겹자모는 두
//! 키 입력으로 분해한다.

use crate::hangul::{
    decompose_syllable, is_compat_jamo, is_vowel_jamo, split_cho, split_jong, split_jung, CHOSEONG,
    JONGSEONG, JUNGSEONG,
};
use crate::layout::{key_for_jamo, Layout};

/// 위치(kind: 0=초성, 1=중성, 2=종성)가 정해진 자모 하나를 키 시퀀스로.
/// 직접 키가 없는 겹자모는 위치에 맞는 규칙으로 분해한다
/// (초성 쌍자음은 같은 키 연타, 겹중성/겹받침은 구성 자모 두 키).
fn push_jamo_keys(out: &mut String, layout: Layout, kind: u8, jamo: char) {
    if let Some(key) = key_for_jamo(layout, kind, jamo) {
        out.push(key);
        return;
    }
    let split = match kind {
        0 => split_cho(jamo),
        1 => split_jung(jamo),
        _ => split_jong(jamo),
    };
    if let Some((a, b)) = split {
        push_jamo_keys(out, layout, kind, a);
        push_jamo_keys(out, layout, kind, b);
    } else {
        // 매핑 밖 자모(옛한글 등)는 변환 포기하고 원문 유지.
        out.push(jamo);
    }
}

/// 음절을 이루지 못한 홑자모: 위치를 알 수 없으므로 자음은 초성 키를
/// 우선하고, 없으면 종성 키를 시도한다 (세벌식에서만 모호하다).
fn push_standalone_jamo(out: &mut String, layout: Layout, jamo: char) {
    let kind = if is_vowel_jamo(jamo) || split_jung(jamo).is_some() {
        1
    } else if key_for_jamo(layout, 0, jamo).is_some() || split_cho(jamo).is_some() {
        0
    } else {
        2
    };
    push_jamo_keys(out, layout, kind, jamo);
}

/// 한글 모드로 잘못 입력된 문자열을 영문 키 시퀀스로 변환한다 (두벌식).
/// 예: `"ㅗ디ㅣㅐ"` → `"hello"`, `"한글"` → `"gksrmf"`.
pub fn hangul_to_english(input: &str) -> String {
    hangul_to_english_with(Layout::Dubeolsik, input)
}

/// 자판 배열을 지정한 역변환.
pub fn hangul_to_english_with(layout: Layout, input: &str) -> String {
    let mut out = String::new();
    for c in input.chars() {
        if let Some((cho, jung, jong_code)) = decompose_syllable(c) {
            push_jamo_keys(&mut out, layout, 0, CHOSEONG[cho]);
            push_jamo_keys(&mut out, layout, 1, JUNGSEONG[jung]);
            if jong_code > 0 {
                push_jamo_keys(&mut out, layout, 2, JONGSEONG[jong_code - 1]);
            }
        } else if is_compat_jamo(c) {
            push_standalone_jamo(&mut out, layout, c);
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::hangul_to_english;

    #[test]
    fn syllables_to_keys() {
        assert_eq!(hangul_to_english("한글"), "gksrmf");
        assert_eq!(hangul_to_english("안녕하세요"), "dkssudgktpdy");
        assert_eq!(hangul_to_english("괜찮아"), "rhoscksgdk");
        assert_eq!(hangul_to_english("닭"), "ekfr");
    }

    #[test]
    fn standalone_jamo_to_keys() {
        assert_eq!(hangul_to_english("ㅗ디ㅣㅐ"), "hello");
        assert_eq!(hangul_to_english("ㅋㅋㅋ"), "zzz");
        assert_eq!(hangul_to_english("ㅘ"), "hk"); // 겹자모 단독 입력도 분해
        assert_eq!(hangul_to_english("ㄳ"), "rt");
    }

    #[test]
    fn shift_jamo_to_uppercase() {
        assert_eq!(hangul_to_english("빨간"), "Qkfrks");
        assert_eq!(hangul_to_english("꽃"), "Rhc");
    }

    #[test]
    fn non_hangul_passthrough() {
        assert_eq!(hangul_to_english("한글 2026!"), "gksrmf 2026!");
        assert_eq!(hangul_to_english("abc"), "abc");
    }
}

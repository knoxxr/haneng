//! 임베드 사전·bigram 모델 조회 (Phase 2 감지 정확도 업그레이드).
//!
//! 데이터는 `lexicon_data.rs`(haneng-datagen이 생성)에 있다:
//! 빈도 상위 영어/한국어 사전과 문자·자모 bigram 조건부 로그확률.

use crate::hangul::{decompose_syllable, is_compat_jamo, CHOSEONG, JONGSEONG, JUNGSEONG};
use crate::lexicon_data::{EN_BIGRAM_LP, EN_WORDS, KO_JAMO_BIGRAM_LP, KO_WORDS};

/// 임베드된 영어 사전 (빈도순 아님, 정렬됨). 정확도 하네스용.
pub fn english_words() -> &'static [&'static str] {
    &EN_WORDS
}

/// 임베드된 한국어 사전 (정렬됨). 정확도 하네스용.
pub fn korean_words() -> &'static [&'static str] {
    &KO_WORDS
}

pub fn english_word_exists(word: &str) -> bool {
    let w = word.to_ascii_lowercase();
    EN_WORDS.binary_search(&w.as_str()).is_ok()
}

pub fn korean_word_exists(word: &str) -> bool {
    KO_WORDS.binary_search(&word).is_ok()
}

/// 사전 단어를 접두사로 갖는 가장 긴 길이(문자 수). 조사·어미가 붙은
/// 형태("괜찮아요" = "괜찮아" + "요")를 부분 인정하기 위한 것.
pub fn korean_longest_dict_prefix_chars(word: &str) -> usize {
    let indices: Vec<usize> = word
        .char_indices()
        .map(|(i, _)| i)
        .skip(1)
        .chain([word.len()])
        .collect();
    for &end in indices.iter().rev() {
        if korean_word_exists(&word[..end]) {
            return word[..end].chars().count();
        }
    }
    0
}

/// 단어의 문자 bigram 평균 로그확률 (ln). a-z 이외 문자가 있으면 None.
pub fn english_bigram_avg_lp(word: &str) -> Option<f64> {
    let mut symbols = Vec::with_capacity(word.len());
    for b in word.to_ascii_lowercase().bytes() {
        if !b.is_ascii_lowercase() {
            return None;
        }
        symbols.push((b - b'a' + 1) as usize);
    }
    avg_lp(
        &symbols,
        &EN_BIGRAM_LP.iter().map(|r| &r[..]).collect::<Vec<_>>(),
    )
}

/// 한글 단어(음절/자모 혼합 가능)의 자모 bigram 평균 로그확률 (ln).
/// 한글 이외 문자가 있으면 None.
pub fn korean_jamo_bigram_avg_lp(word: &str) -> Option<f64> {
    let mut symbols = Vec::new();
    for c in word.chars() {
        if let Some((cho, jung, jong)) = decompose_syllable(c) {
            symbols.push(jamo_symbol(CHOSEONG[cho]));
            symbols.push(jamo_symbol(JUNGSEONG[jung]));
            if jong > 0 {
                symbols.push(jamo_symbol(JONGSEONG[jong - 1]));
            }
        } else if is_compat_jamo(c) {
            symbols.push(jamo_symbol(c));
        } else {
            return None;
        }
    }
    avg_lp(
        &symbols,
        &KO_JAMO_BIGRAM_LP.iter().map(|r| &r[..]).collect::<Vec<_>>(),
    )
}

fn jamo_symbol(c: char) -> usize {
    (c as u32 - 0x3131 + 1) as usize
}

/// 경계(0)→…→경계(0) 전이의 평균 로그확률.
fn avg_lp(symbols: &[usize], table: &[&[i16]]) -> Option<f64> {
    if symbols.is_empty() {
        return None;
    }
    let mut sum = 0f64;
    let mut prev = 0usize;
    for &s in symbols {
        sum += table[prev][s] as f64 / 256.0;
        prev = s;
    }
    sum += table[prev][0] as f64 / 256.0;
    Some(sum / (symbols.len() + 1) as f64)
}

/// 평균 로그확률을 0..1 점수로 사상. 경험적 범위: 흔한 단어 ≈ -3.5,
/// 무작위/오타 시퀀스 ≈ -9 이하.
pub fn normalize_lp(avg: f64) -> f64 {
    ((avg + 8.0) / 4.5).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionaries_contain_common_words() {
        for w in ["hello", "the", "keyboard", "working"] {
            assert!(english_word_exists(w), "EN dict missing {w}");
        }
        for w in ["한글", "안녕하세요", "사랑", "괜찮아"] {
            assert!(korean_word_exists(w), "KO dict missing {w}");
        }
        assert!(!english_word_exists("gksrmf"));
        assert!(!korean_word_exists("ㅗ디ㅣㅐ"));
    }

    #[test]
    #[ignore = "점수 분포 관찰용: cargo test -p haneng-core score_survey -- --ignored --nocapture"]
    fn score_survey() {
        for w in [
            "hello", "working", "sos", "vodka", "ska", "dkssud", "gksrmf", "tkfkd", "rk", "djeldi",
            "dlqfur",
        ] {
            println!(
                "EN {w:12} dict={} lp={:?} score={:?}",
                english_word_exists(w),
                english_bigram_avg_lp(w),
                english_bigram_avg_lp(w).map(normalize_lp)
            );
        }
        for w in [
            "한글",
            "안녕하세요",
            "어디야",
            "괜찮아요",
            "낸",
            "가",
            "패암",
            "재가",
            "ㅗ디ㅣㅐ",
            "ㅋㅋㅋ",
            "내ㅡㄷ",
        ] {
            println!(
                "KO {w:8} dict={} prefix={} lp={:?} score={:?}",
                korean_word_exists(w),
                korean_longest_dict_prefix_chars(w),
                korean_jamo_bigram_avg_lp(w),
                korean_jamo_bigram_avg_lp(w).map(normalize_lp)
            );
        }
    }
}

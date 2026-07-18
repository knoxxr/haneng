//! 잘못된 입력 모드 감지 — v2 (구조 게이트 + 사전 + bigram).
//!
//! 단어 하나를 두 가설("현재 모드가 맞다" / "모드가 틀렸다")로 놓고
//! 양쪽 증거를 비교한다.
//!
//! 1차 게이트는 **한글 구조 검사**:
//! - 영문 단어가 한글이려면 반대 변환 결과가 전부 완성형 음절이어야 한다.
//! - 완성형으로만 이뤄진 한글을 영문으로 바꾸는 것은 가장 위험한 방향이라
//!   더 엄격한 조건(사전 일치 + 추가 마진)을 요구한다.
//!
//! 2차는 **증거 비교**. 말뭉치 관찰 결과(lexicon.rs의 score_survey):
//! bigram 모델은 잘 조합된 가짜("패암", "djeldi")를 걸러내지 못하므로
//! **음성 필터로만** 쓰고, 양성 증거는 사전·접두 일치가 담당한다.
//! `양성 증거 최소치(FLOOR)`와 `증거 차이 마진(민감도별)`을 모두 넘어야
//! 변환한다.

use crate::compose::english_to_hangul_with;
use crate::decompose::hangul_to_english_with;
use crate::hangul::{is_compat_jamo, is_syllable};
use crate::layout::{is_word_key, Layout};
use crate::lexicon;
use std::collections::HashSet;

/// 단어 하나에 대한 판정 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// 그대로 둔다.
    Keep,
    /// 영문 모드로 잘못 입력된 한글 → 변환 결과.
    ToHangul(String),
    /// 한글 모드로 잘못 입력된 영문 → 변환 결과.
    ToEnglish(String),
}

/// 자동 변환 민감도 (PLAN.md F3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sensitivity {
    /// 확실할 때만 변환 (오발동 최소화).
    Conservative,
    #[default]
    Balanced,
    /// 적극 변환 (미탐 최소화).
    Aggressive,
}

impl Sensitivity {
    /// 한글로 변환하려면 (한국어 증거 − 영어 증거)가 이 값 이상이어야 한다.
    fn to_hangul_margin(self) -> f64 {
        match self {
            Sensitivity::Conservative => 0.40,
            Sensitivity::Balanced => 0.20,
            Sensitivity::Aggressive => 0.08,
        }
    }

    /// 영문으로 변환하려면 (영어 증거 − 한국어 증거)가 이 값 이상이어야 한다.
    fn to_english_margin(self) -> f64 {
        match self {
            Sensitivity::Conservative => 0.45,
            Sensitivity::Balanced => 0.25,
            Sensitivity::Aggressive => 0.10,
        }
    }
}

/// 변환 대상 언어 쪽 증거의 절대 최소치 — 마진과 무관하게 이보다 약하면
/// 변환하지 않는다 ("ㅋㅋㅋ"→"zzz" 같은 저증거 변환 차단).
const TARGET_EVIDENCE_FLOOR: f64 = 0.55;
/// 완성형으로만 이뤄진 한글을 영문으로 바꿀 때 추가로 요구하는 마진.
const FULLY_COMPOSED_EXTRA_MARGIN: f64 = 0.15;

/// 단어 단위 감지기. 사용자 예외 사전(undo 학습)을 보유한다.
#[derive(Debug, Default)]
pub struct Detector {
    sensitivity: Sensitivity,
    layout: Layout,
    exceptions: HashSet<String>,
}

impl Detector {
    /// 두벌식 감지기.
    pub fn new(sensitivity: Sensitivity) -> Self {
        Self::with_layout(sensitivity, Layout::Dubeolsik)
    }

    pub fn with_layout(sensitivity: Sensitivity, layout: Layout) -> Self {
        Self {
            sensitivity,
            layout,
            exceptions: HashSet::new(),
        }
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// 사용자가 자동 변환을 되돌린 단어를 학습해 다시 건드리지 않는다.
    pub fn record_undo(&mut self, word: &str) {
        self.exceptions.insert(word.to_string());
    }

    pub fn is_exception(&self, word: &str) -> bool {
        self.exceptions.contains(word)
    }

    /// 단어 경계에서 확정된 단어 하나를 판정한다.
    pub fn analyze(&self, word: &str) -> Verdict {
        let word = word.trim();
        if word.is_empty() || self.exceptions.contains(word) {
            return Verdict::Keep;
        }

        let has_latin = word.chars().any(|c| c.is_ascii_alphabetic());
        let has_hangul = word.chars().any(|c| is_syllable(c) || is_compat_jamo(c));

        match (has_latin, has_hangul) {
            // 두 문자 체계가 섞인 단어는 의도적 입력일 가능성이 높다.
            (true, true) | (false, false) => Verdict::Keep,
            (true, false) => self.analyze_latin(word),
            (false, true) => self.analyze_hangul(word),
        }
    }

    /// 영문 단어: 한글로 완전 조합되고, 한국어 증거가 영어 증거를
    /// 마진 이상 앞서면 변환.
    fn analyze_latin(&self, word: &str) -> Verdict {
        // 세벌식에서는 숫자·문장부호 키도 자모를 낸다 — 자판의 단어 키만 허용.
        if !word
            .chars()
            .all(|c| c.is_ascii_alphabetic() || is_word_key(self.layout, c))
        {
            return Verdict::Keep;
        }
        let candidate = english_to_hangul_with(self.layout, word);
        let fully_syllabic = !candidate.is_empty() && candidate.chars().all(is_syllable);
        if !fully_syllabic {
            return Verdict::Keep;
        }
        // 진짜 영어 단어는 절대 건드리지 않는다 (사전 우선).
        if lexicon::english_word_exists(word) {
            return Verdict::Keep;
        }
        let korean = korean_evidence(&candidate);
        let mut english = english_score(word);
        // 사전에 없는 4자 이상 키열이 사전 한국어 단어로 정확히 조합되면,
        // "영어처럼 보임" 휴리스틱보다 사전 증거를 우선한다. (완전 조합되는
        // 비사전 영어 단어는 실측상 3자 이하 — "ska", "sos" — 라서 안전.)
        if korean == 1.0 && word.chars().count() >= 4 {
            english = english.min(0.75);
        }
        if korean >= TARGET_EVIDENCE_FLOOR
            && korean - english >= self.sensitivity.to_hangul_margin()
        {
            Verdict::ToHangul(candidate)
        } else {
            Verdict::Keep
        }
    }

    /// 한글 단어: 역변환이 영어답고, 영어 증거가 한국어 증거를 마진 이상
    /// 앞서면 변환. 완성형으로만 조합된 단어는 더 엄격하게(사전 일치 필수).
    fn analyze_hangul(&self, word: &str) -> Verdict {
        // 진짜 한국어 단어는 절대 건드리지 않는다 (사전 우선).
        if lexicon::korean_word_exists(word) {
            return Verdict::Keep;
        }
        let candidate = hangul_to_english_with(self.layout, word);
        if candidate.is_empty() || !candidate.chars().all(|c| c.is_ascii_alphabetic()) {
            return Verdict::Keep;
        }
        let broken = word.chars().any(is_compat_jamo);
        let english = if lexicon::english_word_exists(&candidate) {
            1.0
        } else {
            english_score(&candidate)
        };
        let korean = korean_evidence(word);

        let mut margin = self.sensitivity.to_english_margin();
        if !broken {
            // 완성형으로만 이뤄진 한글은 진짜 한국어일 확률이 높다:
            // 영어 사전에 있는 단어로 복원될 때만, 추가 마진을 두고 변환.
            if english < 1.0 {
                return Verdict::Keep;
            }
            margin += FULLY_COMPOSED_EXTRA_MARGIN;
        }
        if english >= TARGET_EVIDENCE_FLOOR && english - korean >= margin {
            Verdict::ToEnglish(candidate)
        } else {
            Verdict::Keep
        }
    }
}

/// 한국어 증거 0..1. 사전 일치(1.0) > 사전 접두 일치(커버리지 비례)
/// > 자모 bigram(음성 필터 성격이라 0.6으로 상한).
fn korean_evidence(word: &str) -> f64 {
    if lexicon::korean_word_exists(word) {
        return 1.0;
    }
    let total = word.chars().count();
    let prefix = lexicon::korean_longest_dict_prefix_chars(word);
    let coverage = prefix as f64 / total as f64;
    let prefix_evidence = if total >= 2 && coverage >= 0.5 {
        0.6 + 0.35 * coverage
    } else {
        0.0
    };
    let bigram_evidence = lexicon::korean_jamo_bigram_avg_lp(word)
        .map(|lp| 0.6 * lexicon::normalize_lp(lp))
        .unwrap_or(0.0);
    prefix_evidence.max(bigram_evidence)
}

/// 어두에 올 수 있는 영어 자음군 (2~3자).
const ENGLISH_ONSETS: &[&str] = &[
    "bl", "br", "ch", "cl", "cr", "cz", "dr", "dw", "fl", "fr", "gh", "gl", "gn", "gr", "kn", "ph",
    "pl", "pr", "ps", "qu", "rh", "sc", "sh", "sk", "sl", "sm", "sn", "sp", "st", "sw", "th", "tr",
    "ts", "tw", "wh", "wr", "chr", "phr", "sch", "scr", "shr", "sph", "spl", "spr", "squ", "str",
    "thr",
];

fn is_english_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}

/// 문자열이 영어 단어답게 생겼는지 0.0(아님)~1.0(그럴듯함)으로 점수화한다.
///
/// 특징 3가지의 가중합:
/// - 모음 비율 (영어 단어는 대략 0.25~0.6): 가중치 0.40
/// - 최장 자음 연쇄 (짧을수록 영어다움): 가중치 0.25
/// - 어두 자음군의 영어 적법성 ("dk-", "gk-"는 영어에 없음): 가중치 0.35
pub fn english_score(word: &str) -> f64 {
    let letters: Vec<char> = word
        .chars()
        .filter(char::is_ascii_alphabetic)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if letters.is_empty() {
        return 0.0;
    }

    let vowel_count = letters.iter().filter(|&&c| is_english_vowel(c)).count();
    let ratio = vowel_count as f64 / letters.len() as f64;
    let ratio_score = if ratio == 0.0 {
        0.0
    } else if ratio < 0.25 {
        ratio / 0.25
    } else if ratio <= 0.6 {
        1.0
    } else {
        (1.0 - (ratio - 0.6) / 0.4).max(0.0)
    };

    let mut max_run = 0usize;
    let mut run = 0usize;
    for &c in &letters {
        if is_english_vowel(c) {
            run = 0;
        } else {
            run += 1;
            max_run = max_run.max(run);
        }
    }
    let run_score = match max_run {
        0..=3 => 1.0,
        4 => 0.5,
        5 => 0.25,
        _ => 0.0,
    };

    let onset: String = letters
        .iter()
        .take_while(|&&c| !is_english_vowel(c))
        .collect();
    let onset_score = if onset.len() <= 1 || ENGLISH_ONSETS.contains(&onset.as_str()) {
        1.0
    } else {
        0.0
    };

    let mut score = 0.40 * ratio_score + 0.25 * run_score + 0.35 * onset_score;
    // 극단 신호는 다른 특징으로 만회할 수 없게 상한을 건다:
    // 모음이 하나도 없거나 자음이 6연쇄 이상이면 영어 단어가 아니다.
    if vowel_count == 0 {
        score = score.min(0.20);
    }
    if max_run >= 6 {
        score = score.min(0.30);
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detector() -> Detector {
        Detector::new(Sensitivity::Balanced)
    }

    #[test]
    fn converts_korean_typed_in_english_mode() {
        for (input, expected) in [
            ("gksrmf", "한글"),
            ("dkssudgktpdy", "안녕하세요"),
            ("tkfkd", "사랑"),
            ("tnrh", "수고"),
            ("rhoscksgdk", "괜찮아"),
        ] {
            assert_eq!(
                detector().analyze(input),
                Verdict::ToHangul(expected.to_string()),
                "input: {input}"
            );
        }
    }

    #[test]
    fn converts_english_typed_in_korean_mode() {
        // 입력은 한글 모드에서 hello/some/working을 쳤을 때 실제 화면에 남는 문자열.
        for (input, expected) in [
            ("ㅗ디ㅣㅐ", "hello"),
            ("내ㅡㄷ", "some"),
            ("재가ㅑㅜㅎ", "working"),
        ] {
            assert_eq!(
                detector().analyze(input),
                Verdict::ToEnglish(expected.to_string()),
                "input: {input}"
            );
        }
    }

    #[test]
    fn keeps_real_english() {
        for word in ["hello", "world", "sos", "sock", "vodka", "strengths", "go"] {
            assert_eq!(detector().analyze(word), Verdict::Keep, "word: {word}");
        }
    }

    #[test]
    fn keeps_real_korean_and_intentional_jamo() {
        // 완성형 한글, 의도적 자모 입력(ㅋㅋㅋ, ㅠㅠ)은 건드리지 않는다.
        for word in ["한글", "안녕하세요", "책상", "ㅋㅋㅋ", "ㅠㅠ", "ㄱㅅ"] {
            assert_eq!(detector().analyze(word), Verdict::Keep, "word: {word}");
        }
    }

    #[test]
    fn keeps_mixed_and_non_letter_words() {
        for word in ["test한글", "abc123", "2026", "", "   "] {
            assert_eq!(detector().analyze(word), Verdict::Keep, "word: {word:?}");
        }
    }

    #[test]
    fn undo_learning_suppresses_reconversion() {
        let mut d = detector();
        assert!(matches!(d.analyze("gksrmf"), Verdict::ToHangul(_)));
        d.record_undo("gksrmf");
        assert_eq!(d.analyze("gksrmf"), Verdict::Keep);
    }

    #[test]
    fn sensitivity_margins_are_ordered() {
        assert!(
            Sensitivity::Conservative.to_hangul_margin() > Sensitivity::Balanced.to_hangul_margin()
        );
        assert!(
            Sensitivity::Balanced.to_hangul_margin() > Sensitivity::Aggressive.to_hangul_margin()
        );
        assert!(
            Sensitivity::Conservative.to_english_margin()
                > Sensitivity::Balanced.to_english_margin()
        );
        assert!(
            Sensitivity::Balanced.to_english_margin() > Sensitivity::Aggressive.to_english_margin()
        );
    }

    #[test]
    fn dictionary_evidence_resolves_v1_ambiguities() {
        // v1 한계였던 케이스: "djeldi"(어디야)는 영어 휴리스틱 점수가 애매해
        // 놓쳤지만, 사전 증거(어디야)로 이제 변환된다.
        assert_eq!(
            detector().analyze("djeldi"),
            Verdict::ToHangul("어디야".to_string())
        );
        // 반대로 사전에 있는 진짜 영어 단어는 한글로 완전 조합돼도 유지.
        // ("ska" → "남"처럼 위험한 충돌은 사전/마진이 막는다.)
        assert_eq!(detector().analyze("ska"), Verdict::Keep);
    }

    #[test]
    fn english_score_sanity() {
        assert!(english_score("hello") > 0.8);
        assert!(english_score("gksrmf") < 0.1);
        assert!(english_score("zzz") < 0.5);
        assert!(english_score("") == 0.0);
    }
}

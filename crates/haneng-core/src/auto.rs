//! Phase 2 — 단어 경계 자동 감지·교정 (플랫폼 공용 오케스트레이션).
//!
//! 어댑터는 경계 키(공백·문장부호)로 단어가 확정되는 순간
//! [`AutoCorrector::on_word_committed`]를 호출한다. 반환된 결정에는 치환
//! 계획뿐 아니라 되돌리기(undo) 재주입 원문과 예외 학습용 단어가 담긴다.
//!
//! 되돌리기 규약: 자동 교정 직후 사용자의 백스페이스 1회는 "교정 취소"다.
//! 사용자 백스페이스가 경계 문자를 이미 지웠으므로, 어댑터는
//! `replacement 글자 수 - 1`회를 더 지우고 [`AutoDecision::revert`]를
//! 재주입한 뒤, [`AutoDecision::screen_word`]를 예외 사전에 학습시킨다.

use crate::compose::english_to_hangul_with;
use crate::detect::{Detector, Verdict};

/// 자동 교정 결정.
#[derive(Debug, PartialEq, Eq)]
pub struct AutoDecision {
    /// 화면에서 지울 글자 수 (단어 + 경계 문자 1).
    pub backspaces: usize,
    /// 주입할 텍스트 (변환된 단어 + 경계 문자).
    pub replacement: String,
    /// 되돌리기 시 재주입할 원문 (원래 화면 단어 + 경계 문자).
    pub revert: String,
    /// 교정 후 전환할 IME 모드 (항상 현재 모드의 반대).
    pub to_korean_mode: bool,
    /// 예외 학습용 단어 — `Detector::analyze`에 들어간 화면 단어와 동일.
    pub screen_word: String,
}

pub struct AutoCorrector {
    detector: Detector,
}

impl AutoCorrector {
    pub fn new(detector: Detector) -> Self {
        Self { detector }
    }

    /// undo 학습 등 감지기 상태 변경용.
    pub fn detector_mut(&mut self) -> &mut Detector {
        &mut self.detector
    }

    /// 경계 문자로 확정된 단어를 판정한다. `keys`는 물리 키 시퀀스,
    /// `korean_mode`는 그 단어를 입력할 때의 IME 모드.
    pub fn on_word_committed(
        &self,
        korean_mode: bool,
        keys: &str,
        boundary: char,
    ) -> Option<AutoDecision> {
        // 화면에 실제로 남은 단어: 한글 모드였다면 자판 배열대로 조합된 결과.
        let screen_word = if korean_mode {
            english_to_hangul_with(self.detector.layout(), keys)
        } else {
            keys.to_string()
        };
        let (converted, to_korean_mode) = match self.detector.analyze(&screen_word) {
            Verdict::Keep => return None,
            Verdict::ToHangul(h) => (h, true),
            Verdict::ToEnglish(e) => (e, false),
        };

        let mut replacement = converted;
        replacement.push(boundary);
        let mut revert = screen_word.clone();
        revert.push(boundary);
        Some(AutoDecision {
            backspaces: screen_word.chars().count() + 1,
            replacement,
            revert,
            to_korean_mode,
            screen_word,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::Sensitivity;

    fn corrector() -> AutoCorrector {
        AutoCorrector::new(Detector::new(Sensitivity::Balanced))
    }

    #[test]
    fn corrects_korean_typed_in_english_mode() {
        let d = corrector()
            .on_word_committed(false, "gksrmf", ' ')
            .expect("must convert");
        assert_eq!(
            d,
            AutoDecision {
                backspaces: 7, // "gksrmf" 6자 + 공백
                replacement: "한글 ".into(),
                revert: "gksrmf ".into(),
                to_korean_mode: true,
                screen_word: "gksrmf".into(),
            }
        );
    }

    #[test]
    fn corrects_english_typed_in_korean_mode() {
        let d = corrector()
            .on_word_committed(true, "hello", '.')
            .expect("must convert");
        // 화면에는 "ㅗ디ㅣㅐ."가 남아 있다 (경계가 preedit를 커밋).
        assert_eq!(
            d,
            AutoDecision {
                backspaces: 5,
                replacement: "hello.".into(),
                revert: "ㅗ디ㅣㅐ.".into(),
                to_korean_mode: false,
                screen_word: "ㅗ디ㅣㅐ".into(),
            }
        );
    }

    #[test]
    fn keeps_correct_input_in_both_modes() {
        assert_eq!(corrector().on_word_committed(false, "hello", ' '), None);
        // 한글 모드에서 "안녕" 입력 (키: dkssud) → 화면 "안녕" → 유지.
        assert_eq!(corrector().on_word_committed(true, "dkssud", ' '), None);
        // 의도적 자모 (ㅋㅋㅋ = zzz 키) → 유지.
        assert_eq!(corrector().on_word_committed(true, "zzz", ' '), None);
    }

    #[test]
    fn undo_learning_prevents_reconversion() {
        let mut c = corrector();
        let d = c.on_word_committed(false, "gksrmf", ' ').unwrap();
        c.detector_mut().record_undo(&d.screen_word);
        assert_eq!(c.on_word_committed(false, "gksrmf", ' '), None);
    }
}

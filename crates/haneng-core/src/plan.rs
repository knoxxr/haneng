//! 플랫폼 공용 — 핫키 변환의 치환 계획 계산.
//!
//! "화면에서 몇 글자를 지우고 무엇을 입력할지"는 OS와 무관하게 두벌식
//! 조합 규칙만으로 결정된다. 백스페이스 횟수 계산은 한글 IME(macOS 한글,
//! MS 한글 IME)의 공통 동작을 따른다: 커밋된 음절은 1회에 1음절,
//! 조합 중(preedit)인 음절은 1회에 자모 키 1개씩 지워진다.

use crate::compose::{english_to_hangul_with, Composer};
use crate::layout::Layout;
use crate::tracker::Target;

/// 치환 실행 계획: 백스페이스 횟수와 이어서 타이핑할 텍스트.
#[derive(Debug, PartialEq, Eq)]
pub struct ReplacePlan {
    pub backspaces: usize,
    pub replacement: String,
}

/// 현재 IME 모드(korean_mode)와 변환 대상으로부터 치환 계획을 만든다.
pub fn build_replace_plan(layout: Layout, korean_mode: bool, target: &Target) -> ReplacePlan {
    let (keys, boundary, injected) = match target {
        Target::Current(keys, injected) => (keys.as_str(), None, *injected),
        Target::Committed(keys, b) => (keys.as_str(), Some(*b), false),
    };

    let (backspaces, mut replacement) = if korean_mode {
        // 화면: 자판 배열대로 조합된 결과. 지우고 원래 친 영문 키를 그대로 입력.
        let backspaces = if boundary.is_none() && !injected {
            // 마지막 음절은 아직 preedit — 백스페이스가 자모 단위로 먹는다.
            let mut composer = Composer::with_layout(layout);
            for key in keys.chars() {
                composer.feed_key(key);
            }
            composer.committed().chars().count() + composer.pending_key_count()
        } else {
            // 경계 입력 또는 직전 주입으로 전부 커밋된 상태.
            english_to_hangul_with(layout, keys).chars().count()
        };
        (backspaces, keys.to_string())
    } else {
        // 화면: 영문 그대로. 지우고 한글 조합 결과 입력.
        (keys.chars().count(), english_to_hangul_with(layout, keys))
    };

    let mut backspaces = backspaces;
    if let Some(b) = boundary {
        backspaces += 1;
        replacement.push(b);
    }
    ReplacePlan {
        backspaces,
        replacement,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Layout;

    #[test]
    fn english_mode_current_word() {
        // 영문 모드에서 "gksrmf" 입력 → 6자 지우고 "한글" 주입.
        let plan = build_replace_plan(
            Layout::Dubeolsik,
            false,
            &Target::Current("gksrmf".into(), false),
        );
        assert_eq!(
            plan,
            ReplacePlan {
                backspaces: 6,
                replacement: "한글".into()
            }
        );
    }

    #[test]
    fn korean_mode_current_word_counts_preedit() {
        // 한글 모드에서 hello 입력 → 화면 "ㅗ디ㅣㅐ"(ㅐ는 preedit 1키):
        // 커밋 3자 + preedit 1키 = 백스페이스 4회, "hello" 주입.
        let plan = build_replace_plan(
            Layout::Dubeolsik,
            true,
            &Target::Current("hello".into(), false),
        );
        assert_eq!(
            plan,
            ReplacePlan {
                backspaces: 4,
                replacement: "hello".into()
            }
        );
    }

    #[test]
    fn committed_word_deletes_boundary_too() {
        // "gksrmf" + 공백 확정 후: 한글 모드 화면 "한글 " → 3회 지우고 "gksrmf ".
        let plan = build_replace_plan(
            Layout::Dubeolsik,
            true,
            &Target::Committed("gksrmf".into(), ' '),
        );
        assert_eq!(
            plan,
            ReplacePlan {
                backspaces: 3,
                replacement: "gksrmf ".into()
            }
        );
    }

    #[test]
    fn reinjected_text_is_fully_committed() {
        // 핫키 토글: 주입된 "한글"은 preedit가 없다 → 2회 지우고 "gksrmf".
        let plan = build_replace_plan(
            Layout::Dubeolsik,
            true,
            &Target::Current("gksrmf".into(), true),
        );
        assert_eq!(
            plan,
            ReplacePlan {
                backspaces: 2,
                replacement: "gksrmf".into()
            }
        );
    }
}

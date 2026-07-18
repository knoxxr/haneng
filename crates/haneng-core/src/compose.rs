//! 두벌식 입력 오토마타: 영문 키 시퀀스 → 한글.
//!
//! 실제 두벌식 IME와 동일한 규칙으로 조합한다:
//! - 초성 + 중성 (+ 종성) 순서로 음절을 쌓는다.
//! - 겹중성(ㅗ+ㅏ=ㅘ)·겹받침(ㄹ+ㄱ=ㄺ)을 결합한다.
//! - 종성 뒤에 모음이 오면 종성(겹받침이면 뒤 자모)이 다음 음절 초성으로
//!   넘어간다 (도깨비불 현상).
//! - 음절을 이루지 못한 자모는 호환 자모로 그대로 남는다 (예: "ㅗ디ㅣㅐ").

use crate::hangul::{
    cho_index, combine_cho, combine_jong, combine_jung, compose_syllable, is_vowel_jamo,
    jong_index, jung_index, split_jong, split_jung,
};
use crate::layout::{key_jamo, KeyJamo, Layout};

/// 자모 스트림을 받아 한글을 조합하는 상태 기계.
/// 플랫폼 어댑터가 키 이벤트를 증분 공급할 수 있도록 공개한다.
#[derive(Debug, Default)]
pub struct Composer {
    layout: Layout,
    cho: Option<char>,
    jung: Option<char>,
    jong: Option<char>,
    /// 종성이 위치 명시 키(세벌식)로 입력됐는가 — 그렇다면 뒤따르는
    /// 모음에 도깨비불을 적용하지 않는다.
    jong_explicit: bool,
    out: String,
}

impl Composer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_layout(layout: Layout) -> Self {
        Self {
            layout,
            ..Self::default()
        }
    }

    /// 물리 키 문자 하나를 자판 배열에 따라 해석해 공급한다.
    pub fn feed_key(&mut self, key: char) {
        match key_jamo(self.layout, key) {
            Some(KeyJamo::Dual(c)) => self.feed_consonant(c),
            Some(KeyJamo::Jung(v)) => self.feed_vowel(v),
            Some(KeyJamo::Cho(c)) => self.feed_cho_explicit(c),
            Some(KeyJamo::Jong(t)) => self.feed_jong_explicit(t),
            Some(KeyJamo::Other(c)) => self.feed_passthrough(c),
            None => self.feed_passthrough(key),
        }
    }

    /// 조합 중인 음절을 확정해 출력 버퍼로 내보낸다.
    fn flush(&mut self) {
        let (cho, jung, jong) = (self.cho.take(), self.jung.take(), self.jong.take());
        self.jong_explicit = false;
        if let (Some(c), Some(v)) = (cho, jung) {
            let ci = cho_index(c).expect("cho slot holds a valid choseong");
            let vi = jung_index(v).expect("jung slot holds a valid jungseong");
            let jong_code = jong
                .map(|j| jong_index(j).expect("jong slot holds a valid jongseong") + 1)
                .unwrap_or(0);
            self.out.push(compose_syllable(ci, vi, jong_code));
            return;
        }
        // 음절을 이루지 못한 자모는 홑자모로 확정 (세벌식은 종성 단독도 가능).
        for jamo in [cho, jung, jong].into_iter().flatten() {
            self.out.push(jamo);
        }
    }

    /// 두벌식 자모 하나를 공급한다.
    pub fn feed_jamo(&mut self, jamo: char) {
        if is_vowel_jamo(jamo) {
            self.feed_vowel(jamo);
        } else {
            self.feed_consonant(jamo);
        }
    }

    fn feed_vowel(&mut self, v: char) {
        if self.jong.is_some() && self.jong_explicit {
            // 세벌식: 종성은 위치가 명시돼 있으므로 도깨비불이 없다.
            self.flush();
            self.jung = Some(v);
        } else if let Some(jong) = self.jong.take() {
            // 도깨비불: 종성(겹받침이면 뒤 자모)이 다음 음절 초성으로 이동.
            let (stay, mover) = match split_jong(jong) {
                Some((first, second)) => (Some(first), second),
                None => (None, jong),
            };
            self.jong = stay;
            self.flush();
            self.cho = Some(mover);
            self.jung = Some(v);
        } else if let Some(cur) = self.jung {
            if let Some(compound) = combine_jung(cur, v) {
                self.jung = Some(compound);
            } else {
                self.flush();
                self.jung = Some(v);
            }
        } else {
            self.jung = Some(v);
        }
    }

    fn feed_consonant(&mut self, c: char) {
        if self.jung.is_none() {
            // 초성만 있거나 빈 상태: 자음이 연달아 오면 앞 자음은 홑자모로 확정.
            if self.cho.is_some() {
                self.flush();
            }
            self.cho = Some(c);
        } else if self.cho.is_none() {
            // 홑모음 상태에서 자음: 모음을 확정하고 새 음절 시작.
            self.flush();
            self.cho = Some(c);
        } else if let Some(cur) = self.jong {
            if let Some(compound) = combine_jong(cur, c) {
                self.jong = Some(compound);
            } else {
                self.flush();
                self.cho = Some(c);
            }
        } else if jong_index(c).is_some() {
            self.jong = Some(c);
        } else {
            // ㄸ·ㅃ·ㅉ은 종성이 될 수 없다.
            self.flush();
            self.cho = Some(c);
        }
    }

    /// 세벌식 초성 키: 새 음절 시작 또는 초성 채움. 같은 초성 연타는
    /// 쌍자음으로 조합한다 (390에는 쌍자음 직접 키가 없다).
    fn feed_cho_explicit(&mut self, c: char) {
        if self.jung.is_none() && self.jong.is_none() {
            if let Some(double) = self.cho.and_then(|cur| combine_cho(cur, c)) {
                self.cho = Some(double);
                return;
            }
        }
        if self.cho.is_some() || self.jung.is_some() || self.jong.is_some() {
            self.flush();
        }
        self.cho = Some(c);
    }

    /// 세벌식 종성 키: 초성+중성이 갖춰진 음절에만 붙는다.
    fn feed_jong_explicit(&mut self, t: char) {
        let attachable = self.cho.is_some() && self.jung.is_some();
        if attachable && self.jong.is_none() {
            self.jong = Some(t);
            self.jong_explicit = true;
        } else if let Some(compound) = self.jong.and_then(|cur| combine_jong(cur, t)) {
            self.jong = Some(compound);
            self.jong_explicit = true;
        } else {
            // 붙을 자리가 없으면 지금까지를 확정하고 홑자모로 남긴다.
            self.flush();
            self.jong = Some(t);
            self.jong_explicit = true;
        }
    }

    /// 자판 밖 문자(공백·숫자·기호)는 조합을 끊고 그대로 내보낸다.
    pub fn feed_passthrough(&mut self, c: char) {
        self.flush();
        self.out.push(c);
    }

    /// 남은 조합을 확정하고 결과를 반환한다.
    pub fn finish(mut self) -> String {
        self.flush();
        self.out
    }

    /// 확정(커밋)된 출력. 조합 중인 음절은 포함하지 않는다.
    pub fn committed(&self) -> &str {
        &self.out
    }

    /// 조합 중(미확정) 상태에 들어간 키 입력 수. 실제 IME의 preedit에서
    /// 백스페이스 1회가 지우는 단위가 자모 키 1개이므로, 플랫폼 어댑터가
    /// 화면 텍스트를 지울 백스페이스 횟수를 계산할 때 쓴다:
    /// `committed().chars().count() + pending_key_count()`.
    pub fn pending_key_count(&self) -> usize {
        let jamo_keys = |c: char, split: fn(char) -> Option<(char, char)>| {
            if split(c).is_some() {
                2
            } else {
                1
            }
        };
        self.cho.map_or(0, |_| 1)
            + self.jung.map_or(0, |j| jamo_keys(j, split_jung))
            + self.jong.map_or(0, |j| jamo_keys(j, split_jong))
    }
}

/// 영문 모드로 잘못 입력된 문자열을 한글로 변환한다 (두벌식).
/// 예: `"gksrmf"` → `"한글"`.
pub fn english_to_hangul(input: &str) -> String {
    english_to_hangul_with(Layout::Dubeolsik, input)
}

/// 자판 배열을 지정한 변환 — 세벌식에서는 숫자·문장부호 키도 자모가 된다.
pub fn english_to_hangul_with(layout: Layout, input: &str) -> String {
    let mut composer = Composer::with_layout(layout);
    for c in input.chars() {
        composer.feed_key(c);
    }
    composer.finish()
}

#[cfg(test)]
mod tests {
    use super::english_to_hangul;

    #[test]
    fn basic_words() {
        assert_eq!(english_to_hangul("gksrmf"), "한글");
        assert_eq!(english_to_hangul("dkssudgktpdy"), "안녕하세요");
        assert_eq!(english_to_hangul("gksrmfdlqfur"), "한글입력");
        assert_eq!(english_to_hangul("quf"), "별");
    }

    #[test]
    fn dokkaebibul_moves_jongseong() {
        // 달 + ㄱㅑㄹ: ㄹ+ㄱ이 겹받침 ㄺ이 됐다가 모음 앞에서 ㄱ만 이동.
        assert_eq!(english_to_hangul("ekfrif"), "달걀");
        // 겹받침 유지: 닭 + 과
        assert_eq!(english_to_hangul("ekfrrhk"), "닭과");
        // 홑받침 전체 이동: 사 + 랑
        assert_eq!(english_to_hangul("tkfkd"), "사랑");
    }

    #[test]
    fn compound_vowels_and_jongseong() {
        assert_eq!(english_to_hangul("rhoscksgdk"), "괜찮아");
        assert_eq!(english_to_hangul("dml"), "의");
        assert_eq!(english_to_hangul("djqt"), "없");
    }

    #[test]
    fn shift_keys() {
        assert_eq!(english_to_hangul("Qkfrks"), "빨간");
        assert_eq!(english_to_hangul("Rhc"), "꽃");
        // Shift 자모가 없는 대문자는 소문자와 동일.
        assert_eq!(english_to_hangul("GKS"), "한");
    }

    #[test]
    fn incomplete_jamo_stay_standalone() {
        assert_eq!(english_to_hangul("r"), "ㄱ");
        assert_eq!(english_to_hangul("rt"), "ㄱㅅ");
        assert_eq!(english_to_hangul("hk"), "ㅘ");
        assert_eq!(english_to_hangul("zzz"), "ㅋㅋㅋ");
    }

    #[test]
    fn committed_and_pending_track_ime_preedit() {
        use crate::layout::key_to_jamo;
        let mut c = super::Composer::new();
        for key in "tkfk".chars() {
            c.feed_jamo(key_to_jamo(key).unwrap());
        }
        // "사라"까지 입력: 도깨비불로 "사"는 커밋, "라"(ㄹ+ㅏ 2키)는 조합 중.
        assert_eq!(c.committed(), "사");
        assert_eq!(c.pending_key_count(), 2);

        let mut c = super::Composer::new();
        for key in "rhos".chars() {
            c.feed_jamo(key_to_jamo(key).unwrap());
        }
        // "괜" 조합 중: ㄱ(1) + ㅙ(2키) + ㄴ(1) = 4키.
        assert_eq!(c.committed(), "");
        assert_eq!(c.pending_key_count(), 4);
    }

    #[test]
    fn passthrough_breaks_composition() {
        assert_eq!(english_to_hangul("gks rmf"), "한 글");
        assert_eq!(english_to_hangul("gks1rmf!"), "한1글!");
    }
}

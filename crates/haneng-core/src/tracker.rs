//! 플랫폼 공용 — 물리 키 분류와 마지막 단어 추적.
//!
//! 어댑터는 OS 키코드를 [`KeyClass`]로 분류해 [`WordBuffer`]에 공급한다.
//! 활성 IME와 무관하게 "어떤 QWERTY 키를 눌렀는가"만 기록하고, 한글 모드였다면
//! 화면에 무엇이 조합됐는지는 [`crate::Composer`]가 재현한다.
//!
//! 프라이버시 원칙(PLAN.md N2): 입력 중인 단어와 직전 단어 하나만 메모리에
//! 유지하고, 그 밖의 키 입력은 어디에도 기록하지 않는다.

/// 물리 키 하나의 분류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyClass {
    /// 단어를 이루는 글쇠 (두벌식 자모가 배정된 라틴 문자).
    Letter(char),
    /// 단어를 확정하는 경계 — 지웠다 다시 입력해도 안전한 문자(공백·문장부호·숫자).
    Boundary(char),
    Backspace,
    /// 엔터·커서 이동 등 버퍼 추적을 포기해야 하는 키.
    Clear,
}

/// 핫키 시점의 변환 대상.
#[derive(Debug)]
pub enum Target {
    /// 아직 경계 없이 입력 중인 단어. bool = 직전에 핫키 변환으로 주입된
    /// 텍스트라서 화면 전체가 커밋 상태인지 여부.
    Current(String, bool),
    /// 경계 문자까지 확정된 직전 단어.
    Committed(String, char),
}

#[derive(Debug, Default)]
pub struct WordBuffer {
    current: String,
    last: Option<(String, char)>,
    /// 현재 단어가 핫키 변환으로 재주입된 상태인가. 이때 화면 텍스트는
    /// preedit 없이 전부 커밋돼 있고, 백스페이스 키 추적은 신뢰할 수 없다.
    converted: bool,
}

impl WordBuffer {
    pub const fn new() -> Self {
        Self {
            current: String::new(),
            last: None,
            converted: false,
        }
    }

    pub fn feed(&mut self, class: KeyClass) {
        match class {
            KeyClass::Letter(c) => {
                if self.converted {
                    // 주입된 텍스트 뒤에 이어 치기 시작 — 화면과 키 버퍼의
                    // 대응이 깨지므로 새 단어로 추적을 다시 시작한다.
                    self.clear();
                }
                self.current.push(c);
            }
            KeyClass::Boundary(b) => {
                if self.current.is_empty() {
                    // 경계 연타: 직전 단어와 경계 사이가 멀어져 추적 포기.
                    self.last = None;
                } else {
                    self.last = Some((std::mem::take(&mut self.current), b));
                }
                self.converted = false;
            }
            KeyClass::Backspace => {
                if self.converted {
                    self.clear();
                } else if self.current.pop().is_none() {
                    self.last = None;
                }
            }
            KeyClass::Clear => self.clear(),
        }
    }

    pub fn clear(&mut self) {
        self.current.clear();
        self.last = None;
        self.converted = false;
    }

    pub fn target(&self) -> Option<Target> {
        if !self.current.is_empty() {
            Some(Target::Current(self.current.clone(), self.converted))
        } else {
            self.last
                .clone()
                .map(|(word, boundary)| Target::Committed(word, boundary))
        }
    }

    /// 핫키 변환 직후 호출: 같은 키 시퀀스를 유지해 핫키 재입력 시
    /// 반대 방향으로 되돌릴 수 있게 한다.
    pub fn mark_converted(&mut self) {
        self.converted = true;
    }
}

#[cfg(test)]
mod tests {
    use super::KeyClass::*;
    use super::*;

    fn feed_str(buf: &mut WordBuffer, s: &str) {
        for c in s.chars() {
            buf.feed(Letter(c));
        }
    }

    #[test]
    fn tracks_current_then_committed() {
        let mut buf = WordBuffer::new();
        feed_str(&mut buf, "gksrmf");
        assert!(matches!(buf.target(), Some(Target::Current(w, false)) if w == "gksrmf"));
        buf.feed(Boundary(' '));
        assert!(matches!(buf.target(), Some(Target::Committed(w, ' ')) if w == "gksrmf"));
    }

    #[test]
    fn backspace_pops_then_falls_back_then_invalidates() {
        let mut buf = WordBuffer::new();
        feed_str(&mut buf, "ab");
        buf.feed(Boundary(' '));
        feed_str(&mut buf, "cd");
        buf.feed(Backspace);
        assert!(matches!(buf.target(), Some(Target::Current(w, _)) if w == "c"));
        // "cd"를 전부 지우면 화면은 "ab " → 직전 단어가 다시 대상이 된다.
        buf.feed(Backspace);
        assert!(matches!(buf.target(), Some(Target::Committed(w, ' ')) if w == "ab"));
        // 경계(공백)까지 지우면 더는 안전하게 추적할 수 없다.
        buf.feed(Backspace);
        assert!(buf.target().is_none());
    }

    #[test]
    fn converted_state_resets_on_typing() {
        let mut buf = WordBuffer::new();
        feed_str(&mut buf, "gks");
        buf.mark_converted();
        assert!(matches!(buf.target(), Some(Target::Current(_, true))));
        buf.feed(Letter('d'));
        assert!(matches!(buf.target(), Some(Target::Current(w, false)) if w == "d"));
    }

    #[test]
    fn clear_keys_drop_everything() {
        let mut buf = WordBuffer::new();
        feed_str(&mut buf, "abc");
        buf.feed(Boundary(' '));
        feed_str(&mut buf, "def");
        buf.feed(Clear);
        assert!(buf.target().is_none());
    }
}

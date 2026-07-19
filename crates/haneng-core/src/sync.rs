//! 주입 직렬화 — 변환(선택·타이핑 주입)은 한 번에 하나만.
//!
//! 핫키 연타 등으로 변환 스레드가 겹치면 주입 이벤트가 서로 뒤섞여
//! 텍스트가 깨지고 모드 추적이 어긋난다. 진행 중일 때 들어온 트리거는
//! 큐잉하지 않고 **버린다** — 완료된 누름만 한 번씩 토글되는 것이
//! 뒤늦게 몰아서 실행되는 것보다 안전하다.

use std::sync::atomic::{AtomicBool, Ordering};

pub struct InjectionLock(AtomicBool);

impl InjectionLock {
    pub const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    /// 잠금 시도. 이미 다른 변환이 진행 중이면 None (호출자는 그냥 반환).
    pub fn try_acquire(&'static self) -> Option<InjectionGuard> {
        if self.0.swap(true, Ordering::SeqCst) {
            None
        } else {
            Some(InjectionGuard(&self.0))
        }
    }
}

impl Default for InjectionLock {
    fn default() -> Self {
        Self::new()
    }
}

/// 드롭 시 잠금 해제 — 조기 return 경로에서도 반드시 풀린다.
pub struct InjectionGuard(&'static AtomicBool);

impl Drop for InjectionGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_fails_until_guard_drops() {
        static LOCK: InjectionLock = InjectionLock::new();
        let guard = LOCK.try_acquire().expect("first acquire");
        assert!(LOCK.try_acquire().is_none(), "재진입은 거부돼야 한다");
        drop(guard);
        assert!(
            LOCK.try_acquire().is_some(),
            "드롭 후에는 다시 잠글 수 있다"
        );
    }
}

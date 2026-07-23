//! 단일 인스턴스 보장 (Unix) — 잠금 파일 `flock`.
//!
//! 데몬을 두 번 실행하면 배지가 겹쳐 뜨거나 자원이 낭비된다. 설정
//! 디렉터리에 잠금 파일을 만들고 배타 잠금을 건다. 잠금은 프로세스가
//! 끝나면(크래시 포함) 커널이 자동 해제하므로 stale 파일 문제가 없다.
//! Windows는 네임드 뮤텍스를 쓰므로 이 모듈은 unix 전용이다.

use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::sync::OnceLock;

extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}
const LOCK_EX: i32 = 2;
const LOCK_NB: i32 = 4;

/// 프로세스 수명 동안 열린 채 유지되는 잠금 파일 핸들.
static GUARD: OnceLock<File> = OnceLock::new();

/// `name.lock`에 배타 잠금을 시도한다. 이미 다른 인스턴스가 쥐고 있으면
/// `false`(호출자는 즉시 종료). 잠금 자체가 불가능한 환경이면 과잉 차단을
/// 피하려 `true`로 통과시킨다.
pub fn acquire(name: &str) -> bool {
    let path = match crate::config::config_dir() {
        Some(dir) => {
            let _ = std::fs::create_dir_all(&dir);
            dir.join(format!("{name}.lock"))
        }
        None => std::env::temp_dir().join(format!("{name}.lock")),
    };
    let Ok(file) = File::create(&path) else {
        return true;
    };
    let locked = unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } == 0;
    if locked {
        // 핸들을 살려 둬야 잠금이 유지된다.
        let _ = GUARD.set(file);
    }
    locked
}

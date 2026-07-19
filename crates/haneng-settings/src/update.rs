//! 업데이트 확인·설치.
//!
//! 네트워크는 **사용자가 버튼을 눌렀을 때만** 사용한다 (프라이버시 원칙:
//! 데몬에는 네트워크 코드가 없고, 설정 앱도 자동으로 확인하지 않는다).
//!
//! - 확인: GitHub 릴리스 API에서 최신 태그를 읽어 현재 버전과 비교.
//! - 설치(Windows): 최신 MSI를 내려받아 데몬을 종료하고 msiexec 실행 —
//!   MSI가 업그레이드 후 데몬을 다시 띄운다. 설정 앱 자신도 교체 대상이라
//!   msiexec을 띄우고 즉시 종료한다.
//! - 설치(macOS/Linux): 릴리스 페이지를 브라우저로 연다.

pub const REPO: &str = "knoxxr/haneng";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    /// 새 버전 태그 (예: "v0.2.0").
    Available(String),
    Installing,
    Error(String),
}

/// 최신 릴리스 태그를 조회해 현재 버전과 비교한다 (블로킹 — 스레드에서 호출).
pub fn check() -> UpdateState {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let body = match ureq::get(&url)
        .header("User-Agent", "haneng-settings")
        .call()
    {
        Ok(mut res) => match res.body_mut().read_to_string() {
            Ok(body) => body,
            Err(e) => return UpdateState::Error(format!("응답 읽기 실패: {e}")),
        },
        Err(e) => return UpdateState::Error(format!("확인 실패: {e}")),
    };
    let Some(tag) = extract_tag_name(&body) else {
        return UpdateState::Error("릴리스 정보를 해석할 수 없음".into());
    };
    if is_newer(&tag, CURRENT_VERSION) {
        UpdateState::Available(tag)
    } else {
        UpdateState::UpToDate
    }
}

/// `"tag_name": "v0.1.2"` 형태에서 태그 추출 (의존성 없는 최소 파싱).
fn extract_tag_name(json: &str) -> Option<String> {
    let idx = json.find("\"tag_name\"")?;
    let rest = &json[idx + 10..];
    let start = rest.find('"')? + 1;
    let end = start + rest[start..].find('"')?;
    Some(rest[start..end].to_string())
}

/// SemVer 비교: tag("v0.2.0")가 current("0.1.2")보다 새 버전인가.
fn is_newer(tag: &str, current: &str) -> bool {
    fn triple(s: &str) -> Option<[u64; 3]> {
        let mut it = s.trim_start_matches('v').splitn(3, '.');
        Some([
            it.next()?.parse().ok()?,
            it.next()?.parse().ok()?,
            it.next()?.parse().ok()?,
        ])
    }
    match (triple(tag), triple(current)) {
        (Some(t), Some(c)) => t > c,
        _ => false,
    }
}

/// Windows: MSI를 내려받아 설치를 시작한다. 성공하면 프로세스를 종료하므로
/// 반환하지 않는다 (Err일 때만 돌아온다).
#[cfg(windows)]
pub fn install(tag: &str) -> Result<(), String> {
    use std::io::Read;
    let url = format!("https://github.com/{REPO}/releases/download/{tag}/haneng-windows.msi");
    let mut res = ureq::get(&url)
        .header("User-Agent", "haneng-settings")
        .call()
        .map_err(|e| format!("다운로드 실패: {e}"))?;
    let mut bytes = Vec::new();
    res.body_mut()
        .as_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("다운로드 실패: {e}"))?;
    let msi = std::env::temp_dir().join("haneng-update.msi");
    std::fs::write(&msi, &bytes).map_err(|e| format!("임시 파일 쓰기 실패: {e}"))?;

    // 실행 중인 데몬을 먼저 종료해야 MSI가 파일을 교체할 수 있다.
    let _ = std::process::Command::new("taskkill")
        .args(["/IM", "hanengw.exe", "/F"])
        .status();
    std::process::Command::new("msiexec")
        .args(["/i", &msi.to_string_lossy(), "/passive"])
        .spawn()
        .map_err(|e| format!("설치 실행 실패: {e}"))?;
    // 설정 앱 자신도 교체되므로 즉시 종료 — 설치가 끝나면 MSI가 데몬을 띄운다.
    std::process::exit(0);
}

/// macOS/Linux: 릴리스 페이지를 브라우저로 연다 (앱 교체 자동화는 추후).
#[cfg(not(windows))]
pub fn install(_tag: &str) -> Result<(), String> {
    let url = format!("https://github.com/{REPO}/releases/latest");
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(&url)
        .spawn()
        .map_err(|e| format!("브라우저 열기 실패: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tag_from_release_json() {
        let json = r#"{"url":"...","tag_name": "v0.1.2","name":"haneng v0.1.2"}"#;
        assert_eq!(extract_tag_name(json).as_deref(), Some("v0.1.2"));
        assert_eq!(extract_tag_name("{}"), None);
    }

    #[test]
    fn semver_comparison() {
        assert!(is_newer("v0.2.0", "0.1.2"));
        assert!(is_newer("v0.1.10", "0.1.2"));
        assert!(!is_newer("v0.1.2", "0.1.2"));
        assert!(!is_newer("v0.1.1", "0.1.2"));
        assert!(!is_newer("garbage", "0.1.2"));
    }
}

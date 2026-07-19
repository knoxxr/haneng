//! Text Input Source(HIToolbox) FFI — 현재 입력 소스가 한글인지 질의.
//!
//! 표시기는 IME를 바꾸지 않으므로 조회만 한다.

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use std::ffi::c_void;

type TISInputSourceRef = *mut c_void;

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    static kTISPropertyInputSourceID: CFStringRef;
    fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
    fn TISGetInputSourceProperty(source: TISInputSourceRef, key: CFStringRef) -> *const c_void;
}

fn is_korean_source_id(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    ["korean", "hangul", "gureum", "kime", "hangul"]
        .iter()
        .any(|k| id.contains(k))
}

/// 현재 키보드 입력 소스가 한글 계열인가. 확인 불가 시 None.
pub fn current_source_is_korean() -> Option<bool> {
    unsafe {
        let source = TISCopyCurrentKeyboardInputSource();
        if source.is_null() {
            return None;
        }
        let value = TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
        let id = if value.is_null() {
            None
        } else {
            Some(CFString::wrap_under_get_rule(value as CFStringRef).to_string())
        };
        CFRelease(source as CFTypeRef);
        id.map(|id| is_korean_source_id(&id))
    }
}

#[cfg(test)]
mod tests {
    use super::is_korean_source_id;

    #[test]
    fn korean_source_ids() {
        assert!(is_korean_source_id(
            "com.apple.inputmethod.Korean.2SetKorean"
        ));
        assert!(is_korean_source_id(
            "org.youknowone.inputmethod.Gureum.han2"
        ));
        assert!(!is_korean_source_id("com.apple.keylayout.ABC"));
        assert!(!is_korean_source_id("com.apple.keylayout.US"));
    }
}

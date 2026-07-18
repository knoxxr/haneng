//! Text Input Source(HIToolbox) FFI — 현재 입력 소스 확인·전환, 보안 입력 감지.

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use core_foundation_sys::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
use std::ffi::c_void;
use std::ptr;

type TISInputSourceRef = *mut c_void;

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    static kTISPropertyInputSourceID: CFStringRef;
    static kTISPropertyInputSourceCategory: CFStringRef;
    static kTISCategoryKeyboardInputSource: CFStringRef;
    fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
    fn TISGetInputSourceProperty(source: TISInputSourceRef, key: CFStringRef) -> *const c_void;
    fn TISCreateInputSourceList(properties: *const c_void, include_all: u8) -> CFArrayRef;
    fn TISSelectInputSource(source: TISInputSourceRef) -> i32;
    fn IsSecureEventInputEnabled() -> u8;
}

/// 비밀번호 필드 등 보안 입력이 활성화돼 있으면 모든 동작을 중단한다
/// (PLAN.md N3 — 절대 위반 불가).
pub fn secure_input_active() -> bool {
    unsafe { IsSecureEventInputEnabled() != 0 }
}

unsafe fn string_property(source: TISInputSourceRef, key: CFStringRef) -> Option<String> {
    let value = TISGetInputSourceProperty(source, key);
    if value.is_null() {
        None
    } else {
        Some(CFString::wrap_under_get_rule(value as CFStringRef).to_string())
    }
}

fn is_korean_source_id(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    ["korean", "hangul", "gureum", "kime"]
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
        let id = string_property(source, kTISPropertyInputSourceID);
        CFRelease(source as CFTypeRef);
        id.map(|id| is_korean_source_id(&id))
    }
}

/// 활성화된 키보드 입력 소스 중 한글(korean=true) 또는 라틴 자판을 찾아 선택한다.
pub fn select_input_source(korean: bool) -> bool {
    unsafe {
        let list = TISCreateInputSourceList(ptr::null(), 0);
        if list.is_null() {
            return false;
        }
        let keyboard_category =
            CFString::wrap_under_get_rule(kTISCategoryKeyboardInputSource).to_string();
        let mut selected = false;
        for i in 0..CFArrayGetCount(list) {
            let source = CFArrayGetValueAtIndex(list, i) as TISInputSourceRef;
            if source.is_null() {
                continue;
            }
            let category = string_property(source, kTISPropertyInputSourceCategory);
            if category.as_deref() != Some(keyboard_category.as_str()) {
                continue;
            }
            let Some(id) = string_property(source, kTISPropertyInputSourceID) else {
                continue;
            };
            let matches = if korean {
                is_korean_source_id(&id)
            } else {
                id.starts_with("com.apple.keylayout.")
            };
            if matches {
                selected = TISSelectInputSource(source) == 0;
                break;
            }
        }
        CFRelease(list as CFTypeRef);
        selected
    }
}

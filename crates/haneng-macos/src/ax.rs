//! 커서 아래 요소가 텍스트 입력인지 Accessibility API로 판별.
//!
//! Windows의 I-빔 커서 비교에 대응하는 macOS 기법: 시스템 전역 AX 요소
//! 트리에서 화면 좌표의 요소를 얻어 역할(role)이 텍스트류인지 본다.
//! **손쉬운 사용(Accessibility) 권한**이 필요하다 — 없으면 항상 false.

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::{CFString, CFStringRef};
use std::ffi::c_void;

type AXUIElementRef = *mut c_void;
type AXError = i32;
const AX_SUCCESS: AXError = 0;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyElementAtPosition(
        application: AXUIElementRef,
        x: f32,
        y: f32,
        element: *mut AXUIElementRef,
    ) -> AXError;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

/// 손쉬운 사용 권한 여부. `prompt=true`면 없을 때 시스템 설정 안내를 띄운다.
pub fn accessibility_trusted(prompt: bool) -> bool {
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let value = CFBoolean::from(prompt);
        let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as *const c_void)
    }
}

/// 텍스트 입력으로 볼 AX 역할.
fn is_text_role(role: &str) -> bool {
    matches!(
        role,
        "AXTextField" | "AXTextArea" | "AXComboBox" | "AXSearchField"
    )
}

/// 화면 좌표(top-left 원점) 아래 요소가 텍스트 입력인가.
pub fn text_input_at(x: f64, y: f64) -> bool {
    unsafe {
        let system = AXUIElementCreateSystemWide();
        if system.is_null() {
            return false;
        }
        let mut element: AXUIElementRef = std::ptr::null_mut();
        let err = AXUIElementCopyElementAtPosition(system, x as f32, y as f32, &mut element);
        CFRelease(system as CFTypeRef);
        if err != AX_SUCCESS || element.is_null() {
            return false;
        }
        let role_key = CFString::new("AXRole");
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err =
            AXUIElementCopyAttributeValue(element, role_key.as_concrete_TypeRef(), &mut value);
        CFRelease(element as CFTypeRef);
        if err != AX_SUCCESS || value.is_null() {
            return false;
        }
        let role = CFString::wrap_under_create_rule(value as CFStringRef).to_string();
        is_text_role(&role)
    }
}

#[cfg(test)]
mod tests {
    use super::is_text_role;

    #[test]
    fn text_roles() {
        assert!(is_text_role("AXTextField"));
        assert!(is_text_role("AXTextArea"));
        assert!(!is_text_role("AXButton"));
        assert!(!is_text_role("AXStaticText"));
    }
}

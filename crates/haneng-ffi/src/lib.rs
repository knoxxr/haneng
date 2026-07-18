//! haneng-core의 C ABI — Wayland 트랙(Fcitx5/IBus 애드온, C/C++)이 소비한다.
//!
//! 규약 (`include/haneng.h`와 동기 유지):
//! - 모든 문자열은 NUL 종단 UTF-8.
//! - 반환된 `char*`는 호출자가 `haneng_free()`로 해제한다.
//! - 잘못된 UTF-8/NULL 입력은 크래시 없이 NULL/0을 반환한다.

use haneng_core::{english_to_hangul, hangul_to_english, Detector, Sensitivity, Verdict};
use std::ffi::{c_char, c_int, CStr, CString};

/// Verdict 코드 (haneng.h의 HANENG_KEEP 등과 일치).
pub const HANENG_KEEP: c_int = 0;
pub const HANENG_TO_HANGUL: c_int = 1;
pub const HANENG_TO_ENGLISH: c_int = 2;

fn input_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

fn output_string(s: String) -> *mut c_char {
    // 내부 NUL이 있을 수 없는 변환 결과지만, 방어적으로 처리.
    CString::new(s).map_or(std::ptr::null_mut(), CString::into_raw)
}

/// 영문 모드로 잘못 친 문자열 → 한글. 해제: haneng_free().
/// # Safety
/// `text`는 NULL이거나 유효한 NUL 종단 문자열이어야 한다.
#[no_mangle]
pub unsafe extern "C" fn haneng_eng_to_han(text: *const c_char) -> *mut c_char {
    input_str(text).map_or(std::ptr::null_mut(), |s| {
        output_string(english_to_hangul(s))
    })
}

/// 한글 모드로 잘못 친 문자열 → 영문. 해제: haneng_free().
/// # Safety
/// `text`는 NULL이거나 유효한 NUL 종단 문자열이어야 한다.
#[no_mangle]
pub unsafe extern "C" fn haneng_han_to_eng(text: *const c_char) -> *mut c_char {
    input_str(text).map_or(std::ptr::null_mut(), |s| {
        output_string(hangul_to_english(s))
    })
}

/// # Safety
/// `s`는 NULL이거나 이 라이브러리가 반환한 포인터여야 하며, 두 번 해제하면 안 된다.
#[no_mangle]
pub unsafe extern "C" fn haneng_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// 감지기 생성. sensitivity: 0=Conservative, 1=Balanced, 2=Aggressive.
#[no_mangle]
pub extern "C" fn haneng_detector_new(sensitivity: c_int) -> *mut Detector {
    let sensitivity = match sensitivity {
        0 => Sensitivity::Conservative,
        2 => Sensitivity::Aggressive,
        _ => Sensitivity::Balanced,
    };
    Box::into_raw(Box::new(Detector::new(sensitivity)))
}

/// # Safety
/// `detector`는 NULL이거나 haneng_detector_new()가 반환한 포인터여야 하며, 두 번 해제하면 안 된다.
#[no_mangle]
pub unsafe extern "C" fn haneng_detector_free(detector: *mut Detector) {
    if !detector.is_null() {
        drop(unsafe { Box::from_raw(detector) });
    }
}

/// 단어 판정. 반환: HANENG_KEEP / HANENG_TO_HANGUL / HANENG_TO_ENGLISH.
/// 변환이 필요하면 *converted에 결과를 담는다 (haneng_free()로 해제).
/// # Safety
/// `detector`는 NULL이거나 유효한 감지기, `word`는 NULL이거나 유효한 NUL 종단
/// 문자열, `converted`는 NULL이거나 쓰기 가능한 포인터 슬롯이어야 한다.
#[no_mangle]
pub unsafe extern "C" fn haneng_detector_analyze(
    detector: *const Detector,
    word: *const c_char,
    converted: *mut *mut c_char,
) -> c_int {
    if !converted.is_null() {
        unsafe { *converted = std::ptr::null_mut() };
    }
    let (Some(detector), Some(word)) = (unsafe { detector.as_ref() }, input_str(word)) else {
        return HANENG_KEEP;
    };
    let (code, text) = match detector.analyze(word) {
        Verdict::Keep => return HANENG_KEEP,
        Verdict::ToHangul(t) => (HANENG_TO_HANGUL, t),
        Verdict::ToEnglish(t) => (HANENG_TO_ENGLISH, t),
    };
    if !converted.is_null() {
        unsafe { *converted = output_string(text) };
    }
    code
}

/// 되돌리기 학습: 이 단어를 다시는 자동 변환하지 않는다.
/// # Safety
/// `detector`는 NULL이거나 유효한 감지기(동시 접근 없음), `word`는 NULL이거나
/// 유효한 NUL 종단 문자열이어야 한다.
#[no_mangle]
pub unsafe extern "C" fn haneng_detector_record_undo(detector: *mut Detector, word: *const c_char) {
    if let (Some(detector), Some(word)) = (unsafe { detector.as_mut() }, input_str(word)) {
        detector.record_undo(word);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    unsafe fn take(ptr: *mut c_char) -> String {
        assert!(!ptr.is_null());
        let s = CStr::from_ptr(ptr).to_str().unwrap().to_string();
        haneng_free(ptr);
        s
    }

    #[test]
    fn conversion_roundtrip_over_ffi() {
        unsafe {
            assert_eq!(take(haneng_eng_to_han(c("gksrmf").as_ptr())), "한글");
            assert_eq!(take(haneng_han_to_eng(c("ㅗ디ㅣㅐ").as_ptr())), "hello");
        }
    }

    #[test]
    fn detector_over_ffi_with_undo_learning() {
        unsafe {
            let d = haneng_detector_new(1);
            let mut out: *mut c_char = std::ptr::null_mut();

            assert_eq!(
                haneng_detector_analyze(d, c("gksrmf").as_ptr(), &mut out),
                HANENG_TO_HANGUL
            );
            assert_eq!(take(out), "한글");

            assert_eq!(
                haneng_detector_analyze(d, c("hello").as_ptr(), std::ptr::null_mut()),
                HANENG_KEEP
            );

            haneng_detector_record_undo(d, c("gksrmf").as_ptr());
            let mut out2: *mut c_char = std::ptr::null_mut();
            assert_eq!(
                haneng_detector_analyze(d, c("gksrmf").as_ptr(), &mut out2),
                HANENG_KEEP
            );
            assert!(out2.is_null());

            haneng_detector_free(d);
        }
    }

    #[test]
    fn null_and_invalid_inputs_are_safe() {
        unsafe {
            assert!(haneng_eng_to_han(std::ptr::null()).is_null());
            assert_eq!(
                haneng_detector_analyze(std::ptr::null(), c("x").as_ptr(), std::ptr::null_mut()),
                HANENG_KEEP
            );
            haneng_free(std::ptr::null_mut());
        }
    }
}

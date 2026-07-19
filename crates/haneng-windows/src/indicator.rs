//! 한/영 상태 배지 — 마우스가 텍스트 입력(I-빔 커서) 위에 있을 때
//! 커서 옆에 현재 IME 모드("한"/"A")를 표시한다.
//!
//! - 입력 영역 판별: 시스템 커서가 I-빔인지 비교 (앱 무관 표준 기법.
//!   커스텀 커서 테마에서는 감지되지 않을 수 있다 — 알려진 한계).
//! - 배지 창: 최상위·비활성·클릭 통과(layered) 팝업. 메시지 루프 스레드
//!   에서 만들어지고 같은 스레드의 LL 마우스 훅이 위치를 갱신한다.
//! - LL 마우스 훅은 모든 마우스 이동마다 불리므로 갱신을 50ms로 스로틀.
//! - 모드 출처: 데몬이 추적하는 한/영 상태 (`set_mode`로 전달받는다).
//! - 끄기: config.txt에 `hover_indicator = off`.

use std::mem::{size_of, zeroed};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, InvalidateRect,
    SetBkMode, SetTextColor, DT_CENTER, DT_SINGLELINE, DT_VCENTER, PAINTSTRUCT, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, GetCursorInfo, LoadCursorW, RegisterClassW,
    SetLayeredWindowAttributes, SetWindowPos, ShowWindow, CURSORINFO, CURSOR_SHOWING, HWND_TOPMOST,
    IDC_IBEAM, LWA_ALPHA, SWP_NOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, WM_PAINT, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

static INDICATOR_HWND: AtomicUsize = AtomicUsize::new(0);
static IBEAM_CURSOR: AtomicUsize = AtomicUsize::new(0);
static VISIBLE: AtomicBool = AtomicBool::new(false);
static KOREAN: AtomicBool = AtomicBool::new(false);
static LAST_UPDATE_MS: AtomicU64 = AtomicU64::new(0);

/// 배지 한 변 크기(px)와 커서 기준 오프셋.
const BADGE_SIZE: i32 = 22;
const OFFSET: i32 = 18;
const THROTTLE_MS: u64 = 50;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// 배지 창 생성 (메시지 루프 스레드에서 한 번 호출).
pub fn init() {
    unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        let class_name = wide("haneng-indicator");
        let mut wc: WNDCLASSW = zeroed();
        wc.lpfnWndProc = Some(wnd_proc);
        wc.hInstance = hinstance;
        wc.lpszClassName = class_name.as_ptr();
        if RegisterClassW(&wc) == 0 {
            return; // 배지는 부가 기능 — 실패해도 데몬은 계속 동작.
        }
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_LAYERED,
            class_name.as_ptr(),
            std::ptr::null(),
            WS_POPUP,
            0,
            0,
            BADGE_SIZE,
            BADGE_SIZE,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return;
        }
        SetLayeredWindowAttributes(hwnd, 0, 235, LWA_ALPHA);
        IBEAM_CURSOR.store(
            LoadCursorW(std::ptr::null_mut(), IDC_IBEAM) as usize,
            Ordering::Relaxed,
        );
        INDICATOR_HWND.store(hwnd as usize, Ordering::Release);
    }
}

/// 추적 중인 한/영 모드 갱신 — 배지가 보이는 중이면 즉시 다시 그린다.
pub fn set_mode(korean: bool) {
    if KOREAN.swap(korean, Ordering::Relaxed) == korean {
        return;
    }
    let hwnd = INDICATOR_HWND.load(Ordering::Acquire) as HWND;
    if !hwnd.is_null() && VISIBLE.load(Ordering::Relaxed) {
        unsafe { InvalidateRect(hwnd, std::ptr::null(), 1) };
    }
}

/// LL 마우스 훅의 이동 이벤트에서 호출 — 반드시 가볍게.
pub fn on_mouse_move() {
    let hwnd = INDICATOR_HWND.load(Ordering::Acquire) as HWND;
    if hwnd.is_null() {
        return;
    }
    // 스로틀: 마우스 이동은 초당 수백 회 — 50ms에 한 번만 처리.
    let now = unsafe { GetTickCount64() };
    if now.saturating_sub(LAST_UPDATE_MS.load(Ordering::Relaxed)) < THROTTLE_MS {
        return;
    }
    LAST_UPDATE_MS.store(now, Ordering::Relaxed);

    unsafe {
        let mut info: CURSORINFO = zeroed();
        info.cbSize = size_of::<CURSORINFO>() as u32;
        if GetCursorInfo(&mut info) == 0 {
            return;
        }
        let over_text = info.flags == CURSOR_SHOWING
            && info.hCursor as usize == IBEAM_CURSOR.load(Ordering::Relaxed);
        if over_text {
            let first_show = !VISIBLE.swap(true, Ordering::Relaxed);
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                info.ptScreenPos.x + OFFSET,
                info.ptScreenPos.y + OFFSET,
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            if first_show {
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
        } else if VISIBLE.swap(false, Ordering::Relaxed) {
            ShowWindow(hwnd, SW_HIDE);
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_PAINT {
        let korean = KOREAN.load(Ordering::Relaxed);
        let mut ps: PAINTSTRUCT = zeroed();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut rect: RECT = zeroed();
        GetClientRect(hwnd, &mut rect);
        // 한 = 파랑(#2B6CB0), 영 = 회색(#4A5568) 배경에 흰 글자 (COLORREF는 BGR).
        let bg = CreateSolidBrush(if korean { 0x00B06C2B } else { 0x0068554A });
        FillRect(hdc, &rect, bg);
        DeleteObject(bg as _);
        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, 0x00FFFFFF);
        let label = wide(if korean { "한" } else { "A" });
        DrawTextW(
            hdc,
            label.as_ptr(),
            -1,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        EndPaint(hwnd, &ps);
        return 0;
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

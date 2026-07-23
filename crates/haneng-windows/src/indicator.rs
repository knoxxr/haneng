//! 한/영 상태 배지 — **포커스된 입력 카렛 옆**에 현재 입력 상태를 표시한다
//! (마우스 위치와 무관):
//! 파랑 "한"(한글) / 회색 "a"(영문 소문자) / 주황 "A"(영문 + Caps Lock).
//!
//! - 표시 조건: 포커스된 텍스트 입력의 카렛을 읽을 수 있으면 표시. 표준
//!   Win32 카렛을 먼저 쓰고, 없으면(크롬·Electron 등) UI Automation으로
//!   폴백한다(`ime::caret_screen_rect`). 카렛을 못 읽으면 숨긴다.
//! - 표시 위치: 카렛 위쪽(`ime::caret_screen_rect`)에 두어 입력 글자를
//!   가리지 않는다. 마우스와 무관하게 카렛을 따라간다.
//! - 구동: 배지 창에 건 타이머(WM_TIMER)가 주기적으로 카렛을 확인해
//!   위치·모드·표시 여부를 갱신한다.
//! - 배지 창: 최상위·비활성·클릭 통과(layered) 팝업.
//! - 모드 출처: 데몬이 추적하는 한/영 상태 (`init`으로 전달받는다).
//! - 끄기: 트레이 토글(ENABLED) 또는 config.txt `hover_indicator = off`.

use std::mem::{size_of, zeroed};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, GetMonitorInfoW,
    InvalidateRect, MonitorFromPoint, SetBkMode, SetTextColor, DT_CENTER, DT_SINGLELINE,
    DT_VCENTER, MONITORINFO, MONITOR_DEFAULTTONEAREST, PAINTSTRUCT, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, RegisterClassW, SetLayeredWindowAttributes,
    SetTimer, SetWindowPos, ShowWindow, HWND_TOPMOST, LWA_ALPHA, SWP_NOACTIVATE, SWP_NOSIZE,
    SWP_SHOWWINDOW, SW_HIDE, WM_PAINT, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

/// 배지에 표시할 입력 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Mode {
    EnglishLower = 0,
    EnglishUpper = 1,
    Korean = 2,
}

impl Mode {
    fn from_u8(v: u8) -> Self {
        match v {
            2 => Mode::Korean,
            1 => Mode::EnglishUpper,
            _ => Mode::EnglishLower,
        }
    }
}

static INDICATOR_HWND: AtomicUsize = AtomicUsize::new(0);
static VISIBLE: AtomicBool = AtomicBool::new(false);
static MODE: AtomicU8 = AtomicU8::new(0);
/// 현재 입력 상태를 알려주는 콜백 (init에서 설정).
static MODE_SOURCE: OnceLock<fn() -> Mode> = OnceLock::new();
/// 배지 표시 활성화 여부 (트레이 토글 — init에서 설정).
static ENABLED_SRC: OnceLock<&'static AtomicBool> = OnceLock::new();

/// 배지 한 변 크기(px)와 카렛과의 간격.
const BADGE_SIZE: i32 = 22;
/// 카렛과 배지 사이 여백 — 배지는 카렛 위쪽에 놓아 입력 글자를 가리지 않는다.
const GAP: i32 = 2;
/// 카렛 위치·모드·표시 여부 확인 주기.
const REFRESH_TIMER_ID: usize = 1;
const REFRESH_MS: u32 = 150;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// 배지 창 생성 + 갱신 타이머 시작 (메시지 루프 스레드에서 한 번 호출).
/// `alpha`는 창 불투명도 (0=완전 투명, 255=불투명).
pub fn init(mode_source: fn() -> Mode, enabled: &'static AtomicBool, alpha: u8) {
    let _ = MODE_SOURCE.set(mode_source);
    let _ = ENABLED_SRC.set(enabled);
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
        SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA);
        INDICATOR_HWND.store(hwnd as usize, Ordering::Release);
        // 마우스와 무관하게 카렛을 따라가도록 상시 타이머를 건다.
        SetTimer(hwnd, REFRESH_TIMER_ID, REFRESH_MS, None);
    }
}

/// 상태 소스를 다시 읽어 바뀌었으면 다시 그린다.
fn refresh_mode(hwnd: HWND) -> Mode {
    let mode = MODE_SOURCE.get().map(|f| f()).unwrap_or(Mode::EnglishLower);
    if MODE.swap(mode as u8, Ordering::Relaxed) != mode as u8 && !hwnd.is_null() {
        unsafe { InvalidateRect(hwnd, std::ptr::null(), 1) };
    }
    mode
}

/// 화면 좌표 `(x, y)`가 속한 모니터의 작업 영역 `(left, top, right, bottom)`.
/// 어떤 모니터에도 속하지 않으면 가장 가까운 모니터를 쓴다.
///
/// 멀티 모니터에서 프라이머리 왼쪽·위쪽 모니터는 좌표가 음수다. 배지를
/// 0으로 자르면 그 카렛을 따라가지 못하고 프라이머리 가장자리에 붙어버리므로,
/// 카렛이 있는 모니터 경계를 기준으로 삼는다.
unsafe fn monitor_bounds(x: i32, y: i32) -> (i32, i32, i32, i32) {
    let mon = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
    let mut mi: MONITORINFO = zeroed();
    mi.cbSize = size_of::<MONITORINFO>() as u32;
    if !mon.is_null() && GetMonitorInfoW(mon, &mut mi) != 0 {
        let r = mi.rcWork;
        (r.left, r.top, r.right, r.bottom)
    } else {
        // 폴백: 가상 화면 전체 (사실상 무제한).
        (i32::MIN, i32::MIN, i32::MAX, i32::MAX)
    }
}

/// 배지 창을 **카렛 바로 위**(넘치면 아래)로 옮긴다. 카렛을 못 읽으면
/// `false`(호출자가 숨긴다) — 마우스 위치에는 표시하지 않는다.
unsafe fn place_badge(hwnd: HWND) -> bool {
    let Some((left, top, bottom)) = crate::ime::caret_screen_rect() else {
        return false;
    };
    // 카렛이 있는 모니터 경계 안에서만 위치를 잡는다 (멀티 모니터: 음수
    // 좌표의 보조 모니터에서도 카렛을 따라가도록 0이 아니라 모니터로 클램프).
    let (mon_left, mon_top, mon_right, mon_bottom) = monitor_bounds(left, top);
    let above = top - BADGE_SIZE - GAP;
    let y = if above >= mon_top {
        above
    } else {
        bottom + GAP
    };
    let x = left.clamp(mon_left, (mon_right - BADGE_SIZE).max(mon_left));
    let y = y.clamp(mon_top, (mon_bottom - BADGE_SIZE).max(mon_top));
    SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        x,
        y,
        0,
        0,
        SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
    true
}

/// 배지를 숨긴다. (타이머는 상시 유지 — 끄지 않는다.)
unsafe fn hide(hwnd: HWND) {
    if VISIBLE.swap(false, Ordering::Relaxed) {
        ShowWindow(hwnd, SW_HIDE);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_TIMER && wparam == REFRESH_TIMER_ID {
        // 마우스와 무관하게: 활성화돼 있고 포커스 카렛을 읽을 수 있으면
        // 그 카렛 위에 표시, 아니면 숨긴다.
        let enabled = ENABLED_SRC
            .get()
            .map(|e| e.load(Ordering::Relaxed))
            .unwrap_or(true);
        if enabled {
            refresh_mode(hwnd);
            if place_badge(hwnd) {
                if !VISIBLE.swap(true, Ordering::Relaxed) {
                    InvalidateRect(hwnd, std::ptr::null(), 1);
                }
            } else {
                hide(hwnd);
            }
        } else {
            hide(hwnd);
        }
        return 0;
    }
    if msg == WM_PAINT {
        let mode = Mode::from_u8(MODE.load(Ordering::Relaxed));
        let mut ps: PAINTSTRUCT = zeroed();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut rect: RECT = zeroed();
        GetClientRect(hwnd, &mut rect);
        // COLORREF는 BGR: 한=파랑(#2B6CB0), a=회색(#4A5568),
        // A=주황(#DD6B20 — Caps Lock 켜짐 경고).
        let (color, label) = match mode {
            Mode::Korean => (0x00B06C2Bu32, "한"),
            Mode::EnglishUpper => (0x00206BDD, "A"),
            Mode::EnglishLower => (0x0068554A, "a"),
        };
        let bg = CreateSolidBrush(color);
        FillRect(hdc, &rect, bg);
        DeleteObject(bg as _);
        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, 0x00FFFFFF);
        let label = wide(label);
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

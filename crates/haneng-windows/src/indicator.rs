//! 한/영 상태 배지 — 마우스가 텍스트 입력(I-빔 커서) 위에 있을 때
//! **입력 카렛(입력 커서) 바로 위**에 현재 입력 상태를 표시한다:
//! 파랑 "한"(한글) / 회색 "a"(영문 소문자) / 주황 "A"(영문 + Caps Lock).
//!
//! - 표시 조건(트리거): 시스템 커서가 I-빔인지 비교 (앱 무관 표준 기법.
//!   커스텀 커서 테마에서는 감지되지 않을 수 있다 — 알려진 한계).
//! - 표시 위치: 마우스가 아니라 포커스 창의 **카렛 위치**
//!   (`ime::caret_screen_rect`). 입력 글자를 가리지 않게 카렛 위쪽에 둔다.
//!   카렛을 못 읽는 앱(크롬·Electron 등)에서는 배지를 숨긴다.
//! - 배지 창: 최상위·비활성·클릭 통과(layered) 팝업. 메시지 루프 스레드
//!   에서 만들어지고 같은 스레드의 LL 마우스 훅이 위치를 갱신한다.
//! - LL 마우스 훅은 모든 마우스 이동마다 불리므로 갱신을 50ms로 스로틀.
//! - 모드 출처: 데몬이 추적하는 한/영 상태 (`init`으로 전달받는다).
//! - 끄기: config.txt에 `hover_indicator = off`.

use std::mem::{size_of, zeroed};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, InvalidateRect,
    SetBkMode, SetTextColor, DT_CENTER, DT_SINGLELINE, DT_VCENTER, PAINTSTRUCT, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, GetCursorInfo, KillTimer, LoadCursorW,
    RegisterClassW, SetLayeredWindowAttributes, SetTimer, SetWindowPos, ShowWindow, CURSORINFO,
    CURSOR_SHOWING, HWND_TOPMOST, IDC_IBEAM, LWA_ALPHA, SWP_NOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW,
    SW_HIDE, WM_PAINT, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
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
static IBEAM_CURSOR: AtomicUsize = AtomicUsize::new(0);
static VISIBLE: AtomicBool = AtomicBool::new(false);
static MODE: AtomicU8 = AtomicU8::new(0);
static LAST_UPDATE_MS: AtomicU64 = AtomicU64::new(0);
/// 현재 입력 상태를 알려주는 콜백 (init에서 설정).
static MODE_SOURCE: OnceLock<fn() -> Mode> = OnceLock::new();

/// 배지 한 변 크기(px)와 카렛과의 간격.
const BADGE_SIZE: i32 = 22;
/// 카렛과 배지 사이 여백 — 배지는 카렛 위쪽에 놓아 입력 글자를 가리지 않는다.
const GAP: i32 = 2;
const THROTTLE_MS: u64 = 50;
/// 배지가 떠 있는 동안 상태 재확인 주기 — 마우스가 멈춰 있어도
/// 한/영·Caps Lock 변화가 반영되게 한다.
const REFRESH_TIMER_ID: usize = 1;
const REFRESH_MS: u32 = 300;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// 배지 창 생성 (메시지 루프 스레드에서 한 번 호출).
/// `alpha`는 창 불투명도 (0=완전 투명, 255=불투명).
pub fn init(mode_source: fn() -> Mode, alpha: u8) {
    let _ = MODE_SOURCE.set(mode_source);
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
        IBEAM_CURSOR.store(
            LoadCursorW(std::ptr::null_mut(), IDC_IBEAM) as usize,
            Ordering::Relaxed,
        );
        INDICATOR_HWND.store(hwnd as usize, Ordering::Release);
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

/// 카렛 위치에 배지 창을 옮긴다. 카렛을 못 읽으면 `false` (호출자가 숨긴다).
/// 카렛 바로 위에 두되 화면 위로 넘치면 아래쪽으로 뒤집는다.
unsafe fn place_at_caret(hwnd: HWND) -> bool {
    let Some((left, top, bottom)) = crate::ime::caret_screen_rect() else {
        return false;
    };
    let x = left.max(0);
    let above = top - BADGE_SIZE - GAP;
    let y = if above >= 0 { above } else { bottom + GAP };
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

/// 배지를 숨기고 재확인 타이머를 멈춘다.
unsafe fn hide(hwnd: HWND) {
    if VISIBLE.swap(false, Ordering::Relaxed) {
        KillTimer(hwnd, REFRESH_TIMER_ID);
        ShowWindow(hwnd, SW_HIDE);
    }
}

/// LL 마우스 훅의 이동 이벤트에서 호출 — 반드시 가볍게.
/// 상태 소스·카렛 질의는 배지를 실제로 표시/갱신할 때만 호출된다 (I-빔 위 +
/// 스로틀 통과 시) — IME 실시간 질의처럼 다소 무거운 소스여도 된다.
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
        // I-빔 위 + 카렛을 읽을 수 있을 때만 표시. 둘 중 하나라도 아니면 숨긴다.
        if over_text {
            refresh_mode(hwnd);
            if place_at_caret(hwnd) {
                let first_show = !VISIBLE.swap(true, Ordering::Relaxed);
                if first_show {
                    InvalidateRect(hwnd, std::ptr::null(), 1);
                    SetTimer(hwnd, REFRESH_TIMER_ID, REFRESH_MS, None);
                }
            } else {
                hide(hwnd);
            }
        } else {
            hide(hwnd);
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_TIMER && wparam == REFRESH_TIMER_ID {
        if VISIBLE.load(Ordering::Relaxed) {
            refresh_mode(hwnd);
            // 입력 중 마우스가 멈춰 있어도 카렛을 따라 위치를 갱신하고,
            // 카렛이 사라졌으면(포커스 상실 등) 숨긴다.
            if !place_at_caret(hwnd) {
                hide(hwnd);
            }
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

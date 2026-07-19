//! 한/영 상태 배지 오버레이 (AppKit NSWindow via objc2).
//!
//! 테두리 없는·비활성·클릭 통과·최상위 창에 색상 + 라벨을 그린다.
//! Windows의 layered 배지 창에 대응한다. AppKit이 한글을 직접 렌더링하므로
//! 별도 폰트 래스터라이저가 필요 없다.

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSFont, NSTextAlignment, NSTextField, NSWindow,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::Mode;

const SIZE: f64 = 24.0;
/// 커서 기준 오프셋 (오른쪽 아래).
const OFFSET: f64 = 18.0;

pub struct Badge {
    mtm: MainThreadMarker,
    window: Retained<NSWindow>,
    label: Retained<NSTextField>,
    visible: bool,
    mode: Option<Mode>,
}

impl Badge {
    pub fn new(mtm: MainThreadMarker) -> Self {
        unsafe {
            let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(SIZE, SIZE));
            let window = NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            );
            window.setOpaque(false);
            window.setHasShadow(false);
            window.setIgnoresMouseEvents(true);
            // 상태 항목보다 위, 모든 스페이스·전체화면 위에 뜨되 활성화하지 않음.
            window.setLevel(objc2_app_kit::NSStatusWindowLevel);
            window.setCollectionBehavior(
                NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::Stationary
                    | NSWindowCollectionBehavior::IgnoresCycle,
            );
            window.setBackgroundColor(Some(&NSColor::clearColor()));

            let label = NSTextField::initWithFrame(NSTextField::alloc(mtm), rect);
            label.setBezeled(false);
            label.setEditable(false);
            label.setSelectable(false);
            label.setDrawsBackground(true);
            label.setAlignment(NSTextAlignment::Center);
            label.setFont(Some(&NSFont::boldSystemFontOfSize(15.0)));
            label.setTextColor(Some(&NSColor::whiteColor()));
            // 세로 중앙 정렬을 위해 살짝 내려 그린다 (라벨은 상단 기준).
            label.setFrame(NSRect::new(
                NSPoint::new(0.0, -3.0),
                NSSize::new(SIZE, SIZE),
            ));
            if let Some(view) = window.contentView() {
                view.setWantsLayer(true);
                view.addSubview(&label);
            }

            Self {
                mtm,
                window,
                label,
                visible: false,
                mode: None,
            }
        }
    }

    fn apply_mode(&mut self, mode: Mode) {
        if self.mode == Some(mode) {
            return;
        }
        self.mode = Some(mode);
        let (r, g, b, text) = match mode {
            Mode::Korean => (0.169, 0.424, 0.690, "한"),
            Mode::EnglishUpper => (0.867, 0.420, 0.125, "A"), // 주황 — Caps Lock 경고
            Mode::EnglishLower => (0.290, 0.333, 0.408, "a"),
        };
        let color = NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0);
        self.label.setBackgroundColor(Some(&color));
        self.label.setStringValue(&NSString::from_str(text));
    }

    /// 커서 위치(top-left 원점)에 모드를 갱신해 표시한다.
    pub fn show_at(&mut self, screen_x: f64, screen_y_top: f64, mode: Mode) {
        self.apply_mode(mode);
        // AppKit은 bottom-left 원점 — 메인 화면 높이로 뒤집는다.
        let screen_h = self
            .window
            .screen()
            .or_else(|| objc2_app_kit::NSScreen::mainScreen(self.mtm))
            .map(|s| s.frame().size.height)
            .unwrap_or(0.0);
        let origin = NSPoint::new(screen_x + OFFSET, screen_h - screen_y_top - OFFSET - SIZE);
        self.window.setFrameOrigin(origin);
        if !self.visible {
            self.window.orderFront(None);
            self.visible = true;
        }
    }

    pub fn hide(&mut self) {
        if self.visible {
            self.window.orderOut(None);
            self.visible = false;
        }
    }
}

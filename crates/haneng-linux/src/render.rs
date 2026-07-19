//! 배지 이미지 래스터라이즈 (fontdue) — 상태별 BGRX 픽셀 버퍼.
//!
//! X11 24비트 창에 `put_image`로 통째로 올린다 (알파 없음 → 불투명 사각형
//! 배지). macOS는 AppKit가 직접 그리므로 이 모듈은 Linux 전용이다.

use crate::Mode;

pub const SIZE: usize = 24;

fn load_font() -> Option<fontdue::Font> {
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/truetype/nanum/NanumGothicBold.ttf",
        "/usr/share/fonts/truetype/nanum/NanumGothic.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", // 최후: 한글 없음
    ];
    let bytes = CANDIDATES.iter().find_map(|p| std::fs::read(p).ok())?;
    fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
}

/// 상태별 (배경 BGR) + 글자.
fn style(mode: Mode) -> ((u8, u8, u8), char) {
    match mode {
        // BGR 순서.
        Mode::Korean => ((0xB0, 0x6C, 0x2B), '한'),
        Mode::EnglishUpper => ((0x20, 0x6B, 0xDD), 'A'), // 주황 경고
        Mode::EnglishLower => ((0x68, 0x55, 0x4A), 'a'),
    }
}

/// NxN BGRX 버퍼를 그린다 (배경색 채우고 흰 글자를 중앙에 합성).
pub fn render(mode: Mode, font: Option<&fontdue::Font>) -> Vec<u8> {
    let ((bb, bg, br), ch) = style(mode);
    let mut buf = vec![0u8; SIZE * SIZE * 4];
    for px in buf.chunks_exact_mut(4) {
        px[0] = bb;
        px[1] = bg;
        px[2] = br;
        px[3] = 0xFF;
    }
    let Some(font) = font else {
        return buf;
    };
    let px_size = SIZE as f32 * 0.7;
    let (metrics, bitmap) = font.rasterize(ch, px_size);
    if metrics.width == 0 || metrics.height == 0 {
        return buf;
    }
    let ox = (SIZE as i32 - metrics.width as i32) / 2;
    let oy = (SIZE as i32 - metrics.height as i32) / 2;
    for gy in 0..metrics.height {
        for gx in 0..metrics.width {
            let cov = bitmap[gy * metrics.width + gx];
            if cov == 0 {
                continue;
            }
            let x = ox + gx as i32;
            let y = oy + gy as i32;
            if x < 0 || y < 0 || x >= SIZE as i32 || y >= SIZE as i32 {
                continue;
            }
            let idx = (y as usize * SIZE + x as usize) * 4;
            let a = cov as u16;
            // 흰 글자를 배경 위에 알파 합성.
            for (c, base) in [(0, bb), (1, bg), (2, br)] {
                let fg = 0xFFu16;
                buf[idx + c] = ((fg * a + base as u16 * (255 - a)) / 255) as u8;
            }
        }
    }
    buf
}

pub fn load_font_cached() -> Option<fontdue::Font> {
    load_font()
}

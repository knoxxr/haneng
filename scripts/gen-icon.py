#!/usr/bin/env python3
"""haneng 아이콘 생성 — 배지 모티프 (파란 "한" / 회색 "A" 세로 분할).

산출물 (커밋 대상, assets/):
- haneng.ico     : 16~256px 멀티사이즈 (exe 아이콘, MSI ARPPRODUCTICON)
- tray-32.rgba   : 32×32 원시 RGBA (트레이 아이콘, include_bytes 용)
- icon-64.rgba   : 64×64 원시 RGBA (설정 창 아이콘)
- icon-256.png   : 미리보기/README 용

실행 (Pillow 필요):
  python3 -m venv /tmp/iconenv && /tmp/iconenv/bin/pip install Pillow
  /tmp/iconenv/bin/python scripts/gen-icon.py
"""

from PIL import Image, ImageChops, ImageDraw, ImageFont

BLUE = (0x2B, 0x6C, 0xB0, 255)
GRAY = (0x4A, 0x55, 0x68, 255)
WHITE = (255, 255, 255, 255)
FONTS = [
    "/System/Library/Fonts/Supplemental/AppleGothic.ttf",  # macOS
    "C:/Windows/Fonts/malgunbd.ttf",  # Windows
    "/usr/share/fonts/truetype/nanum/NanumGothicBold.ttf",  # Linux
]


def font_at(size: int) -> ImageFont.FreeTypeFont:
    for path in FONTS:
        try:
            return ImageFont.truetype(path, size)
        except OSError:
            continue
    raise SystemExit("한글 폰트를 찾지 못함 — FONTS 경로를 수정하세요")


def draw_master(size: int) -> Image.Image:
    # 라운드 사각형 마스크 안에서 왼쪽(파랑)/오른쪽(회색)을 수직선으로 분할.
    radius = size // 5
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [0, 0, size - 1, size - 1], radius=radius, fill=255
    )
    right_half = Image.new("L", (size, size), 0)
    ImageDraw.Draw(right_half).rectangle([size // 2, 0, size - 1, size - 1], fill=255)

    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    img.paste(Image.new("RGBA", (size, size), BLUE), (0, 0), mask)
    img.paste(
        Image.new("RGBA", (size, size), GRAY),
        (0, 0),
        ImageChops.multiply(mask, right_half),
    )
    d = ImageDraw.Draw(img)

    font = font_at(int(size * 0.52))

    def center_text(text: str, cx: int) -> None:
        left, top, right, bottom = d.textbbox((0, 0), text, font=font)
        w, h = right - left, bottom - top
        d.text((cx - w / 2 - left, size / 2 - h / 2 - top), text, font=font, fill=WHITE)

    center_text("한", size // 4)
    center_text("A", size * 3 // 4)
    return img


def main() -> None:
    master = draw_master(256)
    master.save("assets/icon-256.png")
    master.save(
        "assets/haneng.ico",
        sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    for size, name in [(32, "assets/tray-32.rgba"), (64, "assets/icon-64.rgba")]:
        img = master.resize((size, size), Image.LANCZOS)
        with open(name, "wb") as f:
            f.write(img.tobytes("raw", "RGBA"))
    print("생성 완료: assets/haneng.ico, tray-32.rgba, icon-64.rgba, icon-256.png")


if __name__ == "__main__":
    main()

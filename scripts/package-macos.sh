#!/bin/bash
# macOS 배포 패키징: haneng.app 번들 (표시기 데몬 + 설정 앱).
#
# 사용:
#   scripts/package-macos.sh
#   SIGN_IDENTITY="Developer ID Application: ..." scripts/package-macos.sh  # 서명
#
# 로그인 자동 시작: 앱을 /Applications로 옮기고 시스템 설정 → 일반 →
#   로그인 항목에 추가 (또는 dist/kr.haneng.indicator.plist를 ~/Library/LaunchAgents/).
# 실행에는 손쉬운 사용(Accessibility) 권한이 필요하다.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)
APP=dist/haneng.app

cargo build --release -p haneng-macos -p haneng-settings

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/hanengd "$APP/Contents/MacOS/"
cp target/release/haneng-settings "$APP/Contents/MacOS/"

# 아이콘: icon-256.png → haneng.icns.
if command -v iconutil >/dev/null && command -v sips >/dev/null; then
    ICONSET=$(mktemp -d)/haneng.iconset
    mkdir -p "$ICONSET"
    for s in 16 32 64 128 256; do
        sips -z $s $s assets/icon-256.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
    done
    cp assets/icon-256.png "$ICONSET/icon_256x256.png"
    iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/haneng.icns" || true
fi

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key><string>kr.haneng.indicator</string>
    <key>CFBundleName</key><string>haneng</string>
    <key>CFBundleExecutable</key><string>hanengd</string>
    <key>CFBundleIconFile</key><string>haneng</string>
    <key>CFBundleVersion</key><string>${VERSION}</string>
    <key>CFBundleShortVersionString</key><string>${VERSION}</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSUIElement</key><true/>
</dict>
</plist>
PLIST

cat > dist/kr.haneng.indicator.plist <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>kr.haneng.indicator</string>
    <key>ProgramArguments</key>
    <array><string>/Applications/haneng.app/Contents/MacOS/hanengd</string></array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>
</dict>
</plist>
PLIST

if [[ -n "${SIGN_IDENTITY:-}" ]]; then
    codesign --force --options runtime --sign "$SIGN_IDENTITY" \
        "$APP/Contents/MacOS/hanengd" "$APP/Contents/MacOS/haneng-settings" "$APP"
    echo "서명 완료: $SIGN_IDENTITY"
else
    echo "SIGN_IDENTITY 미지정 — 서명 없이 패키징 (첫 실행 시 우클릭 → 열기)"
fi

(cd dist && rm -f haneng-macos.zip && zip -qr haneng-macos.zip haneng.app kr.haneng.indicator.plist)
echo "완료: $APP, dist/haneng-macos.zip (v${VERSION})"

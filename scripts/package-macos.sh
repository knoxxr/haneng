#!/bin/bash
# macOS 배포 패키징: haneng.app 번들 생성 (+ 선택적 서명/zip).
#
# 사용:
#   scripts/package-macos.sh                 # dist/haneng.app + dist/haneng-macos.zip
#   SIGN_IDENTITY="Developer ID Application: ..." scripts/package-macos.sh
#     → codesign까지 수행. 공증(notarization)은 별도로:
#       xcrun notarytool submit dist/haneng-macos.zip --keychain-profile <profile> --wait
#
# 로그인 시 자동 시작: 생성된 앱을 /Applications로 옮긴 뒤
#   시스템 설정 → 일반 → 로그인 항목에 haneng.app 추가 (또는 아래 LaunchAgent):
#   cp dist/kr.haneng.daemon.plist ~/Library/LaunchAgents/ && launchctl load ~/Library/LaunchAgents/kr.haneng.daemon.plist
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' crates/haneng-macos/Cargo.toml | cut -d'"' -f2)
APP=dist/haneng.app

cargo build --release -p haneng-macos -p haneng-settings

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"

cp target/release/hanengd "$APP/Contents/MacOS/"
cp target/release/haneng-settings "$APP/Contents/MacOS/"

cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>kr.haneng.daemon</string>
    <key>CFBundleName</key>
    <string>haneng</string>
    <key>CFBundleExecutable</key>
    <string>hanengd</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <!-- 메뉴바 상주 앱: Dock 아이콘 없음 -->
    <key>LSUIElement</key>
    <true/>
</dict>
</plist>
EOF

# 로그인 자동 시작용 LaunchAgent 템플릿.
cat > dist/kr.haneng.daemon.plist <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>kr.haneng.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>/Applications/haneng.app/Contents/MacOS/hanengd</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
EOF

if [[ -n "${SIGN_IDENTITY:-}" ]]; then
    codesign --force --options runtime --sign "$SIGN_IDENTITY" \
        "$APP/Contents/MacOS/hanengd" "$APP/Contents/MacOS/haneng-settings" "$APP"
    echo "서명 완료: $SIGN_IDENTITY"
else
    echo "SIGN_IDENTITY 미지정 — 서명 없이 패키징 (배포 시 Gatekeeper 경고 발생)"
fi

(cd dist && rm -f haneng-macos.zip && zip -qr haneng-macos.zip haneng.app kr.haneng.daemon.plist)
echo "완료: $APP, dist/haneng-macos.zip (v${VERSION})"

#!/bin/bash
# Linux(X11) 배포 패키징: 표시기 데몬 + 설정 앱 + autostart 템플릿 → tar.gz.
# 실험적 빌드 — 실기기 검증 전.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release -p haneng-linux -p haneng-settings

mkdir -p dist
cp target/release/hanengl target/release/haneng-settings dist/

cat > dist/haneng.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=haneng
Comment=한/영 입력 상태 표시기
Exec=/usr/local/bin/hanengl
X-GNOME-Autostart-enabled=true
DESKTOP

tar -czf dist/haneng-linux-x11.tar.gz -C dist hanengl haneng-settings haneng.desktop
echo "완료: dist/haneng-linux-x11.tar.gz"

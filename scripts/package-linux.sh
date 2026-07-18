#!/bin/bash
# Linux(X11) 배포 패키징: 릴리스 빌드 + tar.gz (+ .desktop 자동 시작 템플릿).
#
# 사용: scripts/package-linux.sh
# 자동 시작: dist/haneng.desktop을 ~/.config/autostart/에 복사.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release -p haneng-linux -p haneng-settings

mkdir -p dist
cp target/release/hanengl target/release/haneng-settings dist/

cat > dist/haneng.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=haneng
Comment=한/영 입력 모드 오타 자동 교정
Exec=/usr/local/bin/hanengl
X-GNOME-Autostart-enabled=true
EOF

tar -czf dist/haneng-linux-x11.tar.gz -C dist hanengl haneng-settings haneng.desktop
echo "완료: dist/haneng-linux-x11.tar.gz"

#!/bin/bash
# 릴리스 절차: 버전 올림 → CHANGELOG 확정 → 커밋 → 태그.
#
# 사용: scripts/release.sh 0.2.0
# 이후: git push origin main --tags  → release.yml이 3개 OS 패키지를
#       드래프트 릴리스에 첨부한다.
set -euo pipefail
cd "$(dirname "$0")/.."

NEW="${1:?사용법: scripts/release.sh <새 버전 예: 0.2.0>}"
[[ "$NEW" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "SemVer 형식이 아님: $NEW"; exit 1; }
[[ -z "$(git status --porcelain)" ]] || { echo "작업 트리가 깨끗하지 않음 — 먼저 커밋/스태시하세요"; exit 1; }

OLD=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)
echo "버전: $OLD → $NEW"

# 워크스페이스 공유 버전이 유일한 버전 선언이다.
sed -i '' "0,/^version = \"$OLD\"/s//version = \"$NEW\"/" Cargo.toml 2>/dev/null \
    || sed -i "0,/^version = \"$OLD\"/s//version = \"$NEW\"/" Cargo.toml
cargo check -q 2>/dev/null || true   # Cargo.lock 버전 갱신

# CHANGELOG: Unreleased 아래에 새 버전 섹션 삽입.
TODAY=$(date +%F)
python3 - "$NEW" "$TODAY" <<'EOF'
import sys
new, today = sys.argv[1], sys.argv[2]
path = "CHANGELOG.md"
text = open(path).read()
marker = "## [Unreleased]\n"
assert marker in text, "CHANGELOG.md에 [Unreleased] 섹션이 없음"
text = text.replace(marker, f"{marker}\n## [{new}] - {today}\n", 1)
open(path, "w").write(text)
EOF

cargo test -q >/dev/null || { echo "테스트 실패 — 릴리스 중단"; git checkout -- .; exit 1; }

git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "release: v$NEW"
git tag "v$NEW"
echo "완료. 배포하려면: git push origin main --tags"

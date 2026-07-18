# haneng — 한/영 입력 모드 오타 자동 교정

`gksrmf`라고 쳐 본 적 있다면, 이 도구가 필요합니다.

haneng은 한글/영문 입력 모드를 착각하고 친 텍스트를 감지해 자동으로
바로잡는 데스크톱 상주 유틸리티입니다. IME를 교체하지 않고 기존 입력기
위에서 동작하며, Windows · macOS · Linux(X11)를 지원합니다.

```
입력:  dkssudgktpdy!  (영문 모드에서 "안녕하세요!"를 침)
결과:  안녕하세요!     ← 스페이스/문장부호 입력 순간 자동 교정 + 모드 전환

입력:  ㅗ디ㅣㅐ        (한글 모드에서 "hello"를 침)
결과:  hello
```

## 기능

- **단어 경계 자동 교정** — 사전(영 1만/한 2만 단어) + 구조 분석으로 판정.
  실측 정확도: 오발동 0건, recall 98~99% (기본 민감도 기준, 말뭉치 시뮬레이션).
- **백스페이스 1회 되돌리기** — 자동 교정 직후 백스페이스를 누르면 원문
  복원 + 그 단어를 다시는 건드리지 않도록 학습.
- **수동 변환 핫키** — macOS `⌘⇧Space`, Windows/Linux `Ctrl+Shift+Space`로
  마지막 단어를 즉시 반대 모드로 변환 (다시 누르면 되돌아옴).
- **두벌식·세벌식(390/최종)** 자판 지원, 민감도 3단계, 앱별 비활성화.
- 트레이(메뉴바) 토글 + 설정 창(`haneng-settings`).

## 프라이버시 원칙

키보드 후킹 도구는 신뢰가 전부입니다. haneng은:

- 모든 처리가 **로컬**입니다. 네트워크 코드 자체가 없습니다.
- 키 입력을 디스크에 기록하지 않습니다. 메모리에도 마지막 1~2 단어만 유지합니다.
- **비밀번호 필드에서는 동작하지 않습니다** (macOS 보안 입력 감지, Windows
  ES_PASSWORD; Linux는 AT-SPI 연동 전까지 미지원 — 알려진 한계).
- 전체 소스가 공개돼 있어 직접 검증할 수 있습니다.

## 설치 / 실행

**[📦 최신 릴리스 다운로드](https://github.com/knoxxr/haneng/releases/latest)**

| OS | 파일 | 비고 |
|---|---|---|
| macOS 13+ | `haneng-macos.zip` | 압축 해제 후 haneng.app 실행. 시스템 설정 → 개인정보 보호 및 보안 → **손쉬운 사용 + 입력 모니터링** 권한 필요. 서명이 없어 첫 실행은 우클릭 → 열기 |
| Windows | `haneng-windows.zip` | `hanengw.exe` 실행 (SmartScreen 경고 시 "추가 정보 → 실행"). 관리자 권한 앱에는 훅이 닿지 않음 |
| Linux (X11) | `haneng-linux-x11.tar.gz` | `hanengl` 실행. 시작 모드를 영문으로 가정하고 한/영 키를 관찰해 추적. Wayland는 미지원 (Fcitx5/IBus 플러그인 트랙 진행 중) |

소스 빌드 (Rust 필요):

```sh
cargo build --release   # target/release/{hanengd|hanengw|hanengl}, haneng-settings
```

설정은 트레이 메뉴 → "설정..." 또는 `haneng-settings` 실행. 설정 파일 위치:
macOS `~/Library/Application Support/haneng/`, Windows `%APPDATA%\haneng\`,
Linux `~/.config/haneng/`.

```ini
# config.txt 예시
auto = on
sensitivity = balanced        # conservative | balanced | aggressive
layout = dubeolsik            # dubeolsik | sebeolsik-390 | sebeolsik-final
disabled_apps = terminal, ssh
```

## 개발

```sh
cargo test                    # 전체 테스트 (정확도 게이트 포함)
cargo clippy --all-targets    # 린트
cargo run -q -p haneng-cli -- "gksrmf dlqfur"   # 엔진 CLI 데모 → "한글 입력"
```

아키텍처와 개발 규약은 [CLAUDE.md](CLAUDE.md), 전체 계획은 [PLAN.md](PLAN.md),
서드파티 데이터 출처는 [NOTICE.md](NOTICE.md) 참고.

## 버전 관리 / 릴리스

- 버전은 [SemVer](https://semver.org/lang/ko/)를 따르고, 루트 `Cargo.toml`의
  `[workspace.package] version`이 **모든 크레이트가 공유하는 단일 출처**입니다.
- 변경 이력은 [CHANGELOG.md](CHANGELOG.md)에 기록합니다 (Keep a Changelog 포맷).
  개발 중에는 `[Unreleased]` 섹션에 쌓습니다.
- 릴리스 절차:

  ```sh
  scripts/release.sh 0.2.0        # 버전 올림 + CHANGELOG 확정 + 테스트 + 커밋 + 태그
  git push origin main --tags     # → CI가 3개 OS 패키지를 드래프트 릴리스에 첨부
  ```

## 라이선스

MIT — [LICENSE](LICENSE)

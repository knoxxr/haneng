# haneng — 한/영 상태 표시기

지금 한글 모드인지 영문 모드인지, 타이핑하기 전에는 알 수 없어서
`gksrmf`라고 쳐 본 적 있다면 — 이 도구가 알려줍니다.

haneng은 **마우스를 입력창 위에 올리면 커서 옆에 현재 입력 상태 배지**를
띄우는 상주 유틸리티입니다. Windows · macOS · Linux(X11)를 지원합니다.

- 파란 **한** — 한글 모드
- 회색 **a** — 영문 소문자
- 주황 **A** — 영문 + Caps Lock (실수로 켜둔 Caps Lock을 한눈에)

## 동작 방식과 프라이버시

- 텍스트 입력 영역 판별: Windows는 I-빔 커서, macOS는 Accessibility 요소
  역할, Linux는 커서 이름으로 감지
- 모드 판별: Windows는 IME 상태 조회, macOS는 입력 소스 조회
- **텍스트를 지우거나 입력하지 않습니다.** 커서 위치와 모드만 읽습니다.
- **네트워크 없음** — 설정 창에서 업데이트 확인 버튼을 눌렀을 때만 예외
- Windows/macOS는 키보드를 관찰하지 않습니다. Linux만 예외로, X11에 IME
  상태 조회 API가 없어 한/영 토글 키를 관찰해 모드를 추적합니다(토글
  여부만 — 입력 내용은 보지 않음).

> 참고: v0.3.x까지 있던 한↔영 자동/수동 변환 기능은 v0.4.0에서
> 제거했습니다 (git 태그 `v0.3.2`에 전체 구현이 보존되어 있습니다).

## 설치

**[📦 최신 릴리스 다운로드](https://github.com/knoxxr/haneng/releases/latest)**

| OS | 파일 | 참고 |
|---|---|---|
| Windows | `haneng-windows.msi` (권장) / `.zip` | 설치 즉시 실행 + 로그인 자동 시작. SmartScreen 경고 시 "추가 정보 → 실행" |
| macOS 11+ (Apple Silicon) | `haneng-macos.zip` | 아래 "macOS 첫 실행" 참고. 실행에는 **손쉬운 사용** 권한 필요 |
| Linux (X11) | `haneng-linux-x11.tar.gz` | `hanengl` 실행. **실험적** — Wayland 미지원 |

### macOS 첫 실행

haneng.app은 Apple 유료 인증서로 서명·공증되지 않았습니다(ad-hoc 서명만).
그래서 다운로드하면 격리(quarantine) 속성이 붙어, macOS가 "손상되었기
때문에 열 수 없습니다"라며 **휴지통으로 보낼 수 있습니다.** 이는 손상이
아니라 서명 미검증 때문이며, 격리 속성을 지우면 정상 실행됩니다:

```sh
# haneng.app을 응용 프로그램 폴더로 옮긴 뒤 (다른 위치면 그 경로로):
xattr -dr com.apple.quarantine /Applications/haneng.app
open /Applications/haneng.app
```

이후 시스템 설정 → 개인정보 보호 및 보안 → **손쉬운 사용**에 haneng을
추가해야 배지가 동작합니다.

트레이/메뉴바 아이콘: 배지 표시 토글 · 설정 · 종료.
설정 창에서 **업데이트 확인** 버튼으로 새 버전을 설치할 수 있습니다
(Windows는 원클릭, macOS/Linux는 다운로드 페이지 열기).

## 설정

설정 창에서 편집하거나 config 파일을 직접 수정합니다 (macOS
`~/Library/Application Support/haneng/`, Windows `%APPDATA%\haneng\`,
Linux `~/.config/haneng/`):

```ini
hover_indicator = on     # off = 배지 끔
initial_mode = korean    # 모드 조회 불가 환경의 초기 표시 (korean|english)
ime_query = on           # (Windows) off = 실시간 IME 조회 끔
linux_toggle_keycodes = 130,108   # (Linux) 한/영 토글 키코드
```

## 알려진 한계

- 커스텀 마우스 커서 테마에서는 텍스트 영역 감지가 안 될 수 있습니다.
- Linux는 실기기 검증 전이며, 한/영은 토글 키 관찰로 추적하므로 시작
  시점이 어긋나면 토글을 한 번 눌러 맞춥니다. Wayland는 지원하지 않습니다.
- 서명/공증이 없어 OS 보안 경고가 뜹니다.

## 개발

```sh
cargo test                                              # 전체 테스트
cargo clippy --all-targets                              # 린트
cargo run -p haneng-macos                               # macOS 데몬 (이 저장소는 macOS에서 개발)
cargo check -p haneng-windows --target x86_64-pc-windows-msvc
cargo check -p haneng-linux   --target x86_64-unknown-linux-gnu
```

아키텍처는 [CLAUDE.md](CLAUDE.md), 서드파티 데이터 출처는 [NOTICE.md](NOTICE.md) 참고.

## 버전 관리 / 릴리스

- [SemVer](https://semver.org/lang/ko/), 루트 `Cargo.toml`의
  `[workspace.package] version`이 단일 출처. 이력은 [CHANGELOG.md](CHANGELOG.md).
- 릴리스: `scripts/release.sh <버전>` 후 `git push origin main --tags`
  → CI가 3개 OS 패키지를 빌드해 드래프트 릴리스에 첨부.

## 라이선스

MIT — [LICENSE](LICENSE)

# haneng — 한/영 상태 표시기

지금 한글 모드인지 영문 모드인지, 타이핑하기 전에는 알 수 없어서
`gksrmf`라고 쳐 본 적 있다면 — 이 도구가 알려줍니다.

haneng은 **마우스를 입력창 위에 올리면 커서 옆에 현재 한/영 상태 배지**
(파란 **한** / 회색 **A**)를 표시하는 Windows 상주 유틸리티입니다.

## 동작 방식과 프라이버시

- 텍스트 입력 영역 판별: 마우스 커서가 I-빔 모양인지 확인 (앱 무관)
- 모드 판별: 포커스된 입력창의 IME 상태를 OS에 실시간 질의
- **키보드 후킹 없음** — 키 입력을 전혀 관찰하지 않습니다
- **텍스트 조작 없음** — 아무것도 지우거나 입력하지 않습니다
- **네트워크 없음** — 설정 창에서 업데이트 확인 버튼을 눌렀을 때만 예외

> 참고: v0.3.x까지 있던 한↔영 자동/수동 변환 기능은 v0.4.0에서
> 제거했습니다 (git 태그 `v0.3.2`에 전체 구현이 보존되어 있습니다).

## 설치

**[📦 최신 릴리스 다운로드](https://github.com/knoxxr/haneng/releases/latest)**

- `haneng-windows.msi` (권장): 설치 즉시 실행 + 로그인 자동 시작 등록.
  새 버전 MSI를 설치하면 자동 업그레이드됩니다.
- `haneng-windows.zip` (포터블): `hanengw.exe` 실행.
- 서명이 없어 SmartScreen 경고가 뜨면 "추가 정보 → 실행".

트레이 아이콘 메뉴: 배지 표시 토글 · 설정 · 종료.
설정 창에서 **업데이트 확인** 버튼으로 새 버전을 원클릭 설치할 수 있습니다.

## 설정

`%APPDATA%\haneng\config.txt` (설정 창에서도 편집 가능):

```ini
hover_indicator = on     # off = 배지 끔 (시작 기본값)
initial_mode = korean    # IME가 상태 조회에 응답하지 않을 때의 초기 표시
ime_query = on           # off = 실시간 질의 끔
```

## 알려진 한계

- 커스텀 마우스 커서 테마에서는 I-빔 감지가 안 될 수 있습니다.
- 일부 앱/IME은 상태 조회에 응답하지 않습니다 — 그 경우 마지막으로
  알아낸 값을 표시하며, `initial_mode`로 초기값을 지정할 수 있습니다.

## 개발

```sh
cargo test                    # 전체 테스트
cargo clippy --all-targets    # 린트
cargo check -p haneng-windows --target x86_64-pc-windows-msvc   # 크로스 체크
```

## 버전 관리 / 릴리스

- 버전은 [SemVer](https://semver.org/lang/ko/), 루트 `Cargo.toml`의
  `[workspace.package] version`이 단일 출처입니다.
- 변경 이력: [CHANGELOG.md](CHANGELOG.md)
- 릴리스: `scripts/release.sh <버전>` 후 `git push origin main --tags`
  → CI가 MSI/zip을 빌드해 드래프트 릴리스에 첨부합니다.

## 라이선스

MIT — [LICENSE](LICENSE). 서드파티 데이터 출처: [NOTICE.md](NOTICE.md)

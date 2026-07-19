# Changelog

포맷: [Keep a Changelog](https://keepachangelog.com/ko/1.1.0/),
버전: [SemVer](https://semver.org/lang/ko/). 릴리스 절차는 `scripts/release.sh` 참고.

## [Unreleased]

## [0.2.0] - 2026-07-19

### Added
- 설정 창에 **업데이트 확인/설치 버튼** — 버튼을 눌렀을 때만 GitHub 릴리스를
  조회한다(자동 확인 없음, 데몬은 여전히 네트워크 코드 없음). 새 버전이
  있으면 Windows는 MSI를 내려받아 원클릭 업그레이드, macOS/Linux는
  다운로드 페이지를 연다.

## [0.1.2] - 2026-07-19

### Changed
- **Windows를 띄어쓰기 기준 수동 변환 전용으로 재설계** (실사용 피드백 반영)
  - 치환을 백스페이스 개수 계산에서 **Ctrl+Shift+Left 선택 위에 타이핑**으로
    변경 — 핫키를 반복해 눌러도 문장이 지워지지 않는다
  - IME 모드를 `WM_IME_CONTROL` 질의(Win11 신형 한글 IME가 응답하지 않음)
    대신 **한/영 키 관찰로 추적**하고, 전환도 한/영 키 주입으로 수행
  - 자동 교정은 Windows에서 비활성 (트레이 토글도 제거), 공백만 단어 경계
  - 변환 대상은 공백으로 확정된 마지막 단어만 (조합 중 preedit 불간섭)

## [0.1.1] - 2026-07-19

### Fixed
- Windows: 릴리스 빌드에서 콘솔 창이 뜨지 않고 트레이로만 상주
  (콘솔을 닫으면 데몬이 함께 종료되던 문제)

### Added
- Windows: MSI 설치 파일 (`haneng-windows.msi`) — Program Files 설치,
  로그인 자동 시작 등록, 설치 직후 자동 실행, 업그레이드 지원

## [0.1.0] - 2026-07-18

첫 릴리스.

### Added
- 코어 엔진: 두벌식 오토마타(도깨비불 포함) 기반 한↔영 양방향 변환,
  세벌식 390/최종 자판(libhangul 데이터 기반) 지원
- 잘못된 입력 모드 감지기: 구조 게이트 + 사전(영 1만/한 2만) + bigram
  음성 필터, 민감도 3단계 — 실측 오발동 0건 / recall 98~99% (Balanced)
- 단어 경계 자동 교정 + 백스페이스 1회 되돌리기(undo) + 예외 사전 학습·영속화
- 수동 변환 핫키 (macOS ⌘⇧Space, Windows/Linux Ctrl+Shift+Space) — 재입력 시 토글
- 상주 데몬: macOS(`hanengd`, CGEventTap), Windows(`hanengw`, WH_KEYBOARD_LL),
  Linux X11(`hanengl`, XRecord/XTest)
- 보안: 비밀번호 필드 차단(macOS 보안 입력·Windows ES_PASSWORD),
  주입 이벤트 마커, 키 입력 미기록(메모리에 마지막 1~2 단어만)
- 트레이(메뉴바) 토글, 설정 창(`haneng-settings`), 앱별 비활성화(`disabled_apps`)
- C FFI(`libhaneng` + `haneng.h`) — Wayland용 Fcitx5/IBus 플러그인 트랙 기반
- CI(3개 OS 테스트·린트) 및 태그 릴리스 파이프라인, OS별 패키징 스크립트

### 알려진 한계
- Linux: 비밀번호 필드 감지 없음(AT-SPI 예정), IME 모드는 한/영 키 관찰로 추적
- Wayland 미지원 (Fcitx5/IBus 플러그인 트랙 진행 중)
- 실기기 검증 전 (3개 OS 모두)

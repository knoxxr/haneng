/* haneng — 한/영 입력 모드 오타 감지·변환 엔진의 C API.
 *
 * crates/haneng-ffi/src/lib.rs와 동기 유지할 것.
 * 모든 문자열은 NUL 종단 UTF-8이며, 반환된 char*는 haneng_free()로 해제한다.
 *
 * 빌드: cargo build -p haneng-ffi --release
 *   → target/release/libhaneng.{a,so,dylib}
 */
#ifndef HANENG_H
#define HANENG_H

#ifdef __cplusplus
extern "C" {
#endif

/* haneng_detector_analyze의 반환 코드 */
#define HANENG_KEEP 0
#define HANENG_TO_HANGUL 1
#define HANENG_TO_ENGLISH 2

/* 민감도 */
#define HANENG_CONSERVATIVE 0
#define HANENG_BALANCED 1
#define HANENG_AGGRESSIVE 2

typedef struct HanengDetector HanengDetector;

/* 영문 모드로 잘못 친 문자열 → 한글 ("gksrmf" → "한글"). */
char *haneng_eng_to_han(const char *text);

/* 한글 모드로 잘못 친 문자열 → 영문 ("ㅗ디ㅣㅐ" → "hello"). */
char *haneng_han_to_eng(const char *text);

/* 이 라이브러리가 반환한 문자열 해제. NULL 허용. */
void haneng_free(char *s);

HanengDetector *haneng_detector_new(int sensitivity);
void haneng_detector_free(HanengDetector *detector);

/* 단어 하나를 판정한다. 변환이 필요하면 *converted에 결과를 담는다
 * (converted는 NULL 가능). 반환: HANENG_KEEP/TO_HANGUL/TO_ENGLISH. */
int haneng_detector_analyze(const HanengDetector *detector, const char *word,
                            char **converted);

/* 되돌리기 학습: 이 단어를 다시는 자동 변환하지 않는다. */
void haneng_detector_record_undo(HanengDetector *detector, const char *word);

#ifdef __cplusplus
}
#endif

#endif /* HANENG_H */

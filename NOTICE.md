# 서드파티 데이터 출처

haneng 코드는 MIT 라이선스이며(LICENSE 참고), 아래 외부 데이터에서 생성한
테이블을 포함한다. 생성 절차는 `crates/haneng-datagen/src/main.rs` 참고.

## FrequencyWords (사전·bigram 통계)

`crates/haneng-core/src/lexicon_data.rs`의 단어 사전과 bigram 로그확률은
OpenSubtitles 2018 말뭉치 기반 빈도 목록에서 생성했다.

- 출처: <https://github.com/hermitdave/FrequencyWords>
- 라이선스: CC-BY-SA 4.0 (해당 데이터 부분에 적용)

## libhangul (세벌식 자판 배열)

`crates/haneng-core/src/sebeolsik_data.rs`의 세벌식 390/최종 키 배열은
libhangul의 자판 정의 파일에서 추출한 사실 정보다.

- 출처: <https://github.com/libhangul/libhangul>
  (`data/keyboards/hangul-keyboard-39.xml.template`, `hangul-keyboard-3f.xml.template`)

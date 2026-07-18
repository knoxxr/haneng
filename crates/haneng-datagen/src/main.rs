//! haneng-datagen — 빈도 단어 목록에서 감지용 임베드 테이블을 생성한다.
//!
//! 입력 (저장소에 커밋하지 않음, `data/`):
//! ```text
//! curl -sL -o data/ko_50k.txt https://raw.githubusercontent.com/hermitdave/FrequencyWords/master/content/2018/ko/ko_50k.txt
//! curl -sL -o data/en_50k.txt https://raw.githubusercontent.com/hermitdave/FrequencyWords/master/content/2018/en/en_50k.txt
//! curl -sL -o data/390.xml https://raw.githubusercontent.com/libhangul/libhangul/main/data/keyboards/hangul-keyboard-39.xml.template
//! curl -sL -o data/3f.xml  https://raw.githubusercontent.com/libhangul/libhangul/main/data/keyboards/hangul-keyboard-3f.xml.template
//! ```
//! 출력: `crates/haneng-core/src/lexicon_data.rs`,
//!       `crates/haneng-core/src/sebeolsik_data.rs` (커밋 대상).
//!
//! 실행: 워크스페이스 루트에서 `cargo run -p haneng-datagen`.
//!
//! 생성물:
//! - EN_WORDS / KO_WORDS: 빈도 상위 사전 (정렬, 이진 탐색용)
//! - EN_BIGRAM_LP: 문자 bigram 조건부 로그확률 P(다음|이전), ln×256 양자화.
//!   심벌 0 = 단어 경계, 1..=26 = a-z.
//! - KO_JAMO_BIGRAM_LP: 자모 bigram (음절을 초/중/종성 호환 자모로 분해한
//!   스트림). 심벌 0 = 단어 경계, 1..=51 = U+3131..U+3163.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;

const EN_DICT_SIZE: usize = 10_000;
const KO_DICT_SIZE: usize = 20_000;
const EN_SYMBOLS: usize = 27;
const KO_SYMBOLS: usize = 52;
/// 미관측 전이의 로그확률 (ln, 양자화 전).
const MISSING_LP: f64 = -12.0;

// ---- 한글 분해 (haneng-core와 동일한 테이블; datagen은 core에 의존하지
// 않아야 부트스트랩이 가능하므로 여기 복제한다) ----

const CHO_COMPAT: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ',
    'ㅌ', 'ㅍ', 'ㅎ',
];
const JUNG_COMPAT: [char; 21] = [
    'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ',
    'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
];
const JONG_COMPAT: [char; 27] = [
    'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ', 'ㅀ', 'ㅁ',
    'ㅂ', 'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];

fn is_syllable(c: char) -> bool {
    ('\u{AC00}'..='\u{D7A3}').contains(&c)
}

/// 한글 단어 → 자모 심벌 인덱스 스트림 (경계 미포함).
fn jamo_symbols(word: &str) -> Vec<usize> {
    let mut out = Vec::new();
    for c in word.chars() {
        assert!(is_syllable(c), "KO_WORDS는 완성형 음절만 담는다");
        let offset = c as u32 - 0xAC00;
        let jong = (offset % 28) as usize;
        let jung = ((offset / 28) % 21) as usize;
        let cho = (offset / 28 / 21) as usize;
        out.push(jamo_index(CHO_COMPAT[cho]));
        out.push(jamo_index(JUNG_COMPAT[jung]));
        if jong > 0 {
            out.push(jamo_index(JONG_COMPAT[jong - 1]));
        }
    }
    out
}

fn jamo_index(c: char) -> usize {
    (c as u32 - 0x3131 + 1) as usize
}

fn en_symbols(word: &str) -> Vec<usize> {
    word.bytes().map(|b| (b - b'a' + 1) as usize).collect()
}

// ---- bigram 집계 ----

/// 빈도 가중 bigram 카운트를 행 정규화한 조건부 로그확률(ln×256, i16)로.
fn bigram_table(entries: &[(Vec<usize>, f64)], symbols: usize) -> Vec<Vec<i16>> {
    let mut counts = vec![vec![0f64; symbols]; symbols];
    for (stream, weight) in entries {
        let mut prev = 0usize; // 경계
        for &s in stream {
            counts[prev][s] += weight;
            prev = s;
        }
        counts[prev][0] += weight; // 끝 경계
    }
    counts
        .iter()
        .map(|row| {
            let total: f64 = row.iter().sum();
            row.iter()
                .map(|&c| {
                    if c > 0.0 && total > 0.0 {
                        ((c / total).ln().max(MISSING_LP) * 256.0).round() as i16
                    } else {
                        (MISSING_LP * 256.0) as i16
                    }
                })
                .collect()
        })
        .collect()
}

fn parse_list(path: &str) -> Vec<(String, f64)> {
    fs::read_to_string(path)
        .unwrap_or_else(|e| {
            panic!("{path} 읽기 실패 ({e}) — 파일 상단 주석의 curl 명령으로 내려받으세요")
        })
        .lines()
        .filter_map(|line| {
            let (word, freq) = line.trim().rsplit_once(' ')?;
            Some((word.to_string(), freq.parse::<f64>().ok()?))
        })
        .collect()
}

fn write_word_array(out: &mut String, name: &str, words: &BTreeSet<String>) {
    writeln!(out, "pub static {name}: [&str; {}] = [", words.len()).unwrap();
    for w in words {
        writeln!(out, "    {w:?},").unwrap();
    }
    writeln!(out, "];").unwrap();
}

fn write_bigram(out: &mut String, name: &str, table: &[Vec<i16>], symbols: usize) {
    writeln!(out, "pub static {name}: [[i16; {symbols}]; {symbols}] = [").unwrap();
    for row in table {
        let cells: Vec<String> = row.iter().map(|v| v.to_string()).collect();
        writeln!(out, "    [{}],", cells.join(", ")).unwrap();
    }
    writeln!(out, "];").unwrap();
}

// ---- 세벌식 자판 (libhangul XML) ----

/// 조합형 자모 코드포인트 → (종류, 호환 자모). 종류: 0=초성, 1=중성, 2=종성, 3=일반 문자.
fn positioned_jamo(value: u32) -> (u8, char) {
    match value {
        0x1100..=0x1112 => (0, CHO_COMPAT[(value - 0x1100) as usize]),
        0x1161..=0x1175 => (1, JUNG_COMPAT[(value - 0x1161) as usize]),
        0x11A8..=0x11C2 => (2, JONG_COMPAT[(value - 0x11A8) as usize]),
        _ => (
            3,
            char::from_u32(value).expect("valid scalar in keyboard xml"),
        ),
    }
}

/// libhangul hangul-keyboard XML(template)에서 (ASCII 키, 종류, 자모) 목록 추출.
fn parse_keyboard_xml(path: &str) -> Vec<(char, u8, char)> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| {
        panic!("{path} 읽기 실패 ({e}) — 파일 상단 주석의 curl 명령으로 내려받으세요")
    });
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("<item key=\"0x") else {
            continue;
        };
        let Some((key_hex, rest)) = rest.split_once("\" value=\"0x") else {
            continue;
        };
        let Some((value_hex, _)) = rest.split_once('"') else {
            continue;
        };
        let key = u32::from_str_radix(key_hex, 16).expect("hex key");
        let value = u32::from_str_radix(value_hex, 16).expect("hex value");
        let key_char = char::from_u32(key).expect("ascii key");
        let (kind, jamo) = positioned_jamo(value);
        out.push((key_char, kind, jamo));
    }
    assert_eq!(
        out.len(),
        94,
        "{path}: 94개의 printable ASCII 항목이어야 함"
    );
    out
}

fn write_keyboard(out: &mut String, name: &str, entries: &[(char, u8, char)]) {
    writeln!(
        out,
        "/// (키, 종류: 0=초성 1=중성 2=종성 3=일반, 자모/문자)\n\
         pub static {name}: [(char, u8, char); {}] = [",
        entries.len()
    )
    .unwrap();
    for (key, kind, jamo) in entries {
        writeln!(out, "    ({key:?}, {kind}, {jamo:?}),").unwrap();
    }
    writeln!(out, "];").unwrap();
}

fn main() {
    let en_raw = parse_list("data/en_50k.txt");
    let ko_raw = parse_list("data/ko_50k.txt");

    // 세벌식 자판 테이블.
    let s390 = parse_keyboard_xml("data/390.xml");
    let s3f = parse_keyboard_xml("data/3f.xml");
    let mut kb_out = String::new();
    writeln!(
        kb_out,
        "//! 자동 생성 파일 — 직접 수정하지 말 것. 재생성: `cargo run -p haneng-datagen`\n\
         //!\n\
         //! 출처: libhangul 자판 데이터 <https://github.com/libhangul/libhangul>\n\
         //! (LGPL-2.1 데이터 파일에서 추출한 키 배열 사실 정보)\n"
    )
    .unwrap();
    write_keyboard(&mut kb_out, "SEBEOLSIK_390", &s390);
    write_keyboard(&mut kb_out, "SEBEOLSIK_FINAL", &s3f);
    fs::write("crates/haneng-core/src/sebeolsik_data.rs", kb_out).expect("자판 테이블 쓰기");

    // 영어: a-z로만 이루어진 토큰.
    let en: Vec<(String, f64)> = en_raw
        .into_iter()
        .filter(|(w, _)| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase()))
        .collect();
    // 한국어: 완성형 음절로만 이루어진 토큰.
    let ko: Vec<(String, f64)> = ko_raw
        .into_iter()
        .filter(|(w, _)| !w.is_empty() && w.chars().all(is_syllable))
        .collect();

    let en_dict: BTreeSet<String> = en
        .iter()
        .take(EN_DICT_SIZE)
        .map(|(w, _)| w.clone())
        .collect();
    let ko_dict: BTreeSet<String> = ko
        .iter()
        .take(KO_DICT_SIZE)
        .map(|(w, _)| w.clone())
        .collect();

    let en_streams: Vec<(Vec<usize>, f64)> = en.iter().map(|(w, f)| (en_symbols(w), *f)).collect();
    let ko_streams: Vec<(Vec<usize>, f64)> =
        ko.iter().map(|(w, f)| (jamo_symbols(w), *f)).collect();

    let en_bigram = bigram_table(&en_streams, EN_SYMBOLS);
    let ko_bigram = bigram_table(&ko_streams, KO_SYMBOLS);

    let mut out = String::new();
    writeln!(
        out,
        "//! 자동 생성 파일 — 직접 수정하지 말 것. 재생성: `cargo run -p haneng-datagen`\n\
         //!\n\
         //! 출처: FrequencyWords (OpenSubtitles 2018 말뭉치 빈도 목록)\n\
         //! <https://github.com/hermitdave/FrequencyWords> — CC-BY-SA 4.0.\n\
         //! 이 파일의 데이터 부분은 위 라이선스의 저작자표시-동일조건변경허락을 따른다.\n\
         //!\n\
         //! - EN_WORDS/KO_WORDS: 빈도 상위 사전 (정렬됨, 이진 탐색용)\n\
         //! - *_BIGRAM_LP: 조건부 로그확률 ln P(다음|이전) × 256 (i16 양자화),\n\
         //!   심벌 0 = 단어 경계, 미관측 = {} ( = -12.0 × 256)\n",
        (MISSING_LP * 256.0) as i16
    )
    .unwrap();
    write_word_array(&mut out, "EN_WORDS", &en_dict);
    write_word_array(&mut out, "KO_WORDS", &ko_dict);
    write_bigram(&mut out, "EN_BIGRAM_LP", &en_bigram, EN_SYMBOLS);
    write_bigram(&mut out, "KO_JAMO_BIGRAM_LP", &ko_bigram, KO_SYMBOLS);

    let dest = "crates/haneng-core/src/lexicon_data.rs";
    fs::write(dest, out).expect("출력 파일 쓰기");
    println!(
        "생성 완료: {dest} (영어 사전 {}개, 한국어 사전 {}개)",
        en_dict.len(),
        ko_dict.len()
    );
}

//! haneng 엔진 CLI — 데모 겸 개발 도구.
//!
//! 인자로 준 텍스트(또는 stdin의 각 줄)를 단어 단위로 감지·교정해 출력한다.
//!
//! ```text
//! haneng "gksrmf dlqfur"        # 자동 감지 교정
//! haneng --to-hangul "gksrmf"   # 강제 영→한
//! haneng --to-english "한글"     # 강제 한→영
//! echo "..." | haneng -v        # 단어별 판정 표시
//! ```

use haneng_core::{english_to_hangul, hangul_to_english, Detector, Sensitivity, Verdict};
use std::io::BufRead;

enum Mode {
    Auto,
    ToHangul,
    ToEnglish,
}

fn main() {
    let mut mode = Mode::Auto;
    let mut sensitivity = Sensitivity::Balanced;
    let mut verbose = false;
    let mut text_args: Vec<String> = Vec::new();

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--to-hangul" => mode = Mode::ToHangul,
            "--to-english" => mode = Mode::ToEnglish,
            "--conservative" => sensitivity = Sensitivity::Conservative,
            "--aggressive" => sensitivity = Sensitivity::Aggressive,
            "-v" | "--verbose" => verbose = true,
            "--help" => {
                eprintln!("usage: haneng [--to-hangul|--to-english] [--conservative|--aggressive] [-v] [TEXT...]");
                eprintln!("TEXT가 없으면 stdin을 줄 단위로 처리한다.");
                return;
            }
            _ => text_args.push(arg),
        }
    }

    let detector = Detector::new(sensitivity);
    if text_args.is_empty() {
        for line in std::io::stdin().lock().lines() {
            let line = line.expect("stdin is valid UTF-8");
            println!("{}", process(&detector, &mode, verbose, &line));
        }
    } else {
        let line = text_args.join(" ");
        println!("{}", process(&detector, &mode, verbose, &line));
    }
}

fn process(detector: &Detector, mode: &Mode, verbose: bool, line: &str) -> String {
    match mode {
        Mode::ToHangul => english_to_hangul(line),
        Mode::ToEnglish => hangul_to_english(line),
        Mode::Auto => {
            let mut out = String::new();
            let mut word = String::new();
            for c in line.chars() {
                if c.is_whitespace() {
                    out.push_str(&correct_word(detector, verbose, &word));
                    word.clear();
                    out.push(c);
                } else {
                    word.push(c);
                }
            }
            out.push_str(&correct_word(detector, verbose, &word));
            out
        }
    }
}

/// 단어 하나를 판정해 교정한다. 감지기는 순수 문자 단어만 다루므로
/// 앞뒤 문장부호는 떼어냈다가 다시 붙인다 (단어 경계 처리의 임시 구현 —
/// 플랫폼 어댑터에서는 커밋 시점의 단어 버퍼가 이 역할을 한다).
fn correct_word(detector: &Detector, verbose: bool, word: &str) -> String {
    let stripped: &str = word.trim_matches(|c: char| c.is_ascii_punctuation());
    if stripped.is_empty() {
        return word.to_string();
    }
    let start = word.find(stripped).expect("stripped is a substring");
    let (prefix, rest) = word.split_at(start);
    let (core, suffix) = rest.split_at(stripped.len());

    let corrected = match detector.analyze(core) {
        Verdict::Keep => return word.to_string(),
        Verdict::ToHangul(s) | Verdict::ToEnglish(s) => s,
    };
    if verbose {
        eprintln!("  [{core} → {corrected}]");
    }
    format!("{prefix}{corrected}{suffix}")
}

//! Phase 0 정확도 하네스의 시드 (PLAN.md 9절).
//!
//! 지금은 수작업 라벨링한 소형 말뭉치로 (1) 변환 왕복 무결성과
//! (2) 감지기의 precision(오발동 0건)/recall을 게이트한다.
//! Phase 2에서 대형 말뭉치 기반 시뮬레이션으로 확장한다.

use haneng_core::{english_to_hangul, hangul_to_english, Detector, Sensitivity, Verdict};

/// 완성형 한글 단어는 영문 키로 분해했다가 다시 조합하면 원문과 같아야 한다.
#[test]
fn korean_roundtrip_integrity() {
    let words = [
        "한글",
        "안녕하세요",
        "감사합니다",
        "대한민국",
        "학교",
        "컴퓨터",
        "키보드",
        "사랑",
        "없다",
        "닭",
        "달걀",
        "괜찮아요",
        "값",
        "의사",
        "왜",
        "왔다",
        "빨간",
        "꽃",
        "많이",
        "읽었다",
        "앉아",
        "훑어",
        "아이",
        "이응",
    ];
    for word in words {
        let keys = hangul_to_english(word);
        assert_eq!(
            english_to_hangul(&keys),
            word,
            "roundtrip failed: {word} → {keys}"
        );
    }
}

/// 영문 단어도 한글 키로 갔다가 돌아오면 원문(소문자 기준)과 같아야 한다.
#[test]
fn english_roundtrip_integrity() {
    let words = ["hello", "some", "working", "thanks", "computer", "keyboard"];
    for word in words {
        let hangul = english_to_hangul(word);
        assert_eq!(
            hangul_to_english(&hangul),
            word,
            "roundtrip failed: {word} → {hangul}"
        );
    }
}

/// 영문 모드로 잘못 친 한국어 — Balanced에서 반드시 변환돼야 하는 시드.
const MUST_CONVERT_TO_HANGUL: &[(&str, &str)] = &[
    ("gksrmf", "한글"),
    ("dkssudgktpdy", "안녕하세요"),
    ("rkatkgkqslek", "감사합니다"),
    ("tkfkd", "사랑"),
    ("tnrh", "수고"),
    ("rhoscksgdk", "괜찮아"),
    ("djqtek", "없다"),
    ("zjavbxj", "컴퓨터"),
];

/// 한글 모드로 잘못 친 영어 — Balanced에서 반드시 변환돼야 하는 시드.
const MUST_CONVERT_TO_ENGLISH: &[(&str, &str)] = &[
    ("ㅗ디ㅣㅐ", "hello"),
    ("내ㅡㄷ", "some"),
    ("재가ㅑㅜㅎ", "working"),
    ("소무ㅏㄴ", "thanks"),
];

/// 절대 건드리면 안 되는 정상 입력 (precision 게이트: 오발동 0건).
const MUST_KEEP: &[&str] = &[
    // 정상 영어
    "hello",
    "world",
    "thanks",
    "keyboard",
    "question",
    "strengths",
    "sos",
    "sock",
    "go",
    "vodka",
    "the",
    "and",
    "with",
    // 정상 한국어
    "한글",
    "안녕하세요",
    "감사합니다",
    "대한민국",
    "책상",
    // 의도적 자모 입력
    "ㅋㅋㅋ",
    "ㅠㅠ",
    "ㄱㅅ",
    "ㅇㅋ",
    // 혼합·비문자
    "test한글",
    "abc123",
    "2026",
    "v1.0",
];

#[test]
fn recall_on_seed_corpus() {
    let d = Detector::new(Sensitivity::Balanced);
    for &(input, expected) in MUST_CONVERT_TO_HANGUL {
        assert_eq!(
            d.analyze(input),
            Verdict::ToHangul(expected.to_string()),
            "must convert: {input}"
        );
    }
    for &(input, expected) in MUST_CONVERT_TO_ENGLISH {
        assert_eq!(
            d.analyze(input),
            Verdict::ToEnglish(expected.to_string()),
            "must convert: {input}"
        );
    }
}

// ---- 임베드 사전 기반 말뭉치 게이트 (PLAN.md 목표: FP < 0.5%, recall > 90%) ----
//
// 사전을 시뮬레이션 말뭉치로 쓴다: 한국어 사전 단어를 영문 모드로 "잘못 친"
// 키열은 반드시 복원돼야 하고(recall), 영어 사전 단어는 절대 변환되면
// 안 된다(precision). 표본은 정렬된 사전에서 등간 추출.

use haneng_core::lexicon;

#[test]
fn corpus_recall_korean_typed_in_english_mode() {
    let d = Detector::new(Sensitivity::Balanced);
    let sample: Vec<&str> = lexicon::korean_words()
        .iter()
        .step_by(37)
        .copied()
        .collect();
    let mut hits = 0usize;
    let mut misses: Vec<&str> = Vec::new();
    for &word in &sample {
        let keys = hangul_to_english(word);
        match d.analyze(&keys) {
            Verdict::ToHangul(h) if h == word => hits += 1,
            _ => misses.push(word),
        }
    }
    let recall = hits as f64 / sample.len() as f64;
    assert!(
        recall >= 0.90,
        "recall {recall:.3} ({hits}/{}), 예: {:?}",
        sample.len(),
        &misses[..misses.len().min(15)]
    );
}

#[test]
fn corpus_recall_english_typed_in_korean_mode() {
    let d = Detector::new(Sensitivity::Balanced);
    let sample: Vec<&str> = lexicon::english_words()
        .iter()
        .step_by(29)
        .copied()
        .collect();
    let mut hits = 0usize;
    let mut misses: Vec<&str> = Vec::new();
    for &word in &sample {
        // 한글 모드에서 이 영어 단어를 쳤을 때 화면에 남는 문자열.
        let screen = english_to_hangul(word);
        match d.analyze(&screen) {
            Verdict::ToEnglish(e) if e == word => hits += 1,
            _ => misses.push(word),
        }
    }
    let recall = hits as f64 / sample.len() as f64;
    assert!(
        recall >= 0.90,
        "recall {recall:.3} ({hits}/{}), 예: {:?}",
        sample.len(),
        &misses[..misses.len().min(15)]
    );
}

// ---- 세벌식 (libhangul 자판 데이터 기반) ----

use haneng_core::{english_to_hangul_with, hangul_to_english_with, Layout};

#[test]
fn sebeolsik_known_key_mappings() {
    // libhangul XML에서 직접 확인한 항목: 390에서 h=ㄴ(초성), f=ㅏ, 2=ㅆ(종성).
    assert_eq!(english_to_hangul_with(Layout::Sebeolsik390, "hf"), "나");
    assert_eq!(english_to_hangul_with(Layout::Sebeolsik390, "hf2"), "났");
    // 종성 뒤 모음: 세벌식에는 도깨비불이 없다 — ㅆ은 앞 음절에 남는다.
    assert_eq!(english_to_hangul_with(Layout::Sebeolsik390, "hf2f"), "났ㅏ");
}

#[test]
fn sebeolsik_roundtrip_integrity() {
    let words = [
        "한글",
        "안녕하세요",
        "감사합니다",
        "컴퓨터",
        "없다",
        "닭",
        "달걀",
        "괜찮아요",
        "값",
        "의사",
        "왜",
        "왔다",
        "빨간",
        "꽃",
        "많이",
        "읽었다",
    ];
    for layout in [Layout::Sebeolsik390, Layout::SebeolsikFinal] {
        for word in words {
            let keys = hangul_to_english_with(layout, word);
            assert_eq!(
                english_to_hangul_with(layout, &keys),
                word,
                "{layout:?} roundtrip failed: {word} → {keys}"
            );
        }
    }
}

#[test]
fn sebeolsik_corpus_roundtrip() {
    // 임베드 한국어 사전 표본 전체가 두 세벌식 자판에서 왕복 무결해야 한다.
    for layout in [Layout::Sebeolsik390, Layout::SebeolsikFinal] {
        for &word in lexicon::korean_words().iter().step_by(53) {
            let keys = hangul_to_english_with(layout, word);
            assert_eq!(
                english_to_hangul_with(layout, &keys),
                word,
                "{layout:?} roundtrip failed: {word} → {keys}"
            );
        }
    }
}

#[test]
fn sebeolsik_detection_converts_wrong_mode_korean() {
    for layout in [Layout::Sebeolsik390, Layout::SebeolsikFinal] {
        let d = Detector::with_layout(Sensitivity::Balanced, layout);
        for word in ["한글", "안녕하세요", "감사합니다"] {
            let keys = hangul_to_english_with(layout, word);
            assert_eq!(
                d.analyze(&keys),
                Verdict::ToHangul(word.to_string()),
                "{layout:?}: {word} (keys: {keys})"
            );
        }
        // 진짜 영어는 세벌식 감지기에서도 유지.
        for word in ["hello", "world", "the"] {
            assert_eq!(d.analyze(word), Verdict::Keep, "{layout:?}: {word}");
        }
    }
}

/// 정확도 실측 리포트 (게이트 아님):
/// `cargo test -p haneng-core --test accuracy metrics_survey -- --ignored --nocapture`
#[test]
#[ignore = "관찰용 리포트"]
fn metrics_survey() {
    for sensitivity in [
        Sensitivity::Conservative,
        Sensitivity::Balanced,
        Sensitivity::Aggressive,
    ] {
        let d = Detector::new(sensitivity);
        let ko_sample: Vec<&str> = lexicon::korean_words().iter().step_by(7).copied().collect();
        let ko_hits = ko_sample
            .iter()
            .filter(
                |w| matches!(d.analyze(&hangul_to_english(w)), Verdict::ToHangul(h) if h == **w),
            )
            .count();
        let en_sample: Vec<&str> = lexicon::english_words()
            .iter()
            .step_by(7)
            .copied()
            .collect();
        let en_hits = en_sample
            .iter()
            .filter(
                |w| matches!(d.analyze(&english_to_hangul(w)), Verdict::ToEnglish(e) if e == **w),
            )
            .count();
        println!(
            "{sensitivity:?}: 한→영모드 recall {:.1}% ({ko_hits}/{}), 영→한모드 recall {:.1}% ({en_hits}/{})",
            100.0 * ko_hits as f64 / ko_sample.len() as f64,
            ko_sample.len(),
            100.0 * en_hits as f64 / en_sample.len() as f64,
            en_sample.len(),
        );
    }
}

#[test]
fn corpus_zero_false_positives() {
    // 어느 민감도에서도: 진짜 영어 단어(영문 모드)와 진짜 한국어 단어
    // (한글 모드)는 절대 변환되지 않는다.
    for sensitivity in [
        Sensitivity::Conservative,
        Sensitivity::Balanced,
        Sensitivity::Aggressive,
    ] {
        let d = Detector::new(sensitivity);
        for &word in lexicon::english_words().iter().step_by(13) {
            assert_eq!(
                d.analyze(word),
                Verdict::Keep,
                "영어 단어 오발동 ({sensitivity:?}): {word}"
            );
        }
        for &word in lexicon::korean_words().iter().step_by(13) {
            assert_eq!(
                d.analyze(word),
                Verdict::Keep,
                "한국어 단어 오발동 ({sensitivity:?}): {word}"
            );
        }
    }
}

#[test]
fn corpus_keeps_nondict_english_words() {
    // 사전 밖 영어(신조어·기술 용어·고유명사스러운 것)도 변환되면 안 된다.
    let d = Detector::new(Sensitivity::Balanced);
    for word in [
        "kubernetes",
        "grep",
        "vim",
        "rustacean",
        "tokio",
        "webhook",
        "middleware",
        "notarization",
        "bigram",
        "dubeolsik",
        "qwerty",
        "asdf",
    ] {
        assert_eq!(d.analyze(word), Verdict::Keep, "비사전 영어 오발동: {word}");
    }
}

#[test]
fn zero_false_positives_on_seed_corpus() {
    // 오발동은 제품 신뢰를 무너뜨린다 (PLAN.md 목표: FP < 0.5%).
    // 시드 말뭉치에서는 모든 민감도에서 0건이어야 한다.
    for sensitivity in [
        Sensitivity::Conservative,
        Sensitivity::Balanced,
        Sensitivity::Aggressive,
    ] {
        let d = Detector::new(sensitivity);
        for &word in MUST_KEEP {
            assert_eq!(
                d.analyze(word),
                Verdict::Keep,
                "false positive at {sensitivity:?}: {word}"
            );
        }
    }
}

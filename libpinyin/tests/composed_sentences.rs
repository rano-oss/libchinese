//! Long-sentence composition and ranking config knobs.
//!
//! Note: userdict.redb is exclusive — keep these in one test body.

use libchinese_core::{CandidateRankMode, ImeProfile};
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/converted/simplified"),
        PathBuf::from("/usr/share/libpinyin/data/simplified"),
    ];
    for p in candidates {
        if p.join("lexicon.fst").exists() {
            return p;
        }
    }
    panic!("no pinyin data dir found");
}

#[test]
fn composed_sentences_and_rank_options() {
    // Isolate from other test binaries that also open userdict.redb.
    let ud = std::env::temp_dir().join(format!(
        "libchinese-composed-userdict-{}-{}.redb",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::env::set_var("LIBCHINESE_USERDICT", &ud);

    let engine = libpinyin::Engine::from_data_dir(data_dir()).expect("engine");
    engine.set_english_enabled(false);
    engine.set_emoji_enabled(false);
    engine.set_allow_fuzzy(true);
    engine.set_emit_composed_sentences(true);

    // woshinuoweiren → 我是挪威人 (C pinyin_get_sentence parity)
    let tops: Vec<_> = engine
        .input("woshinuoweiren")
        .into_iter()
        .take(5)
        .map(|c| c.text)
        .collect();
    assert_eq!(
        tops.first().map(String::as_str),
        Some("我是挪威人"),
        "expected composed sentence at top-1, got {tops:?}"
    );

    for (input, expect) in [
        ("woshizhongguoren", "我是中国人"),
        ("woaini", "我爱你"),
        ("womenyiqiqubeijing", "我们一起去北京"),
    ] {
        let texts: Vec<_> = engine
            .input(input)
            .into_iter()
            .take(8)
            .map(|c| c.text)
            .collect();
        assert!(
            texts.iter().any(|t| t == expect),
            "{input}: missing {expect}, got {texts:?}"
        );
    }

    // Option off: suppress composed sentence
    engine.set_emit_composed_sentences(false);
    let tops: Vec<_> = engine
        .input("woshinuoweiren")
        .into_iter()
        .take(8)
        .map(|c| c.text)
        .collect();
    assert!(
        !tops.iter().any(|t| t == "我是挪威人"),
        "composed sentence should be suppressed when option is off, got {tops:?}"
    );
    engine.set_emit_composed_sentences(true);

    // Phrase-length rank prefers Chinese char length (C SORT_BY_PHRASE_LENGTH).
    engine.set_allow_fuzzy(false);
    engine.set_emit_composed_sentences(false);

    engine.set_candidate_rank_mode(CandidateRankMode::MatchedSpanThenScore);
    let span_tops: Vec<_> = engine
        .input("beijing")
        .into_iter()
        .take(8)
        .map(|c| c.text)
        .collect();

    engine.set_candidate_rank_mode(CandidateRankMode::PhraseLengthThenScore);
    let phrase_tops: Vec<_> = engine
        .input("beijing")
        .into_iter()
        .take(8)
        .map(|c| c.text)
        .collect();

    assert_eq!(span_tops.first().map(String::as_str), Some("北京"));
    assert_eq!(phrase_tops.first().map(String::as_str), Some("北京"));
    // Phrase-length mode should surface 1-char unigrams (C-style padding).
    assert!(
        phrase_tops.iter().any(|t| t.chars().count() == 1),
        "PhraseLengthThenScore missing unigrams in top-8: {phrase_tops:?}"
    );
    assert_eq!(
        engine.candidate_rank_mode(),
        CandidateRankMode::PhraseLengthThenScore
    );

    // Profiles: BestUx keeps sentences in the bar; Compat keeps them on best_sentence().
    engine.set_profile(ImeProfile::BestUx);
    assert_eq!(engine.profile(), ImeProfile::BestUx);
    assert!(engine.emit_composed_sentences());
    assert_eq!(
        engine.candidate_rank_mode(),
        CandidateRankMode::MatchedSpanThenScore
    );
    let best_ux_top = engine
        .input("woshinuoweiren")
        .into_iter()
        .next()
        .map(|c| c.text);
    assert_eq!(best_ux_top.as_deref(), Some("我是挪威人"));

    engine.set_profile(ImeProfile::LibpinyinCompat);
    assert!(!engine.emit_composed_sentences());
    assert_eq!(
        engine.candidate_rank_mode(),
        CandidateRankMode::PhraseLengthThenScore
    );
    assert!(!engine.english_enabled());
    assert!(!engine.emoji_enabled());
    let compat_tops: Vec<_> = engine
        .input("woshinuoweiren")
        .into_iter()
        .take(5)
        .map(|c| c.text)
        .collect();
    assert!(
        !compat_tops.iter().any(|t| t == "我是挪威人"),
        "compat candidate bar should not fold sentence; got {compat_tops:?}"
    );
    assert_eq!(
        engine.best_sentence("woshinuoweiren").as_deref(),
        Some("我是挪威人"),
        "compat profile still exposes sentence via best_sentence()"
    );

    // PinyinConfig helpers stay in sync with engine profiles.
    let ux = libpinyin::PinyinConfig::best_ux();
    assert!(ux.emit_composed_sentences);
    let compat = libpinyin::PinyinConfig::libpinyin_compat();
    assert!(!compat.emit_composed_sentences);
}

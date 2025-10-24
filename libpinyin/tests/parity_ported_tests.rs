// Parity tests (Phase 2 -> Option 2)
//
// Purpose: scaffold and begin porting representative upstream test vectors
// for segmentation, n-gram scoring, and lookup parity. Replace the placeholder
// cases with exact upstream vectors as they are made available.
//
// These are integration-style tests placed in the crate `libpinyin`'s `tests/`
// directory. They exercise public APIs from `libpinyin` and `libchinese-core`.
// Adjust imports if your Cargo.toml uses different package names.

use libchinese_core::{Config, Interpolator, Lexicon, Model, NGramModel, UserDict};
use libpinyin::engine::Engine;
use libpinyin::parser::Parser;

#[test]
fn parity_segmentation_basic_examples() {
    // Representative segmentation cases. These are sanity checks that mirror
    // expected upstream behavior for common inputs. Replace with upstream
    // test vectors when available.
    let mut p = Parser::new();
    p.insert_syllable("ni");
    p.insert_syllable("hao");
    p.insert_syllable("zhong");
    p.insert_syllable("guo");

    let s1 = p.segment_best("nihao", false);
    let texts: Vec<String> = s1.into_iter().map(|s| s.text).collect();
    assert_eq!(texts, vec!["ni".to_string(), "hao".to_string()]);

    let s2 = p.segment_best("zhongguo", false);
    let texts2: Vec<String> = s2.into_iter().map(|s| s.text).collect();
    assert_eq!(texts2, vec!["zhong".to_string(), "guo".to_string()]);
}

#[test]
fn parity_ngram_scoring_example() {
    // Small n-gram scoring parity example. When we port upstream tests, we will
    // compare the computed ln-prob sums against the canonical C++ values.
    let mut m = NGramModel::new();
    m.insert_unigram("你", -1.0_f64);
    m.insert_unigram("好", -1.2_f64);
    m.insert_bigram("你", "好", -0.2);

    let tokens = vec!["你".to_string(), "好".to_string()];
    // Use interpolation weights that favor bigram like upstream examples.
    // The core API expects a Config reference for interpolation weights.
    let cfg = libchinese_core::Config {
        fuzzy: vec![],
        unigram_weight: 0.3,
        bigram_weight: 0.6,
        trigram_weight: 0.1,
        sort_by_phrase_length: false,
        sort_without_longer_candidate: false,
        max_cache_size: 1000,
        auto_suggestion: false,
        max_prediction_length: 3,
        min_prediction_frequency: 0.0,
        prefer_phrase_predictions: false,
        min_suggestion_trigger_length: 2,
        full_width_enabled: false,
        select_keys: "123456789".to_string(),
        masked_phrases: Default::default(),
        correction_penalty: 200,
        fuzzy_penalty_multiplier: 100,
        incomplete_penalty: 500,
        unknown_penalty: 1000,
        unknown_cost: 10.0,
    };
    let score = m.score_sequence(&tokens, &cfg);

    // expected computed score from equivalent arithmetic
    let expected = -1.5_f32;
    assert!((score - expected).abs() < 1e-6);
}

#[test]
fn parity_engine_lookup_flow() {
    // Small end-to-end flow: parser -> lexicon -> ngram -> userdict -> engine
    let mut lex = Lexicon::new();
    // core::Lexicon::insert uses apostrophe-separated keys after segmentation fix
    lex.insert("ni'hao", "你好");
    lex.insert("ni'hao", "你号");

    let mut ng = NGramModel::new();
    ng.insert_unigram("你", -1.0_f64);
    ng.insert_unigram("好", -1.0_f64);
    ng.set_interpolator(Interpolator::empty_for_test());

    // Construct a default UserDict using the current core API.
    let temp_path = std::env::temp_dir().join(format!(
        "test_userdict_parity_{}.redb", std::process::id()
    ));
    let user = UserDict::new(&temp_path).expect("create test userdict");
    let cfg = libpinyin::PinyinConfig::default().into_base();
    let model = Model::new(lex, ng, user, cfg);

    // Parser is now created internally with PINYIN_SYLLABLES
    let engine = Engine::new(model);
    let cands = engine.input("nihao");
    assert!(!cands.is_empty());
    // Top candidate should be the highest-frequency phrase in the lexicon
    assert_eq!(cands[0].text, "你好");
}

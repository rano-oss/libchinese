// Parity tests (Phase 2 -> Option 2)
//
// Purpose: scaffold and begin porting representative upstream test vectors
// for segmentation, n-gram scoring, and lookup parity. Replace the placeholder
// cases with exact upstream vectors as they are made available.
//
// These are integration-style tests placed in the crate `libpinyin`'s `tests/`
// directory. They exercise public APIs from `libpinyin` and `libchinese-core`.
// Adjust imports if your Cargo.toml uses different package names.

use libchinese_core::{Config, Lexicon, Model, NGramModel, UserDict};
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
    m.insert_unigram("你", -1.0);
    m.insert_unigram("好", -1.2);
    m.insert_bigram("你", "好", -0.2);

    let tokens = vec!["你".to_string(), "好".to_string()];
    // Use interpolation weights that favor bigram like upstream examples.
    // The core API expects a Config reference for interpolation weights.
    let cfg = Config {
        fuzzy: Vec::new(),
        unigram_weight: 0.3,
        bigram_weight: 0.6,
        trigram_weight: 0.1,
    };
    let score = m.score_sequence(&tokens, &cfg);

    // expected computed score from equivalent arithmetic
    let expected = -1.5_f32;
    assert!((score - expected).abs() < 1e-4);
}

#[test]
fn parity_engine_lookup_flow() {
    // Small end-to-end flow: parser -> lexicon -> ngram -> userdict -> engine
    let mut lex = Lexicon::new();
    // core::Lexicon::insert currently accepts (key, phrase) only.
    lex.insert("nihao", "你好");
    lex.insert("nihao", "你号");

    let mut ng = NGramModel::new();
    ng.insert_unigram("你", -1.0);
    ng.insert_unigram("好", -1.0);

    // Construct a default UserDict using the current core API.
    let user = UserDict::new();
    let cfg = Config::default();
    let model = Model::new(lex, ng, user, cfg, None);

    let parser = Parser::with_syllables(&["ni", "hao"]);
    let engine = Engine::new(model, parser);
    let cands = engine.input("nihao");
    assert!(!cands.is_empty());
    // Top candidate should be the highest-frequency phrase in the lexicon
    assert_eq!(cands[0].text, "你好");
}

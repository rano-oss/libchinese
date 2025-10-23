use libchinese_core::{Config, Interpolator, Lexicon, Model, NGramModel, UserDict};
use libpinyin::engine::Engine;
use libpinyin::parser::Parser;

/// Ported lookup parity tests (userdict commit ranking)
///
/// These tests exercise the end-to-end flow:
///  - parser segmentation -> lexicon lookup -> n-gram scoring -> userdict boost
///  - committing a phrase via `Engine::commit` should increase its ranking
///    for subsequent queries.
///
/// The tests are intentionally deterministic: we craft unigram log-probabilities
/// so one phrase starts ranked above the other, then we repeatedly commit the
/// lower-ranked phrase until it overtakes the previously higher-ranked item.
#[test]
fn userdict_commit_changes_ranking_end_to_end() {
    // Build lexicon with two competing phrases for the key "ni'hao".
    let mut lex = Lexicon::new();
    // Use apostrophe-separated keys after segmentation fix
    lex.insert("ni'hao", "你好");
    lex.insert("ni'hao", "你号");

    // Build an n-gram model that strongly favors "你号" initially.
    // Tokens are characters; we assign ln-probabilities (higher = less negative).
    let mut ng = NGramModel::new();
    // Shared first character
    ng.insert_unigram("你", -1.0_f64);
    // Make "号" likely (better score) and "好" unlikely so "你号" ranks higher initially.
    ng.insert_unigram("号", -1.0_f64); // favorable
    ng.insert_unigram("好", -3.0_f64); // unfavorable
    ng.set_interpolator(Interpolator::empty_for_test());

    // Empty user dictionary initially.
    let temp_path = std::env::temp_dir().join(format!(
        "test_userdict_lookup_{}.redb", std::process::id()
    ));
    let user = UserDict::new(&temp_path).expect("create test userdict");

    // Config with default interpolation weights (not critical for this test).
    let cfg = libpinyin::PinyinConfig::default().into_base();
    let model = Model::new(lex, ng, user, cfg);

    // Engine doesn't need to be mutable - commit() takes &self
    // Parser is now created internally with PINYIN_SYLLABLES
    let engine = Engine::new(model);

    // Initial query: expect "你号" to be top due to ngram advantages.
    let cands_before = engine.input("nihao");
    assert!(!cands_before.is_empty(), "expected at least one candidate");
    assert_eq!(
        cands_before[0].text, "你号",
        "initial top candidate should be 你号"
    );

    // Commit the target phrase "你好" multiple times to boost it in userdict.
    // Each commit increments learned count by 1; boost applied is ln(1 + freq).
    for _ in 0..10 {
        engine.commit("你好");
    }

    // After commits, "你好" should overtake "你号".
    let cands_after = engine.input("nihao");
    assert!(
        !cands_after.is_empty(),
        "expected at least one candidate after commits"
    );
    assert_eq!(
        cands_after[0].text, "你好",
        "after commits, top candidate should be 你好"
    );
}

#[test]
fn model_candidates_for_key_respects_userdict_boost() {
    // This test verifies the boosting behavior at the Model level
    // without exercising Engine::commit (we manipulate UserDict directly).

    // Setup lexicon
    let mut lex = Lexicon::new();
    lex.insert("nihao", "你好");
    lex.insert("nihao", "你号");

    // N-gram probabilities configured to favor "你号" initially
    let mut ng = NGramModel::new();
    ng.insert_unigram("你", -1.0_f64);
    ng.insert_unigram("号", -0.5_f64);
    ng.insert_unigram("好", -3.0_f64);
    ng.set_interpolator(Interpolator::empty_for_test());

    // Pre-populate userdict and boost "你好" several times
    let temp_path = std::env::temp_dir().join(format!(
        "test_userdict_ranking_{}.redb", std::process::id()
    ));
    let user = UserDict::new(&temp_path).expect("create test userdict");
    // Manually learn the phrase multiple times to simulate prior selections
    // Increase iterations to ensure the userdict boost surpasses the n-gram gap.
    for _ in 0..20 {
        user.learn("你好");
    }

    let cfg = libpinyin::PinyinConfig::default().into_base();
    let model = Model::new(lex, ng, user, cfg);

    // Directly ask the model for candidates for the key and verify ordering.
    let candidates = model.candidates_for_key("nihao", 10);
    assert!(!candidates.is_empty());
    // Because we boosted "你好" via userdict, it should be the top candidate now.
    assert_eq!(candidates[0].text, "你好");
}

#[test]
fn apostrophe_in_input_is_treated_as_separator() {
    // Parser should treat apostrophe as a separator when apostrophe is part of the trie
    // (mirror upstream behavior where apostrophes separate ambiguous syllable boundaries).
    let mut p = Parser::new();
    // Insert apostrophe as an explicit token so parser recognizes it and will skip it in output.
    p.insert_syllable("'");
    p.insert_syllable("a");
    p.insert_syllable("b");

    let seg = p.segment_best("a'b", false);
    let texts: Vec<String> = seg.into_iter().map(|s| s.text).collect();
    assert_eq!(texts, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn segment_top_k_returns_alternatives_with_fuzzy() {
    // Build a parser that allows either 'zhong' as a single syllable or the
    // fuzzy split 'z' + 'ong' when fuzzy is enabled.
    let mut p = Parser::new();
    p.insert_syllable("zhong");
    p.insert_syllable("z");
    p.insert_syllable("ong");

    // Without fuzzy, top-k should yield single-best segmentation (['zhong']).
    let res_no_fuzzy = p.segment_top_k("zhong", 3, false);
    assert!(!res_no_fuzzy.is_empty());
    assert_eq!(res_no_fuzzy[0], p.segment_best("zhong", false));

    // With fuzzy allowed, we expect at least two alternatives (['zhong'] and ['z','ong']).
    let res_fuzzy = p.segment_top_k("zhong", 3, true);
    // Expect at least two hypotheses
    assert!(
        res_fuzzy.len() >= 2,
        "expected at least two segmentations when fuzzy is enabled"
    );

    // Ensure one alternative uses a fuzzy token (has Syllable.fuzzy == true)
    let has_fuzzy = res_fuzzy.iter().any(|seq| seq.iter().any(|s| s.fuzzy));
    assert!(
        has_fuzzy,
        "expected at least one fuzzy alternative in top-k results"
    );
}

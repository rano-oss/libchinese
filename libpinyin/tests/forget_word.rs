//! Forget-word (Ctrl+7): remove learned freq + mask from candidates.

use libchinese_core::{ImeEngine, KeyEvent, KeyResult};
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

fn isolate_userdict() {
    let ud = std::env::temp_dir().join(format!(
        "libchinese-forget-ud-{}-{}.redb",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_file(&ud);
    unsafe {
        std::env::set_var("LIBCHINESE_USERDICT", &ud);
    }
}

fn type_ascii(ime: &mut ImeEngine<libpinyin::parser::Parser>, s: &str) {
    for ch in s.chars() {
        let _ = ime.process_key(KeyEvent::Char(ch));
    }
}

#[test]
fn forget_word_hides_candidate() {
    isolate_userdict();
    let engine = libpinyin::Engine::from_data_dir(data_dir()).expect("load");
    engine.set_english_enabled(false);
    engine.set_emoji_enabled(false);
    {
        let mut cfg = engine.config_mut();
        cfg.auto_suggestion = false;
        cfg.learning_enabled = true;
    }

    // Boost a secondary candidate so we can forget something visible.
    engine.commit("背景");

    let mut ime = ImeEngine::from_arc_with_page_size(engine.inner_arc(), 9);
    type_ascii(&mut ime, "beijing");
    let before = ime.context().candidates.clone();
    assert!(
        before.iter().any(|t| t == "背景"),
        "expected 背景 among candidates after learn, got {before:?}"
    );

    // Move cursor to 背景 if not selected.
    while ime.context().candidates.get(ime.context().candidate_cursor) != Some(&"背景".to_string())
    {
        assert_eq!(ime.process_key(KeyEvent::Down), KeyResult::Handled);
    }

    assert_eq!(ime.process_key(KeyEvent::ForgetWord), KeyResult::Handled);
    let after = ime.context().candidates.clone();
    assert!(
        !after.iter().any(|t| t == "背景"),
        "背景 should be forgotten/masked, got {after:?}"
    );
    assert!(engine.userdict().is_masked("背景"));
    assert_eq!(engine.userdict().frequency("背景"), 0);

    // Fresh query still hides it.
    let mut ime = ImeEngine::from_arc_with_page_size(engine.inner_arc(), 9);
    type_ascii(&mut ime, "beijing");
    assert!(
        !ime.context().candidates.iter().any(|t| t == "背景"),
        "mask should persist across sessions, got {:?}",
        ime.context().candidates
    );
}

//! Mac/Windows home-feel: Chinese punct, v/u helpers, tone Tab, pin phrase.

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
        "libchinese-home-feel-{}-{}.redb",
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
fn chinese_punct_and_helpers_and_tone_and_pin() {
    isolate_userdict();
    let engine = libpinyin::Engine::from_data_dir(data_dir()).expect("load");
    engine.set_english_enabled(false);
    engine.set_emoji_enabled(false);
    {
        let mut cfg = engine.config_mut();
        cfg.auto_suggestion = false;
        cfg.chinese_punctuation = true;
        cfg.v_mode_enabled = true;
        cfg.u_mode_enabled = true;
        cfg.inline_prediction = false;
        cfg.learning_enabled = true;
    }

    // Idle Chinese punctuation
    let mut ime = ImeEngine::from_arc_with_page_size(engine.inner_arc(), 9);
    assert_eq!(ime.process_key(KeyEvent::Char('.')), KeyResult::Handled);
    assert_eq!(ime.context().commit_text, "。");

    // v-mode numerals
    let mut ime = ImeEngine::from_arc_with_page_size(engine.inner_arc(), 9);
    type_ascii(&mut ime, "v123");
    assert!(
        ime.context().candidates.iter().any(|t| t == "一二三"),
        "v123 helpers: {:?}",
        ime.context().candidates
    );

    // u-mode symbol
    let mut ime = ImeEngine::from_arc_with_page_size(engine.inner_arc(), 9);
    type_ascii(&mut ime, "uheart");
    assert!(
        ime.context().candidates.iter().any(|t| t == "♥"),
        "uheart: {:?}",
        ime.context().candidates
    );

    // Tone filter: ma → 妈/麻/马/骂; Tab should narrow
    let mut ime = ImeEngine::from_arc_with_page_size(engine.inner_arc(), 9);
    type_ascii(&mut ime, "ma");
    let before = ime.context().candidates.clone();
    assert!(!before.is_empty());
    assert_eq!(ime.process_key(KeyEvent::Tab), KeyResult::Handled); // 1声
    let t1 = ime.context().candidates.clone();
    assert!(!t1.is_empty());
    // After a few Tabs we return to all
    let _ = ime.process_key(KeyEvent::Tab);
    let _ = ime.process_key(KeyEvent::Tab);
    let _ = ime.process_key(KeyEvent::Tab);
    let _ = ime.process_key(KeyEvent::Tab); // back to all
    assert_eq!(
        ime.context().candidates.len(),
        before.len(),
        "tone filter should restore full list"
    );

    // Pin phrase
    let mut ime = ImeEngine::from_arc_with_page_size(engine.inner_arc(), 9);
    type_ascii(&mut ime, "beijing");
    let top = ime.context().candidates.first().cloned().expect("cands");
    assert_eq!(ime.process_key(KeyEvent::PinPhrase), KeyResult::Handled);
    assert_eq!(engine.userdict().frequency(&top), 500);

    // Commit default + Chinese punct
    let mut ime = ImeEngine::from_arc_with_page_size(engine.inner_arc(), 9);
    type_ascii(&mut ime, "nihao");
    assert_eq!(ime.process_key(KeyEvent::Char('.')), KeyResult::Handled);
    assert!(
        ime.context().commit_text.ends_with('。'),
        "got {:?}",
        ime.context().commit_text
    );
    assert!(!ime.context().commit_text.is_empty());

    // Inline prediction stays enabled without crashing across commit → compose.
    {
        let mut cfg = engine.config_mut();
        cfg.inline_prediction = true;
        cfg.auto_suggestion = false;
    }
    let mut ime = ImeEngine::from_arc_with_page_size(engine.inner_arc(), 9);
    type_ascii(&mut ime, "nihao");
    assert_eq!(ime.process_key(KeyEvent::Space), KeyResult::Handled);
    assert!(!ime.context().commit_text.is_empty());
    type_ascii(&mut ime, "ma");
    assert!(
        !ime.context().candidates.is_empty(),
        "inline_prediction path should still yield candidates"
    );
}

//! 以词定字: `[` / `]` pick a character from the highlighted phrase.
//!
//! One test body so we only open the shared userdict.redb once (redb is exclusive).

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
        "libchinese-choose-char-ud-{}-{}.redb",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_file(&ud);
    // SAFETY: test-only, before engine open.
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
fn choose_char_from_phrase_behaviour() {
    isolate_userdict();
    let engine = libpinyin::Engine::from_data_dir(data_dir()).expect("load");
    engine.set_english_enabled(false);
    engine.set_emoji_enabled(false);
    {
        let mut cfg = engine.config_mut();
        cfg.choose_char_from_phrase = true;
        cfg.auto_suggestion = false;
    }

    let mut ime = ImeEngine::from_arc_with_page_size(engine.inner_arc(), 9);
    type_ascii(&mut ime, "zhongguo");
    assert_eq!(
        ime.context().candidates.first().map(|s| s.as_str()),
        Some("中国"),
        "expected 中国 top-1, got {:?}",
        ime.context().candidates
    );

    assert_eq!(
        ime.process_key(KeyEvent::ChooseCharFromPhrase(0)),
        KeyResult::Handled
    );
    assert_eq!(ime.context().commit_text, "中");
    assert!(ime.context().preedit_text.is_empty());
    assert!(ime.context().candidates.is_empty());

    let mut ime = ImeEngine::from_arc_with_page_size(engine.inner_arc(), 9);
    type_ascii(&mut ime, "zhongguo");
    assert_eq!(
        ime.process_key(KeyEvent::ChooseCharFromPhrase(-1)),
        KeyResult::Handled
    );
    assert_eq!(ime.context().commit_text, "国");

    {
        let mut cfg = engine.config_mut();
        cfg.choose_char_from_phrase = false;
    }
    let mut ime = ImeEngine::from_arc_with_page_size(engine.inner_arc(), 9);
    type_ascii(&mut ime, "zhongguo");
    assert_eq!(
        ime.process_key(KeyEvent::ChooseCharFromPhrase(0)),
        KeyResult::NotHandled
    );
    assert!(ime.context().commit_text.is_empty());
    assert!(!ime.context().candidates.is_empty());
}

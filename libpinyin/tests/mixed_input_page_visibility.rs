//! Mixed-input extras must appear on the first candidate page (page size 9).
//!
//! One test body so we only open the shared userdict.redb once (redb is exclusive).

use libchinese_core::{ImeEngine, KeyEvent};
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

fn type_ascii(ime: &mut ImeEngine<libpinyin::parser::Parser>, s: &str) {
    for ch in s.chars() {
        let _ = ime.process_key(KeyEvent::Char(ch));
    }
}

#[test]
fn english_and_emoji_on_first_page_via_ime_engine() {
    let engine = libpinyin::Engine::from_data_dir(data_dir()).expect("load");
    engine.set_english_enabled(true);
    engine.set_emoji_enabled(true);
    let page_size = 9;

    let mut ime = ImeEngine::from_arc_with_page_size(engine.inner_arc(), page_size);
    type_ascii(&mut ime, "hello");
    let page = &ime.context().candidates;
    assert!(
        page.iter().any(|t| t.eq_ignore_ascii_case("hello")),
        "expected 'hello' on first page, got {:?}",
        page
    );

    // Fresh IME session for emoji keyword (same engine / userdict handle).
    let mut ime = ImeEngine::from_arc_with_page_size(engine.inner_arc(), page_size);
    type_ascii(&mut ime, "smile");
    let page = &ime.context().candidates;
    let want = libchinese_core::EmojiDict::from_file(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/emoji.table"),
    )
    .expect("emoji.table")
    .lookup_best("smile")
    .expect("smile keyword")
    .emoji
    .clone();
    assert!(
        page.iter().any(|t| t == &want),
        "expected emoji {:?} on first page, got {:?}",
        want,
        page
    );
}

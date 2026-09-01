//! Integration tests for libzhuyin IME functionality.

use libchinese_core::KeyEvent;
use libzhuyin::{create_ime_engine_eten, create_ime_engine_hsu, create_ime_engine_standard};
use std::sync::atomic::{AtomicU32, Ordering};

const DATA_DIR: &str = "../data/converted/zhuyin_traditional";

/// Generate a unique temp userdict path per call to avoid redb lock conflicts.
static COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_userdict_dir() -> std::path::PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::path::PathBuf::from(format!("/tmp/libzhuyin-test-{}-{}", std::process::id(), id));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Helper: create a standard engine with an isolated userdict.
fn make_standard(page_size: usize) -> libchinese_core::ImeEngine<libzhuyin::ZhuyinParser> {
    let ud = temp_userdict_dir().join("userdict.redb");
    libzhuyin::create_ime_engine_standard_with_userdict(DATA_DIR, page_size, &ud)
        .expect("Failed to create Standard IME engine")
}

/// Helper: create an HSU engine with an isolated userdict.
fn make_hsu(page_size: usize) -> libchinese_core::ImeEngine<libzhuyin::ZhuyinParser> {
    let ud = temp_userdict_dir().join("userdict.redb");
    libzhuyin::create_ime_engine_hsu_with_userdict(DATA_DIR, page_size, &ud)
        .expect("Failed to create HSU IME engine")
}

/// Helper: create an ETEN engine with an isolated userdict.
fn make_eten(page_size: usize) -> libchinese_core::ImeEngine<libzhuyin::ZhuyinParser> {
    let ud = temp_userdict_dir().join("userdict.redb");
    libzhuyin::create_ime_engine_eten_with_userdict(DATA_DIR, page_size, &ud)
        .expect("Failed to create ETEN IME engine")
}

#[test]
fn test_data_directory_exists() {
    use std::path::Path;
    let path = Path::new(DATA_DIR);
    assert!(path.exists(), "Data directory {:?} does not exist", path);
    assert!(path.join("lexicon.fst").exists());
    assert!(path.join("lexicon.dat").exists());
}

#[test]
fn test_hsu_factory_creates_ime() {
    let _ime = make_hsu(5);
}

#[test]
fn test_standard_factory_creates_ime() {
    let _ime = make_standard(5);
}

#[test]
fn test_eten_factory_creates_ime() {
    let _ime = make_eten(5);
}

#[test]
fn test_ime_basic_input_flow() {
    let mut ime = make_standard(5);

    // Type ㄋㄧˇ (ni3 = 你)
    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));
    ime.process_key(KeyEvent::Char('ˇ'));

    let context = ime.context();
    assert!(!context.preedit_text.is_empty(), "Should have preedit");
    assert!(
        !context.candidates.is_empty(),
        "Should have candidates for ㄋㄧˇ"
    );
}

#[test]
fn test_ime_candidate_selection() {
    let mut ime = make_standard(5);

    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));
    ime.process_key(KeyEvent::Char('ˇ'));

    assert!(!ime.context().candidates.is_empty());

    // Select first candidate with space
    ime.process_key(KeyEvent::Space);

    assert!(
        !ime.context().commit_text.is_empty(),
        "Should have committed text"
    );
}

#[test]
fn test_ime_number_selection() {
    let mut ime = make_standard(5);

    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));
    ime.process_key(KeyEvent::Char('ˇ'));

    if ime.context().candidates.len() >= 2 {
        ime.process_key(KeyEvent::Number(2));
        assert!(!ime.context().commit_text.is_empty());
    }
}

#[test]
fn test_ime_backspace() {
    let mut ime = make_standard(5);

    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));
    ime.process_key(KeyEvent::Char('ˇ'));

    let len_before = ime.context().preedit_text.len();

    ime.process_key(KeyEvent::Backspace);

    assert!(
        ime.context().preedit_text.len() < len_before,
        "Backspace should shorten preedit"
    );
}

#[test]
fn test_ime_escape_clears() {
    let mut ime = make_standard(5);

    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));
    ime.process_key(KeyEvent::Char('ˇ'));
    ime.process_key(KeyEvent::Escape);

    let ctx = ime.context();
    assert!(ctx.preedit_text.is_empty(), "Escape should clear preedit");
    assert!(ctx.candidates.is_empty(), "Escape should clear candidates");
}

#[test]
fn test_ime_page_navigation() {
    let mut ime = make_standard(3);

    ime.process_key(KeyEvent::Char('ㄧ'));

    ime.process_key(KeyEvent::PageDown);

    let ctx = ime.context();
    if ctx.auxiliary_text.contains('/') {
        assert!(
            !ctx.candidates.is_empty(),
            "Should have candidates on next page"
        );
    }
}

#[test]
fn test_ime_cursor_navigation() {
    let mut ime = make_standard(5);

    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));

    let initial_cursor = ime.context().candidate_cursor;

    ime.process_key(KeyEvent::Down);

    if ime.context().candidates.len() > 1 {
        assert!(
            ime.context().candidate_cursor > initial_cursor,
            "Cursor should move down"
        );

        ime.process_key(KeyEvent::Up);
        assert_eq!(
            ime.context().candidate_cursor,
            initial_cursor,
            "Cursor should return"
        );
    }
}

#[test]
fn test_hsu_fuzzy_matching() {
    let mut ime = make_hsu(5);

    ime.process_key(KeyEvent::Char('ㄓ'));

    assert!(
        !ime.context().candidates.is_empty(),
        "HSU should produce fuzzy candidates"
    );
}

#[test]
fn test_standard_nasal_fuzzy_matching() {
    let mut ime = make_standard(5);

    ime.process_key(KeyEvent::Char('ㄊ'));
    ime.process_key(KeyEvent::Char('ㄢ'));

    assert!(
        !ime.context().candidates.is_empty(),
        "Standard should produce nasal fuzzy candidates"
    );
}

#[test]
fn test_all_layouts_produce_candidates() {
    for make_fn in [make_hsu, make_standard, make_eten] {
        let mut ime = make_fn(5);

        ime.process_key(KeyEvent::Char('ㄋ'));
        ime.process_key(KeyEvent::Char('ㄧ'));
        ime.process_key(KeyEvent::Char('ˇ'));

        assert!(
            !ime.context().candidates.is_empty(),
            "Layout should produce candidates"
        );
    }
}

#[test]
fn test_ime_reset_state() {
    let mut ime = make_standard(5);

    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));
    ime.process_key(KeyEvent::Space);

    assert!(!ime.context().commit_text.is_empty());
    assert!(
        ime.context().preedit_text.is_empty(),
        "Preedit should clear after commit"
    );
}

#[test]
fn test_ime_multiple_commits() {
    let mut ime = make_standard(5);

    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));
    ime.process_key(KeyEvent::Space);
    assert!(!ime.context().commit_text.is_empty());

    ime.process_key(KeyEvent::Char('ㄏ'));
    ime.process_key(KeyEvent::Char('ㄠ'));
    ime.process_key(KeyEvent::Space);
    assert!(!ime.context().commit_text.is_empty());
}

#[test]
fn test_ime_enter_commits() {
    let mut ime = make_standard(5);

    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));
    ime.process_key(KeyEvent::Enter);

    assert!(
        ime.context().preedit_text.is_empty(),
        "Enter should clear preedit"
    );
}

#[test]
fn test_different_page_sizes() {
    for page_size in [3, 5, 10] {
        let mut ime = make_standard(page_size);

        ime.process_key(KeyEvent::Char('ㄋ'));

        assert!(
            ime.context().candidates.len() <= page_size,
            "Candidates should not exceed page size {}",
            page_size
        );
    }
}

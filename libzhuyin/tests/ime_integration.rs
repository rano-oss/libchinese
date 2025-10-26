//! Integration tests for libzhuyin IME functionality.
//!
//! Tests the complete IME workflow including:
//! - Factory functions for different keyboard layouts (HSU, Standard, ETEN)
//! - Input processing and candidate generation
//! - Fuzzy matching behavior across layouts

use libchinese_core::KeyEvent;
use libzhuyin::{create_ime_engine_eten, create_ime_engine_hsu, create_ime_engine_standard};

// Note: Integration tests run from crate directory (libzhuyin), not workspace root
const DATA_DIR: &str = "../data/converted/zhuyin_traditional";

#[test]
fn test_data_directory_exists() {
    use std::path::Path;
    let path = Path::new(DATA_DIR);
    assert!(
        path.exists(),
        "Data directory {:?} does not exist. CWD: {:?}",
        path,
        std::env::current_dir().unwrap()
    );

    let lexicon_fst = path.join("lexicon.fst");
    assert!(
        lexicon_fst.exists(),
        "lexicon.fst does not exist at {:?}",
        lexicon_fst
    );
}

#[test]
fn test_hsu_factory_creates_ime() {
    let result = create_ime_engine_hsu(DATA_DIR, 5);
    assert!(
        result.is_ok(),
        "Failed to create HSU IME engine: {:?}",
        result.err()
    );
}

#[test]
fn test_standard_factory_creates_ime() {
    let result = create_ime_engine_standard(DATA_DIR, 5);
    assert!(
        result.is_ok(),
        "Failed to create Standard IME engine: {:?}",
        result.err()
    );
}

#[test]
fn test_eten_factory_creates_ime() {
    let result = create_ime_engine_eten(DATA_DIR, 5);
    assert!(
        result.is_ok(),
        "Failed to create ETEN IME engine: {:?}",
        result.err()
    );
}

#[test]
fn test_ime_basic_input_flow() {
    let mut ime = create_ime_engine_standard(DATA_DIR, 5).expect("Failed to create IME engine");

    // Type proper bopomofo with tone marks: ㄋㄧˇ (ni, 你) + ㄏㄠˇ (hao, 好)
    eprintln!("DEBUG: Typing ㄋ");
    ime.process_key(KeyEvent::Char('ㄋ'));
    let ctx1 = ime.context();
    eprintln!(
        "DEBUG: After ㄋ: preedit='{}', candidates={}",
        ctx1.preedit_text,
        ctx1.candidates.len()
    );

    eprintln!("DEBUG: Typing ㄧ");
    ime.process_key(KeyEvent::Char('ㄧ'));
    let ctx2 = ime.context();
    eprintln!(
        "DEBUG: After ㄧ: preedit='{}', candidates={}",
        ctx2.preedit_text,
        ctx2.candidates.len()
    );

    eprintln!("DEBUG: Typing ˇ (tone 3)");
    ime.process_key(KeyEvent::Char('ˇ')); // tone 3

    // Should have input buffer and candidates
    let context = ime.context();
    eprintln!(
        "DEBUG: Final: preedit='{}', candidates={}",
        context.preedit_text,
        context.candidates.len()
    );
    if !context.candidates.is_empty() {
        eprintln!("DEBUG: first candidate='{}'", context.candidates[0]);
    }
    assert!(
        !context.preedit_text.is_empty(),
        "Preedit should contain input after 你"
    );
    assert!(
        context.candidates.len() > 0,
        "Should have candidates for ㄋㄧˇ (你)"
    );
}

#[test]
fn test_ime_candidate_selection() {
    let mut ime = create_ime_engine_standard(DATA_DIR, 5).expect("Failed to create IME engine");

    // Type ㄋㄧˇ (ni third tone - 你)
    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));
    ime.process_key(KeyEvent::Char('ˇ'));

    // Should have candidates
    let context = ime.context();
    assert!(
        context.candidates.len() > 0,
        "Should have candidates for ㄋㄧˇ"
    );

    // Select first candidate with space
    ime.process_key(KeyEvent::Space);

    let context = ime.context();
    assert!(
        !context.commit_text.is_empty(),
        "Should have committed text"
    );
}

#[test]
fn test_ime_number_selection() {
    let mut ime = create_ime_engine_standard(DATA_DIR, 5).expect("Failed to create IME engine");

    // Type ㄋㄧˇ (ni third tone)
    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));
    ime.process_key(KeyEvent::Char('ˇ'));

    let context = ime.context();
    let num_candidates = context.candidates.len();

    if num_candidates >= 2 {
        // Select second candidate with number 2
        ime.process_key(KeyEvent::Number(2));

        let context = ime.context();
        assert!(
            !context.commit_text.is_empty(),
            "Should have committed text from number selection"
        );
    }
}

#[test]
fn test_ime_backspace() {
    let mut ime = create_ime_engine_standard(DATA_DIR, 5).expect("Failed to create IME engine");

    // Type some input with tones
    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));
    ime.process_key(KeyEvent::Char('ˇ'));

    let context = ime.context();
    let preedit_len_before = context.preedit_text.len();

    // Backspace once
    ime.process_key(KeyEvent::Backspace);

    let context = ime.context();
    assert!(
        context.preedit_text.len() < preedit_len_before,
        "Preedit should be shorter after backspace"
    );
}

#[test]
fn test_ime_escape_clears() {
    let mut ime = create_ime_engine_standard(DATA_DIR, 5).expect("Failed to create IME engine");

    // Type some input
    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));
    ime.process_key(KeyEvent::Char('ˇ'));

    // Press escape
    ime.process_key(KeyEvent::Escape);

    let context = ime.context();
    assert!(
        context.preedit_text.is_empty(),
        "Preedit should be cleared after escape"
    );
    assert!(
        context.candidates.is_empty(),
        "Candidates should be cleared after escape"
    );
}

#[test]
fn test_ime_page_navigation() {
    let mut ime = create_ime_engine_standard(DATA_DIR, 3)
        .expect("Failed to create IME engine with page size 3");

    // Type input that generates many candidates - use ㄧ (i) which is common
    ime.process_key(KeyEvent::Char('ㄧ'));

    let context = ime.context();
    let initial_candidates = context.candidates.clone();

    // Page down
    ime.process_key(KeyEvent::PageDown);

    let context = ime.context();
    // If there are more pages, candidates should change
    if !context.auxiliary_text.is_empty() && context.auxiliary_text.contains('/') {
        // Multi-page, so candidates on next page may be different
        let new_candidates = &context.candidates;
        // Just verify we have candidates on the new page
        assert!(
            !new_candidates.is_empty(),
            "Should have candidates on next page"
        );
    }
}

#[test]
fn test_ime_cursor_navigation() {
    let mut ime = create_ime_engine_standard(DATA_DIR, 5).expect("Failed to create IME engine");

    // Type input that generates candidates
    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));

    let context = ime.context();
    let initial_cursor = context.candidate_cursor;

    // Move cursor down
    ime.process_key(KeyEvent::Down);

    let context = ime.context();
    if context.candidates.len() > 1 {
        assert!(
            context.candidate_cursor > initial_cursor,
            "Cursor should move down"
        );

        // Move cursor up
        ime.process_key(KeyEvent::Up);

        let context = ime.context();
        assert_eq!(
            context.candidate_cursor, initial_cursor,
            "Cursor should return to initial position"
        );
    }
}

#[test]
fn test_hsu_fuzzy_matching() {
    // HSU layout: ㄓ and ㄐ share the 'j' key, should match with penalty
    let mut ime = create_ime_engine_hsu(DATA_DIR, 5).expect("Failed to create HSU IME engine");

    // Type ㄓ (zh sound)
    ime.process_key(KeyEvent::Char('ㄓ'));

    let context = ime.context();

    // Should have candidates - exact matches or fuzzy matches
    // With HSU fuzzy rules, ㄓ can match words with ㄐ (j sound)
    assert!(
        context.candidates.len() > 0,
        "HSU should provide candidates with fuzzy matching"
    );

    // Note: We can't easily test that specific fuzzy matches appear without
    // knowing the exact lexicon content, but we can verify candidates are generated
}

#[test]
fn test_standard_nasal_fuzzy_matching() {
    // Standard layout: ㄢ and ㄤ are commonly confused
    let mut ime =
        create_ime_engine_standard(DATA_DIR, 5).expect("Failed to create Standard IME engine");

    // Type a syllable with ㄢ
    ime.process_key(KeyEvent::Char('ㄊ'));
    ime.process_key(KeyEvent::Char('ㄢ'));

    let context = ime.context();

    // Should have candidates
    // With Standard fuzzy rules, ㄢ can match words with ㄤ
    assert!(
        context.candidates.len() > 0,
        "Standard should provide candidates with nasal fuzzy matching"
    );
}

#[test]
fn test_all_layouts_produce_candidates() {
    // Test each layout sequentially, ensuring cleanup between iterations
    let layouts = vec!["HSU", "Standard", "ETEN"];

    for name in layouts {
        let result = match name {
            "HSU" => create_ime_engine_hsu(DATA_DIR, 5),
            "Standard" => create_ime_engine_standard(DATA_DIR, 5),
            "ETEN" => create_ime_engine_eten(DATA_DIR, 5),
            _ => panic!("Unknown layout: {}", name),
        };

        assert!(
            result.is_ok(),
            "{} layout failed to create IME engine",
            name
        );

        let mut ime = result.unwrap();

        // Type complete syllable with tone: ㄋㄧˇ (ni3 = 你)
        ime.process_key(KeyEvent::Char('ㄋ'));
        ime.process_key(KeyEvent::Char('ㄧ'));
        ime.process_key(KeyEvent::Char('ˇ'));

        let context = ime.context();

        // All layouts should produce candidates for basic input
        assert!(
            context.candidates.len() > 0,
            "{} layout should produce candidates for 'ㄋㄧˇ'",
            name
        );

        // Explicitly drop to release userdict lock
        drop(ime);
    }
}

#[test]
fn test_ime_reset_state() {
    let mut ime = create_ime_engine_standard(DATA_DIR, 5).expect("Failed to create IME engine");

    // Type and commit
    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));
    ime.process_key(KeyEvent::Space);

    let context = ime.context();
    assert!(
        !context.commit_text.is_empty(),
        "Should have committed text"
    );

    // After commit, preedit should be cleared
    let context = ime.context();
    assert!(
        context.preedit_text.is_empty(),
        "Preedit should be cleared after commit"
    );
}

#[test]
fn test_ime_multiple_commits() {
    let mut ime = create_ime_engine_standard(DATA_DIR, 5).expect("Failed to create IME engine");

    // First commit
    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));
    ime.process_key(KeyEvent::Space);

    let first_commit = ime.context().commit_text.clone();
    assert!(!first_commit.is_empty(), "First commit should not be empty");

    // Second commit
    ime.process_key(KeyEvent::Char('ㄏ'));
    ime.process_key(KeyEvent::Char('ㄠ'));
    ime.process_key(KeyEvent::Space);

    let second_commit = ime.context().commit_text.clone();
    assert!(
        !second_commit.is_empty(),
        "Second commit should not be empty"
    );

    // Commits should be different (unless by chance they're the same phrase)
    // This mainly tests that the IME can handle multiple input cycles
}

#[test]
fn test_ime_enter_commits() {
    let mut ime = create_ime_engine_standard(DATA_DIR, 5).expect("Failed to create IME engine");

    // Type input
    ime.process_key(KeyEvent::Char('ㄋ'));
    ime.process_key(KeyEvent::Char('ㄧ'));

    // Press enter instead of space
    ime.process_key(KeyEvent::Enter);

    let context = ime.context();
    // Either commits selected candidate or raw input
    assert!(
        context.preedit_text.is_empty(),
        "Preedit should be cleared after enter"
    );
}

#[test]
fn test_different_page_sizes() {
    for page_size in [3, 5, 10] {
        let mut ime =
            create_ime_engine_standard(DATA_DIR, page_size).expect("Failed to create IME engine");

        // Type input
        ime.process_key(KeyEvent::Char('ㄋ'));

        let context = ime.context();
        // Number of candidates on first page should not exceed page_size
        assert!(
            context.candidates.len() <= page_size,
            "Candidates on page should not exceed page size {}",
            page_size
        );
    }
}

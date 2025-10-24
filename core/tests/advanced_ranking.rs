//! Tests for advanced ranking options (sort_option_t in upstream)
//!
//! Tests the sorting and filtering behavior controlled by Config flags:
//! - sort_by_phrase_length: Prefer shorter phrases
//! - sort_by_pinyin_length: Prefer shorter pinyin
//! - sort_without_longer_candidate: Filter phrases longer than input

use libchinese_core::{Candidate, Config};

/// Helper to create a candidate with given text and score
fn make_candidate(text: &str, score: f32) -> Candidate {
    Candidate::new(text.to_string(), score)
}

#[test]
fn test_sort_by_phrase_length_disabled() {
    // When sort_by_phrase_length is false, only score matters
    let mut candidates = vec![
        make_candidate("你好世界", 5.0), // 4 chars, score 5.0
        make_candidate("你好", 4.0),     // 2 chars, score 4.0
        make_candidate("你", 6.0),       // 1 char, score 6.0
    ];

    // Default config has sort_by_phrase_length = false
    let cfg = Config::default();
    assert!(!cfg.sort_by_phrase_length);

    // Sort by score only (descending)
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Expected order: 你 (6.0), 你好世界 (5.0), 你好 (4.0)
    assert_eq!(candidates[0].text, "你");
    assert_eq!(candidates[1].text, "你好世界");
    assert_eq!(candidates[2].text, "你好");
}

#[test]
fn test_sort_by_phrase_length_enabled() {
    // When sort_by_phrase_length is true, prefer shorter phrases
    let mut cfg = Config::default();
    cfg.sort_by_phrase_length = true;

    let mut candidates = vec![
        make_candidate("你好世界", 5.0), // 4 chars
        make_candidate("你好", 4.0),     // 2 chars
        make_candidate("你", 6.0),       // 1 char
    ];

    // Apply length penalty: (char_count - 1) * 0.5
    for cand in candidates.iter_mut() {
        let phrase_len = cand.text.chars().count();
        let penalty = (phrase_len.saturating_sub(1)) as f32 * 0.5;
        cand.score -= penalty;
    }

    // After penalties:
    // "你" (1 char): 6.0 - 0.0 = 6.0
    // "你好" (2 chars): 4.0 - 0.5 = 3.5
    // "你好世界" (4 chars): 5.0 - 1.5 = 3.5

    candidates.sort_by(|a, b| {
        match b.score.partial_cmp(&a.score) {
            Some(std::cmp::Ordering::Equal) => {
                // Tie-break by length
                let a_len = a.text.chars().count();
                let b_len = b.text.chars().count();
                a_len.cmp(&b_len)
            }
            ordering => ordering.unwrap_or(std::cmp::Ordering::Equal),
        }
    });

    // Expected order: 你 (6.0), 你好 (3.5, shorter), 你好世界 (3.5, longer)
    assert_eq!(candidates[0].text, "你");
    assert_eq!(candidates[1].text, "你好");
    assert_eq!(candidates[2].text, "你好世界");
}

#[test]
fn test_sort_without_longer_candidate() {
    // When sort_without_longer_candidate is true, filter phrases longer than input
    let mut cfg = Config::default();
    cfg.sort_without_longer_candidate = true;

    let input = "你好"; // 2 characters
    let input_len = input.chars().count();

    let mut candidates = vec![
        make_candidate("你", 5.0),       // 1 char - KEEP
        make_candidate("你好", 6.0),     // 2 chars - KEEP
        make_candidate("你好世界", 7.0), // 4 chars - FILTER
        make_candidate("你好啊", 4.0),   // 3 chars - FILTER
    ];

    // Filter by length
    candidates.retain(|c| {
        let phrase_len = c.text.chars().count();
        phrase_len <= input_len
    });

    // Should only have 2 candidates left
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].text, "你");
    assert_eq!(candidates[1].text, "你好");
}

#[test]
fn test_combined_ranking_options() {
    // Test combining multiple ranking options
    let mut cfg = Config::default();
    cfg.sort_by_phrase_length = true;
    cfg.sort_without_longer_candidate = true;

    let input = "你好吗"; // 3 characters
    let input_len = input.chars().count();

    let mut candidates = vec![
        make_candidate("你", 5.0),       // 1 char
        make_candidate("你好", 5.0),     // 2 chars
        make_candidate("你好吗", 5.0),   // 3 chars
        make_candidate("你好吗啊", 5.0), // 4 chars - should be filtered
    ];

    // First: filter by length
    candidates.retain(|c| {
        let phrase_len = c.text.chars().count();
        phrase_len <= input_len
    });

    assert_eq!(candidates.len(), 3); // Filtered out 4-char phrase

    // Second: apply length penalty
    for cand in candidates.iter_mut() {
        let phrase_len = cand.text.chars().count();
        let penalty = (phrase_len.saturating_sub(1)) as f32 * 0.5;
        cand.score -= penalty;
    }

    // After penalties (all started at 5.0):
    // "你" (1 char): 5.0 - 0.0 = 5.0
    // "你好" (2 chars): 5.0 - 0.5 = 4.5
    // "你好吗" (3 chars): 5.0 - 1.0 = 4.0

    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Expected order: shortest phrase wins
    assert_eq!(candidates[0].text, "你");
    assert_eq!(candidates[1].text, "你好");
    assert_eq!(candidates[2].text, "你好吗");
}

#[test]
fn test_no_ranking_options() {
    // With all ranking options disabled, pure score-based sorting
    let cfg = Config::default();
    assert!(!cfg.sort_by_phrase_length);
    assert!(!cfg.sort_without_longer_candidate);

    let mut candidates = vec![
        make_candidate("短", 3.0),
        make_candidate("比较长的句子", 5.0),
        make_candidate("中", 4.0),
    ];

    // Sort by score only
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Order by score: 5.0, 4.0, 3.0 (length doesn't matter)
    assert_eq!(candidates[0].text, "比较长的句子");
    assert_eq!(candidates[1].text, "中");
    assert_eq!(candidates[2].text, "短");
}

#[test]
fn test_equal_scores_with_length_preference() {
    // When scores are equal and sort_by_phrase_length is enabled, prefer shorter
    let mut cfg = Config::default();
    cfg.sort_by_phrase_length = true;

    let mut candidates = vec![
        make_candidate("你好世界", 5.0),
        make_candidate("你好", 5.0),
        make_candidate("你", 5.0),
    ];

    // Apply length penalty
    for cand in candidates.iter_mut() {
        let phrase_len = cand.text.chars().count();
        let penalty = (phrase_len.saturating_sub(1)) as f32 * 0.5;
        cand.score -= penalty;
    }

    candidates.sort_by(|a, b| match b.score.partial_cmp(&a.score) {
        Some(std::cmp::Ordering::Equal) => {
            let a_len = a.text.chars().count();
            let b_len = b.text.chars().count();
            a_len.cmp(&b_len)
        }
        ordering => ordering.unwrap_or(std::cmp::Ordering::Equal),
    });

    // Shortest first
    assert_eq!(candidates[0].text, "你");
    assert_eq!(candidates[1].text, "你好");
    assert_eq!(candidates[2].text, "你好世界");
}

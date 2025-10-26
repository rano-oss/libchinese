/// Prefix trie for syllable validation and segmentation.
use std::collections::HashMap;

/// A simple Trie (prefix tree) for storing valid syllables.
///
/// Used by parsers in libpinyin and libzhuyin for syllable validation
/// and prefix matching during DP-based segmentation.
#[derive(Debug, Default)]
pub struct TrieNode {
    children: HashMap<char, Box<TrieNode>>,
    is_end: bool,
    /// When `is_end` is true, `word` contains the syllable string.
    word: Option<String>,
}

impl TrieNode {
    /// Create a new empty trie root.
    pub fn new() -> Self {
        Self {
            children: HashMap::new(),
            is_end: false,
            word: None,
        }
    }

    /// Insert a syllable into the trie.
    pub fn insert(&mut self, syllable: &str) {
        let mut node = self;
        for ch in syllable.chars() {
            node = node
                .children
                .entry(ch)
                .or_insert_with(|| Box::new(TrieNode::new()));
        }
        node.is_end = true;
        node.word = Some(syllable.to_string());
    }

    /// Check whether the trie contains exactly the given word.
    ///
    /// Returns `true` only if `word` exists as a complete syllable,
    /// not just as a prefix.
    pub fn contains_word(&self, word: &str) -> bool {
        let mut node = self;
        for ch in word.chars() {
            if let Some(child) = node.children.get(&ch) {
                node = child;
            } else {
                return false;
            }
        }
        node.is_end
    }

    /// Walk the trie starting at a position in `input` and return all matched
    /// prefixes.
    ///
    /// This is the core operation used during DP segmentation: from a given
    /// position, find all valid syllables that can start there.
    ///
    /// # Arguments
    /// * `input` - The full input as a character slice
    /// * `start` - The character index to start walking from
    ///
    /// # Returns
    /// Vector of `(end_index, matched_string)` tuples where:
    /// - `end_index` is the exclusive character index after the match
    /// - `matched_string` is the syllable text
    ///
    /// Results are returned in order of increasing length.
    pub fn walk_prefixes(&self, input: &[char], start: usize) -> Vec<(usize, String)> {
        let mut res = Vec::new();
        let mut node = self;
        let mut idx = start;
        while idx < input.len() {
            let ch = input[idx];
            if let Some(child) = node.children.get(&ch) {
                node = child;
                idx += 1;
                if node.is_end {
                    if let Some(w) = &node.word {
                        res.push((idx, w.clone()));
                    }
                }
            } else {
                break;
            }
        }
        res
    }
}

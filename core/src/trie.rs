/// Prefix trie for syllable validation and segmentation.
use std::collections::HashMap;

/// A simple Trie (prefix tree) for storing valid syllables.
///
/// Used by parsers in libpinyin and libzhuyin for syllable validation
/// and prefix matching during DP-based segmentation.
///
/// # Example
/// ```
/// use libchinese_core::trie::TrieNode;
///
/// let mut trie = TrieNode::new();
/// trie.insert("ni");
/// trie.insert("hao");
///
/// assert!(trie.contains_word("ni"));
/// assert!(!trie.contains_word("n"));
///
/// let input: Vec<char> = "nihao".chars().collect();
/// let prefixes = trie.walk_prefixes(&input, 0);
/// assert_eq!(prefixes.len(), 1);
/// assert_eq!(prefixes[0], (2, "ni".to_string()));
/// ```
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
    ///
    /// # Example
    /// ```
    /// use libchinese_core::trie::TrieNode;
    ///
    /// let mut trie = TrieNode::new();
    /// trie.insert("ni");
    /// assert!(trie.contains_word("ni"));
    /// ```
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
    ///
    /// # Example
    /// ```
    /// use libchinese_core::trie::TrieNode;
    ///
    /// let mut trie = TrieNode::new();
    /// trie.insert("nihao");
    ///
    /// assert!(trie.contains_word("nihao"));
    /// assert!(!trie.contains_word("ni"));  // prefix, not a complete word
    /// ```
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
    ///
    /// # Example
    /// ```
    /// use libchinese_core::trie::TrieNode;
    ///
    /// let mut trie = TrieNode::new();
    /// trie.insert("ni");
    /// trie.insert("hao");
    ///
    /// let input: Vec<char> = "nihao".chars().collect();
    ///
    /// // From position 0, should match "ni"
    /// let prefixes = trie.walk_prefixes(&input, 0);
    /// assert_eq!(prefixes.len(), 1);
    /// assert_eq!(prefixes[0], (2, "ni".to_string()));
    ///
    /// // From position 2, should match "hao"
    /// let prefixes = trie.walk_prefixes(&input, 2);
    /// assert_eq!(prefixes.len(), 1);
    /// assert_eq!(prefixes[0], (5, "hao".to_string()));
    /// ```
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_insert_and_contains() {
        let mut trie = TrieNode::new();
        trie.insert("ni");
        trie.insert("hao");
        trie.insert("nihao");

        assert!(trie.contains_word("ni"));
        assert!(trie.contains_word("hao"));
        assert!(trie.contains_word("nihao"));
        assert!(!trie.contains_word("n"));
        assert!(!trie.contains_word("ha"));
        assert!(!trie.contains_word("niha"));
    }

    #[test]
    fn test_walk_prefixes_basic() {
        let mut trie = TrieNode::new();
        trie.insert("ni");
        trie.insert("hao");

        let input: Vec<char> = "nihao".chars().collect();

        let prefixes = trie.walk_prefixes(&input, 0);
        assert_eq!(prefixes.len(), 1);
        assert_eq!(prefixes[0], (2, "ni".to_string()));

        let prefixes = trie.walk_prefixes(&input, 2);
        assert_eq!(prefixes.len(), 1);
        assert_eq!(prefixes[0], (5, "hao".to_string()));
    }

    #[test]
    fn test_walk_prefixes_multiple_matches() {
        let mut trie = TrieNode::new();
        trie.insert("n");
        trie.insert("ni");
        trie.insert("nih");

        let input: Vec<char> = "nihao".chars().collect();

        let prefixes = trie.walk_prefixes(&input, 0);
        assert_eq!(prefixes.len(), 3);
        assert_eq!(prefixes[0], (1, "n".to_string()));
        assert_eq!(prefixes[1], (2, "ni".to_string()));
        assert_eq!(prefixes[2], (3, "nih".to_string()));
    }

    #[test]
    fn test_walk_prefixes_no_match() {
        let mut trie = TrieNode::new();
        trie.insert("ni");
        trie.insert("hao");

        let input: Vec<char> = "xihao".chars().collect();

        let prefixes = trie.walk_prefixes(&input, 0);
        assert_eq!(prefixes.len(), 0);
    }

    #[test]
    fn test_unicode_zhuyin() {
        let mut trie = TrieNode::new();
        trie.insert("ㄋㄧˇ");
        trie.insert("ㄏㄠˇ");

        assert!(trie.contains_word("ㄋㄧˇ"));
        assert!(trie.contains_word("ㄏㄠˇ"));

        let input: Vec<char> = "ㄋㄧˇㄏㄠˇ".chars().collect();
        let prefixes = trie.walk_prefixes(&input, 0);
        assert_eq!(prefixes.len(), 1);
        assert_eq!(prefixes[0].1, "ㄋㄧˇ");
    }
}

/*!
libzhuyin parser skeleton - `libzhuyin/src/parser.rs`

Purpose
-------
- Provide a small, testable starting implementation of a Zhuyin (bopomofo)
  segmentation parser.
- Mirror the pinyin parser design (trie + DP segmentation) but specialized for
  zhuyin syllables and potential fuzzy rules.
- This file is intentionally lightweight and correctness-first; it is meant to
  be extended with language-specific fuzzy rules, table loaders, and test
  vectors in later phases.

References (upstream libpinyin)
- src/storage/zhuyin_parser2.cpp
- src/storage/zhuyin_table.h

Notes
-----
- The parser operates on Unicode characters (Bopomofo or ASCII tokens).
- Public API:
  - `ZhuyinParser::with_syllables(&[&str])` to seed the syllable set.
  - `segment_best(&self, input: &str, allow_fuzzy: bool) -> Vec<ZhuyinSyllable>`
  - `segment_top_k(&self, input: &str, k: usize, allow_fuzzy: bool) -> Vec<Vec<ZhuyinSyllable>>`
- FuzzyMap included as a small placeholder. Real fuzzy rules for zhuyin can be
  richer (tone-insensitive mapping, alternate finals, etc).
*/

use std::collections::HashMap;

/// A matched zhuyin syllable with metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZhuyinSyllable {
    /// The matched syllable token (e.g. a bopomofo sequence or romanized form).
    pub text: String,
    /// Whether this syllable was matched via a fuzzy alternative.
    pub fuzzy: bool,
}

impl ZhuyinSyllable {
    pub fn new<T: Into<String>>(text: T, fuzzy: bool) -> Self {
        Self {
            text: text.into(),
            fuzzy,
        }
    }
}

/// Simple Trie node for Unicode characters (used for zhuyin syllable storage).
#[derive(Default, Debug)]
pub struct TrieNode {
    children: HashMap<char, Box<TrieNode>>,
    is_end: bool,
    /// When `is_end` == true, `word` stores the syllable as a string.
    word: Option<String>,
}

impl TrieNode {
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

    /// Check if the trie contains the exact word.
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

    /// Walk prefixes starting at `start` (char index) over `input_chars`.
    /// Returns vector of (end_index_exclusive, matched_string).
    pub fn walk_prefixes(&self, input_chars: &[char], start: usize) -> Vec<(usize, String)> {
        let mut res = Vec::new();
        let mut node = self;
        let mut idx = start;
        while idx < input_chars.len() {
            let ch = input_chars[idx];
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

/// Placeholder fuzzy map for zhuyin.
/// Real rules may include:
///  - tone-insensitive matching
///  - finals merging
///  - common alternates
#[derive(Default, Debug)]
pub struct ZhuyinFuzzy {
    /// map canonical -> alternatives
    map: HashMap<String, Vec<String>>,
    /// penalty applied for fuzzy substitutions (tunable)
    #[allow(dead_code)]
    pub penalty: f32,
}

impl ZhuyinFuzzy {
    pub fn new() -> Self {
        let mut fm = Self {
            map: HashMap::new(),
            penalty: 1.0,
        };

        // Example symmetric mappings (illustrative; adjust as needed)
        // These keys might be bopomofo characters or ascii representations.
        fm.add_pair("ㄓ", "ㄗ");
        fm.add_pair("ㄔ", "ㄘ");
        fm.add_pair("ㄕ", "ㄙ");
        fm.add_pair("ㄌ", "ㄋ");

        fm
    }

    fn add_pair(&mut self, a: &str, b: &str) {
        self.map
            .entry(a.to_string())
            .or_default()
            .push(b.to_string());
        self.map
            .entry(b.to_string())
            .or_default()
            .push(a.to_string());
    }

    /// Return alternatives including original token.
    pub fn alternatives(&self, tok: &str) -> Vec<String> {
        let key = tok.trim().to_string();
        let mut out = Vec::new();
        out.push(key.clone());
        if let Some(alts) = self.map.get(&key) {
            for a in alts.iter() {
                if !out.contains(a) {
                    out.push(a.clone());
                }
            }
        }
        out
    }


}

/// The public Zhuyin parser type.
#[derive(Debug)]
pub struct ZhuyinParser {
    trie: TrieNode,
    fuzzy: ZhuyinFuzzy,
}

impl ZhuyinParser {
    pub fn new() -> Self {
        Self {
            trie: TrieNode::new(),
            fuzzy: ZhuyinFuzzy::new(),
        }
    }

    /// Create a parser seeded with a list of valid zhuyin syllables.
    /// Syllables are inserted verbatim (unicode-aware).
    pub fn with_syllables<T: AsRef<str>>(syllables: &[T]) -> Self {
        let mut p = ZhuyinParser::new();
        for s in syllables {
            p.insert_syllable(s.as_ref());
        }
        p
    }

    /// Insert a single syllable into the internal trie.
    pub fn insert_syllable(&mut self, syllable: &str) {
        if !syllable.trim().is_empty() {
            self.trie.insert(syllable.trim());
        }
    }



    /// Best segmentation using dynamic programming.
    /// `allow_fuzzy` permits fuzzy-alternative matching with a penalty.
    ///
    /// Cost model (simple starter):
    /// - exact match: cost 1.0
    /// - fuzzy match: cost 1.5
    /// - unknown char fallback: cost 10.0
    ///
    /// The DP minimizes total cost (prefer longer / fewer segments).
    pub fn segment_best(&self, input: &str, allow_fuzzy: bool) -> Vec<ZhuyinSyllable> {
        // Normalize: remove whitespace, operate on char vector
        let chars: Vec<char> = input.chars().filter(|c| !c.is_whitespace()).collect();
        let n = chars.len();
        if n == 0 {
            return Vec::new();
        }

        // dp_cost[i] = best cost for suffix starting at i
        let mut dp_cost: Vec<f32> = vec![std::f32::INFINITY; n + 1];
        // dp_choice[i] = Option<(next_pos, matched_string, fuzzy_flag)>
        let mut dp_choice: Vec<Option<(usize, String, bool)>> = vec![None; n + 1];

        dp_cost[n] = 0.0;

        // iterate backward
        for pos in (0..n).rev() {
            // exact matches from trie
            let prefixes = self.trie.walk_prefixes(&chars, pos);
            for (end, matched) in prefixes.iter() {
                let seg_cost = 1.0;
                let cand = seg_cost + dp_cost[*end];
                if cand < dp_cost[pos] {
                    dp_cost[pos] = cand;
                    dp_choice[pos] = Some((*end, matched.clone(), false));
                }
            }

            // fuzzy attempts (if enabled): try short substrings for alternative matching
            if allow_fuzzy {
                // Consider lengths up to 3-4 chars (typical zhuyin syllable lengths small)
                for len in 1..=4 {
                    if pos + len > n {
                        break;
                    }
                    let substr: String = chars[pos..pos + len].iter().collect();
                    let alts = self.fuzzy.alternatives(&substr);
                    for alt in alts.into_iter() {
                        if self.trie.contains_word(&alt) {
                            // require same char-length match for this placeholder approach
                            if alt.chars().count() == substr.chars().count() {
                                let end = pos + len;
                                let seg_cost = 1.5; // fuzzy penalty
                                let cand = seg_cost + dp_cost[end];
                                if cand < dp_cost[pos] {
                                    dp_cost[pos] = cand;
                                    dp_choice[pos] = Some((end, alt.clone(), true));
                                }
                            }
                        }
                    }
                }
            }

            // fallback: consume one char as unknown token with high penalty
            if dp_choice[pos].is_none() {
                let end = pos + 1;
                let substr: String = chars[pos..end].iter().collect();
                let seg_cost = 10.0;
                let cand = seg_cost + dp_cost[end];
                if cand < dp_cost[pos] {
                    dp_cost[pos] = cand;
                    dp_choice[pos] = Some((end, substr, false));
                }
            }
        }

        // reconstruct segmentation from dp_choice[0]
        let mut out: Vec<ZhuyinSyllable> = Vec::new();
        let mut cur = 0usize;
        while cur < n {
            if let Some((next, text, fuzzy_flag)) = &dp_choice[cur] {
                out.push(ZhuyinSyllable::new(text.clone(), *fuzzy_flag));
                cur = *next;
            } else {
                // defensive: consume single char
                let ch: String = chars[cur].to_string();
                out.push(ZhuyinSyllable::new(ch, false));
                cur += 1;
            }
        }
        out
    }

    /// Return top-K segmentations. Placeholder: returns best segmentation only.
    /// A full implementation should enumerate alternatives (beam search / k-best DP).
    pub fn segment_top_k(
        &self,
        input: &str,
        _k: usize,
        allow_fuzzy: bool,
    ) -> Vec<Vec<ZhuyinSyllable>> {
        vec![self.segment_best(input, allow_fuzzy)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trie_basic_insert_and_contains() {
        let mut trie = TrieNode::new();
        trie.insert("ㄓ");
        trie.insert("ㄗ");
        assert!(trie.contains_word("ㄓ"));
        assert!(trie.contains_word("ㄗ"));
        assert!(!trie.contains_word("ㄔ"));
    }

    #[test]
    fn fuzzy_basic_alternatives() {
        let fm = ZhuyinFuzzy::new();
        let alts = fm.alternatives("ㄓ");
        assert!(alts.contains(&"ㄓ".to_string()));
        assert!(alts.contains(&"ㄗ".to_string()));
        assert!(fm.is_equivalent("ㄓ", "ㄗ"));
    }

    #[test]
    fn parser_segment_simple() {
        let mut p = ZhuyinParser::new();
        // seed a few syllables (using bopomofo characters or ASCII placeholders)
        p.insert_syllable("ㄋㄧ");
        p.insert_syllable("ㄏㄠ");
        p.insert_syllable("ㄓㄨㄥ");
        p.insert_syllable("ㄍㄨㄛ");

        let seg = p.segment_best("ㄋㄧㄏㄠ", false);
        let texts: Vec<String> = seg.into_iter().map(|s| s.text).collect();
        assert_eq!(texts, vec!["ㄋㄧ".to_string(), "ㄏㄠ".to_string()]);

        let seg2 = p.segment_best("ㄓㄨㄥㄍㄨㄛ", false);
        let texts2: Vec<String> = seg2.into_iter().map(|s| s.text).collect();
        assert_eq!(texts2, vec!["ㄓㄨㄥ".to_string(), "ㄍㄨㄛ".to_string()]);
    }

    #[test]
    fn parser_unknown_fallback() {
        let mut p = ZhuyinParser::new();
        p.insert_syllable("ㄋㄧ");
        let seg = p.segment_best("ㄋㄧX", false);
        let texts: Vec<String> = seg.into_iter().map(|s| s.text).collect();
        assert_eq!(texts, vec!["ㄋㄧ".to_string(), "X".to_string()]);
    }
}

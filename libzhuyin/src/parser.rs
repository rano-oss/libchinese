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

use libchinese_core::{FuzzyMap, TrieNode};

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

/// Zhuyin fuzzy matching now uses the shared `libchinese_core::FuzzyMap`.
///
/// The parser is initialized with fuzzy rules from `crate::standard_fuzzy_rules()`
/// which includes:
///  - HSU keyboard layout corrections (ㄓ/ㄐ, ㄔ/ㄑ, ㄕ/ㄒ, etc.)
///  - ETEN26 keyboard corrections
///  - Common bopomofo alternates
///
/// Users can provide custom rules via `with_fuzzy_rules()`.
///
/// See `libchinese_core::fuzzy` module for the implementation.

/// The public Zhuyin parser type.
#[derive(Debug)]
pub struct ZhuyinParser {
    trie: TrieNode,
    fuzzy: FuzzyMap,
}

impl ZhuyinParser {
    /// Create a parser seeded with a list of valid zhuyin syllables and fuzzy rules.
    pub fn new<T: AsRef<str>>(fuzzy_rules: Vec<String>, syllables: &[T]) -> Self {
        let mut trie = TrieNode::new();
        for s in syllables {
            trie.insert(s.as_ref());
        }
        Self {
            trie,
            fuzzy: FuzzyMap::from_rules(&fuzzy_rules),
        }
    }

    /// Apply zhuyin corrections to a string.
    /// Returns corrected alternatives (similar to pinyin corrections).
    ///
    /// Corrections (from libpinyin's ZHUYIN_* flags):
    /// - SHUFFLE: medial/final order corrections (e.g., ㄨㄟ ↔ ㄩㄟ)
    /// - HSU: HSU keyboard layout corrections (ㄓ/ㄐ, ㄔ/ㄑ, ㄕ/ㄒ)
    /// - ETEN26: ETEN26 keyboard layout corrections (ㄓ/ㄗ, ㄔ/ㄘ, ㄕ/ㄙ)
    pub fn apply_corrections(&self, s: &str) -> Vec<String> {
        let mut results = Vec::new();

        // ZHUYIN_CORRECT_SHUFFLE: medial/final order corrections
        // ㄨㄟ <-> ㄩㄟ (u-final vs ü-final confusion)
        if s.contains("ㄨㄟ") {
            results.push(s.replace("ㄨㄟ", "ㄩㄟ"));
        }
        if s.contains("ㄩㄟ") {
            results.push(s.replace("ㄩㄟ", "ㄨㄟ"));
        }

        // ㄨㄣ <-> ㄩㄣ correction
        if s.contains("ㄨㄣ") {
            results.push(s.replace("ㄨㄣ", "ㄩㄣ"));
        }
        if s.contains("ㄩㄣ") {
            results.push(s.replace("ㄩㄣ", "ㄨㄣ"));
        }

        // ZHUYIN_CORRECT_HSU: HSU keyboard layout corrections
        // ㄓ/ㄐ, ㄔ/ㄑ, ㄕ/ㄒ confusion (HSU maps these to same keys)
        let hsu_pairs = vec![
            ("ㄓ", "ㄐ"),
            ("ㄐ", "ㄓ"),
            ("ㄔ", "ㄑ"),
            ("ㄑ", "ㄔ"),
            ("ㄕ", "ㄒ"),
            ("ㄒ", "ㄕ"),
        ];

        for (from, to) in hsu_pairs {
            if s.contains(from) {
                results.push(s.replace(from, to));
            }
        }

        // ZHUYIN_CORRECT_ETEN26: ETEN26 keyboard layout corrections
        // ㄓ/ㄗ, ㄔ/ㄘ, ㄕ/ㄙ confusion (ETEN26-specific errors)
        let eten_pairs = vec![
            ("ㄓ", "ㄗ"),
            ("ㄗ", "ㄓ"),
            ("ㄔ", "ㄘ"),
            ("ㄘ", "ㄔ"),
            ("ㄕ", "ㄙ"),
            ("ㄙ", "ㄕ"),
        ];

        for (from, to) in eten_pairs {
            if s.contains(from) {
                results.push(s.replace(from, to));
            }
        }

        results
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
    /// For custom penalty configuration, use `segment_best_with_config`.
    pub fn segment_best(&self, input: &str, allow_fuzzy: bool) -> Vec<ZhuyinSyllable> {
        let config = libchinese_core::Config::default();
        self.segment_best_internal(input, allow_fuzzy, &config)
    }

    /// Perform segmentation with custom config for penalty tuning.
    pub fn segment_best_with_config(
        &self,
        input: &str,
        allow_fuzzy: bool,
        config: &libchinese_core::Config,
    ) -> Vec<ZhuyinSyllable> {
        self.segment_best_internal(input, allow_fuzzy, config)
    }

    fn segment_best_internal(
        &self,
        input: &str,
        allow_fuzzy: bool,
        config: &libchinese_core::Config,
    ) -> Vec<ZhuyinSyllable> {
        // Normalize: remove whitespace, operate on char vector
        let chars: Vec<char> = input.chars().filter(|c| !c.is_whitespace()).collect();
        let n = chars.len();
        if n == 0 {
            return Vec::new();
        }

        // dp_cost[i] = best cost for suffix starting at i
        let mut dp_cost: Vec<f32> = vec![f32::INFINITY; n + 1];
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

                    // Try zhuyin corrections first (shuffle, HSU, ETEN26) - lower penalty than fuzzy
                    let corrections = self.apply_corrections(&substr);
                    for corrected in corrections {
                        if self.trie.contains_word(&corrected) && corrected != substr {
                            let end = pos + len;
                            if end <= n && !dp_cost[end].is_infinite() {
                                let seg_cost = config.correction_penalty as f32; // Correction penalty from config (default: 200)
                                let cand = seg_cost + dp_cost[end];
                                if cand < dp_cost[pos] {
                                    dp_cost[pos] = cand;
                                    dp_choice[pos] = Some((end, corrected.clone(), false));
                                }
                            }
                        }
                    }

                    // Then try fuzzy alternatives
                    let alts = self.fuzzy.alternatives(&substr);
                    for (alt, penalty) in alts.into_iter() {
                        if self.trie.contains_word(&alt) {
                            // require same char-length match for this placeholder approach
                            if alt.chars().count() == substr.chars().count() {
                                let end = pos + len;
                                let seg_cost = penalty * (config.fuzzy_penalty_multiplier as f32); // use fuzzy penalty from config
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
                let seg_cost = config.unknown_cost; // penalty from config (default: 10.0)
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
    /// For custom penalty configuration, use `segment_top_k_with_config`.
    pub fn segment_top_k(
        &self,
        input: &str,
        _k: usize,
        allow_fuzzy: bool,
    ) -> Vec<Vec<ZhuyinSyllable>> {
        vec![self.segment_best(input, allow_fuzzy)]
    }

    /// Return top-K segmentations with custom config for penalty tuning.
    pub fn segment_top_k_with_config(
        &self,
        input: &str,
        _k: usize,
        allow_fuzzy: bool,
        config: &libchinese_core::Config,
    ) -> Vec<Vec<ZhuyinSyllable>> {
        vec![self.segment_best_with_config(input, allow_fuzzy, config)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fuzzy_basic_alternatives() {
        // Test the standard fuzzy rules
        let rules = crate::standard_fuzzy_rules();
        let fm = FuzzyMap::from_rules(&rules);

        // The alternatives() returns Vec<(String, f32)> now
        let alts = fm.alternatives("ㄓ");
        let alt_strings: Vec<String> = alts.iter().map(|(s, _)| s.clone()).collect();

        assert!(
            alt_strings.contains(&"ㄓ".to_string()),
            "Should contain original"
        );
        // Note: Actual alternatives depend on configured rules in standard_fuzzy_rules()
        // If ㄓ=ㄐ is configured, ㄐ should be in alternatives
    }

    #[test]
    fn parser_segment_simple() {
        let rules = crate::standard_fuzzy_rules();
        let mut p = ZhuyinParser::new(rules, &["ㄋㄧ", "ㄏㄠ", "ㄓㄨㄥ", "ㄍㄨㄛ"]);

        let seg = p.segment_best("ㄋㄧㄏㄠ", false);
        let texts: Vec<String> = seg.into_iter().map(|s| s.text).collect();
        assert_eq!(texts, vec!["ㄋㄧ".to_string(), "ㄏㄠ".to_string()]);

        let seg2 = p.segment_best("ㄓㄨㄥㄍㄨㄛ", false);
        let texts2: Vec<String> = seg2.into_iter().map(|s| s.text).collect();
        assert_eq!(texts2, vec!["ㄓㄨㄥ".to_string(), "ㄍㄨㄛ".to_string()]);
    }

    #[test]
    fn parser_unknown_fallback() {
        let rules = crate::standard_fuzzy_rules();
        let mut p = ZhuyinParser::new(rules, &["ㄋㄧ", "ㄏㄠ", "ㄓㄨㄥ", "ㄍㄨㄛ"]);
        let seg = p.segment_best("ㄋㄧX", false);
        let texts: Vec<String> = seg.into_iter().map(|s| s.text).collect();
        assert_eq!(texts, vec!["ㄋㄧ".to_string(), "X".to_string()]);
    }
}

// Implement core::SyllableType for ZhuyinSyllable
impl libchinese_core::SyllableType for ZhuyinSyllable {
    fn text(&self) -> &str {
        &self.text
    }

    fn is_fuzzy(&self) -> bool {
        self.fuzzy
    }
}

// Implement core::SyllableParser for ZhuyinParser
impl libchinese_core::SyllableParser for ZhuyinParser {
    type Syllable = ZhuyinSyllable;

    fn segment_top_k(&self, input: &str, k: usize, allow_fuzzy: bool) -> Vec<Vec<Self::Syllable>> {
        self.segment_top_k(input, k, allow_fuzzy)
    }
}

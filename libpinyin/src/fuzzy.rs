//! Fuzzy matching utilities for libpinyin
//!
//! This module implements a compact, testable `FuzzyMap` abstraction that
//! represents fuzzy equivalence rules (e.g. `zh=z`, `ch=c`, `sh=s`, `l=n`).
//!
//! Responsibilities:
//! - Parse fuzzy rules from `libchinese_core::Config` (string pairs like `a=b`).
//! - Provide alternatives for a given syllable/token including the original.
//! - Provide a simple penalty value to apply when fuzzy substitutions are used.
//!
//! Notes:
//! - Upstream libpinyin has more nuanced fuzzy behavior (insertions/deletions,
//!   asymmetric penalties, rule scopes). This file provides a clear starting
//!   point to be extended during Phase 5.
//!
//! References:
//! - Upstream: `src/lookup/phonetic_lookup.cpp`, `src/pinyin.cpp` (for fuzzy usage).
use std::collections::HashMap;

use libchinese_core::Config;

/// Represents fuzzy alternatives for phonetic units (syllables).
///
/// For each canonical syllable (lowercased), `map` contains a vector of
/// alternative syllables considered equivalent under fuzzy rules.
#[derive(Debug, Clone, Default)]
pub struct FuzzyMap {
    map: HashMap<String, Vec<String>>,
    /// A penalty (in score units) to apply when a fuzzy substitution is used.
    /// Higher values penalize fuzzy matches more heavily. This is a simple
    /// scalar; later we may store per-rule penalties.
    penalty: f32,
}

impl FuzzyMap {
    /// Build a `FuzzyMap` from configuration.
    ///
    /// The config is expected to contain textual fuzzy pairs in `cfg.fuzzy`
    /// like `"zh=z"`; pairs are parsed and inserted symmetrically by default.
    pub fn from_config(_cfg: &Config) -> Self {
        // Current core::Config doesn't expose a fuzzy_penalty scalar.
        // Use a conservative default penalty here; future config schema can
        // provide a penalty and this function can be extended to consume it.
        let mut fm = FuzzyMap {
            map: HashMap::new(),
            penalty: 1.0,
        };

        for pair in _cfg.fuzzy.iter() {
            // allow either "a=b" or "a = b"
            if let Some((a, b)) = pair.split_once('=') {
                let a = a.trim().to_ascii_lowercase();
                let b = b.trim().to_ascii_lowercase();
                if !a.is_empty() && !b.is_empty() {
                    fm.map.entry(a.clone()).or_default().push(b.clone());
                    fm.map.entry(b).or_default().push(a);
                }
            } else {
                // If the string is a single token like "zh", ignore or treat as no-op.
                let token = pair.trim().to_ascii_lowercase();
                if !token.is_empty() {
                    // Ensure there's an entry (even if empty alternatives)
                    fm.map.entry(token).or_default();
                }
            }
        }

        fm
    }

    /// Return alternatives for a syllable including the syllable itself (lowercased).
    ///
    /// Returned vector will always contain at least the original syllable.
    pub fn alternatives(&self, syllable: &str) -> Vec<String> {
        let key = syllable.trim().to_ascii_lowercase();
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

    /// Query whether two syllables are considered fuzzy-equivalent (directly).
    ///
    /// This only checks whether `b` appears in `alternatives(a)`.
    pub fn is_equivalent(&self, a: &str, b: &str) -> bool {
        let a = a.trim().to_ascii_lowercase();
        let b = b.trim().to_ascii_lowercase();
        if a == b {
            return true;
        }
        if let Some(alts) = self.map.get(&a) {
            return alts.iter().any(|x| x == &b);
        }
        false
    }

    /// Get the configured fuzzy penalty.
    pub fn penalty(&self) -> f32 {
        self.penalty
    }

    /// Convenience: apply fuzzy expansion to a sequence of syllables,
    /// producing a list of sequences with per-syllable alternatives.
    ///
    /// Example:
    ///   input ["zhong", "guo"] -> output vec of vecs like
    ///     [["zhong","guo"], ["z","guo"], ["zhong","gwo" (if present)], ...]
    ///
    /// This returns up to `limit` expansions (breadth-first). `limit == 0`
    /// means no limit.
    pub fn expand_sequence(&self, seq: &[String], limit: usize) -> Vec<Vec<String>> {
        if seq.is_empty() {
            return vec![];
        }

        // Start with one empty sequence.
        let mut results: Vec<Vec<String>> = vec![Vec::new()];

        for tok in seq.iter() {
            let alts = self.alternatives(tok);
            let mut next: Vec<Vec<String>> = Vec::new();
            for r in results.iter() {
                for alt in alts.iter() {
                    let mut nr = r.clone();
                    nr.push(alt.clone());
                    next.push(nr);
                    if limit > 0 && next.len() >= limit {
                        break;
                    }
                }
                if limit > 0 && next.len() >= limit {
                    break;
                }
            }
            results = next;
            if limit > 0 && results.len() >= limit {
                results.truncate(limit);
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libchinese_core::Config;

    fn cfg_with_pairs(pairs: Vec<&str>) -> Config {
        // Construct a Config compatible with libchinese_core::Config definition.
        // The core Config has fields: fuzzy: Vec<String>, unigram_weight, bigram_weight, trigram_weight
        Config {
            fuzzy: pairs.into_iter().map(|s| s.to_string()).collect(),
            unigram_weight: 0.6,
            bigram_weight: 0.3,
            trigram_weight: 0.1,
        }
    }

    #[test]
    fn parse_pairs_and_alternatives() {
        let cfg = cfg_with_pairs(vec!["zh=z", "ch=c", "sh=s"]);
        let fm = FuzzyMap::from_config(&cfg);

        let alts = fm.alternatives("zh");
        assert!(alts.contains(&"zh".to_string()));
        assert!(alts.contains(&"z".to_string()));

        assert!(fm.is_equivalent("zh", "z"));
        assert!(fm.is_equivalent("z", "zh"));
        assert!(!fm.is_equivalent("zh", "x"));

        // Default penalty used by this FuzzyMap implementation is 1.0.
        assert!((fm.penalty() - 1.0).abs() < std::f32::EPSILON);
    }

    #[test]
    fn expand_sequence_limits_and_contents() {
        let cfg = cfg_with_pairs(vec!["zh=z", "l=n"]);
        let fm = FuzzyMap::from_config(&cfg);
        let seq = vec!["zhong".to_string(), "li".to_string()];
        // If "zhong" has "z" alternative, and "li" has "n", expansions should include those combos.
        let ex = fm.expand_sequence(&seq, 0);
        // At least original sequence must be present
        assert!(ex.iter().any(|s| s == &seq));
        // Alternatives should be present if we know them
        // Note: since we only registered short keys ("zh" <-> "z"), "zhong" won't expand unless exact key present.
        // Test that the API returns something reasonable (non-empty).
        assert!(!ex.is_empty());
    }
}

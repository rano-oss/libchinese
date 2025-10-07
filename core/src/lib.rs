//! libchinese-core
//!
//! Core model, dictionary, n-gram scoring, user dictionary and configuration
//! shared by language-specific crates (libpinyin, libzhuyin).
//!
//! This file contains lightweight, well-documented implementations and public
//! types intended to be used by downstream crates. Implementations are
//! intentionally pragmatic and easy to replace with more optimized versions
//! (fst-backed lexicon, redb-backed user dictionary, etc).
//!
//! Public API:
//! - `Candidate`
//! - `Model`
//! - `NGramModel`
//! - `Lexicon`
//! - `UserDict`
//! - `Config`
use serde::{Deserialize, Serialize};
use std::collections::HashMap as AHashMap;
use std::sync::{Arc, RwLock};

/// A single text candidate with an associated score.
///
/// Scores are on a relative scale; higher is better. Use `f32` for compactness
/// and performance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub text: String,
    pub score: f32,
}

impl Candidate {
    pub fn new<T: Into<String>>(text: T, score: f32) -> Self {
        Candidate {
            text: text.into(),
            score,
        }
    }
}

/// Configuration for fuzzy matching and n-gram weights.
///
/// This is designed to be deserialized from TOML via `serde`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Fuzzy equivalence rules represented as pairs like "zh=z".
    /// Downstream code can parse these into maps.
    pub fuzzy: Vec<String>,

    /// Weights for linear interpolation of n-gram probabilities.
    /// Expected to sum (or be normalized by the scoring code).
    pub unigram_weight: f32,
    pub bigram_weight: f32,
    pub trigram_weight: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            fuzzy: vec!["zh=z".into(), "ch=c".into(), "sh=s".into(), "l=n".into()],
            unigram_weight: 0.6,
            bigram_weight: 0.3,
            trigram_weight: 0.1,
        }
    }
}

/// Utility helpers.
pub mod utils {
    /// Normalize input strings (NFC) and trim whitespace.
    pub fn normalize(s: &str) -> String {
        use unicode_normalization::UnicodeNormalization;
        s.nfc().collect::<String>().trim().to_string()
    }
}

/// Simple in-memory lexicon.
///
/// Lookups map a pinyin-sequence key (e.g. "nihao") to a list of Chinese
/// phrases. This is intentionally basic; downstream crates can replace this
/// with an `fst`-based implementation for space/time improvements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Lexicon {
    // Keyed by a "joined" pinyin sequence. In practice, language crates will
    // choose a canonical joiner (like ""), or join on spaces.
    map: AHashMap<String, Vec<String>>,
}

impl Lexicon {
    pub fn new() -> Self {
        Self {
            map: AHashMap::new(),
        }
    }

    /// Insert a mapping from pinyin key to phrase.
    pub fn insert<K: Into<String>, V: Into<String>>(&mut self, key: K, phrase: V) {
        let key = key.into();
        let phrase = phrase.into();
        self.map.entry(key).or_default().push(phrase);
    }

    /// Lookup candidates for a given pinyin key.
    pub fn lookup(&self, key: &str) -> Vec<String> {
        self.map.get(key).cloned().unwrap_or_else(|| Vec::new())
    }

    /// Load a small default lexicon (for tests or quick demos).
    pub fn load_demo() -> Self {
        let mut lx = Self::new();
        lx.insert("nihao", "你好");
        lx.insert("nihao", "你号");
        lx.insert("zhongguo", "中国");
        lx.insert("zhongguo", "中华");
        lx
    }
}

/// Lightweight NGram model with unigram, bigram and trigram log-probabilities.
///
/// Probabilities are stored as natural log (ln) values. Scoring performs a
/// linear interpolation of n-gram probabilities using configurable lambda
/// weights from `Config`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NGramModel {
    // The maps store log probabilities for each n-gram.
    unigram: AHashMap<String, f32>,                   // log(p(w))
    bigram: AHashMap<(String, String), f32>,          // log(p(w2|w1))
    trigram: AHashMap<(String, String, String), f32>, // log(p(w3|w1,w2))
}

impl NGramModel {
    pub fn new() -> Self {
        Self {
            unigram: AHashMap::new(),
            bigram: AHashMap::new(),
            trigram: AHashMap::new(),
        }
    }

    /// Insert unigram probability (natural log).
    pub fn insert_unigram(&mut self, w: &str, log_p: f32) {
        self.unigram.insert(w.to_string(), log_p);
    }

    /// Insert bigram probability (natural log).
    pub fn insert_bigram(&mut self, w1: &str, w2: &str, log_p: f32) {
        self.bigram.insert((w1.to_string(), w2.to_string()), log_p);
    }

    /// Insert trigram probability (natural log).
    pub fn insert_trigram(&mut self, w1: &str, w2: &str, w3: &str, log_p: f32) {
        self.trigram
            .insert((w1.to_string(), w2.to_string(), w3.to_string()), log_p);
    }

    /// Score a token sequence (slice of tokens) using interpolation weights.
    ///
    /// This is a simple implementation:
    /// score = sum_t [ lambda1 * logP_unigram(t) + lambda2 * logP_bigram(prev,t) + lambda3 * logP_trigram(prev2,prev,t) ]
    ///
    /// For missing n-grams, the implementation falls back to unigram log-prob
    /// if present, otherwise a small floor probability is used.
    pub fn score_sequence(&self, tokens: &[String], cfg: &Config) -> f32 {
        if tokens.is_empty() {
            return std::f32::NEG_INFINITY;
        }

        // floor log-probability: a very small probability in log space
        let floor = -20.0f32; // ~= 2e-9

        let mut score = 0f32;
        for i in 0..tokens.len() {
            let u = self.unigram.get(&tokens[i]).copied().unwrap_or(floor);

            let b = if i >= 1 {
                let key = (tokens[i - 1].clone(), tokens[i].clone());
                self.bigram.get(&key).copied().unwrap_or(u)
            } else {
                u
            };

            let t = if i >= 2 {
                let key = (
                    tokens[i - 2].clone(),
                    tokens[i - 1].clone(),
                    tokens[i].clone(),
                );
                self.trigram.get(&key).copied().unwrap_or(b)
            } else {
                b
            };

            let interpolated =
                cfg.unigram_weight * u + cfg.bigram_weight * b + cfg.trigram_weight * t;
            score += interpolated;
        }

        score
    }
}

/// User dictionary holding learned phrase frequencies.
///
/// This implementation stores data in-memory via a thread-safe map. It is
/// intended to be swapped for a persistent backend (e.g. `redb`) later.
#[derive(Debug, Default, Clone)]
pub struct UserDict {
    // Map phrase -> frequency (or score)
    inner: Arc<RwLock<AHashMap<String, u64>>>,
}

impl UserDict {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(AHashMap::new())),
        }
    }

    /// Increment the learned count for `phrase` by 1.
    pub fn learn(&self, phrase: &str) {
        if let Ok(mut map) = self.inner.write() {
            let c = map.entry(phrase.to_string()).or_insert(0);
            *c += 1;
        }
    }

    /// Get learned frequency for `phrase`.
    pub fn frequency(&self, phrase: &str) -> u64 {
        if let Ok(map) = self.inner.read() {
            map.get(phrase).copied().unwrap_or(0)
        } else {
            0
        }
    }

    /// Merge another UserDict into this one (summing frequencies).
    pub fn merge_from(&self, other: &UserDict) {
        if let (Ok(mut dst), Ok(src)) = (self.inner.write(), other.inner.read()) {
            for (k, v) in src.iter() {
                let entry = dst.entry(k.clone()).or_insert(0);
                *entry += *v;
            }
        }
    }
}

/// High-level Model combining lexicon, n-gram model and user dictionary.
///
/// Downstream engine implementations (lang-specific) will use this Model to
/// generate and score candidates.
#[derive(Debug, Clone)]
pub struct Model {
    pub lexicon: Arc<Lexicon>,
    pub ngram: Arc<NGramModel>,
    pub userdict: UserDict,
    pub config: Config,
}

impl Model {
    /// Create a new model with defaults.
    pub fn new(lexicon: Lexicon, ngram: NGramModel, userdict: UserDict, config: Config) -> Self {
        Self {
            lexicon: Arc::new(lexicon),
            ngram: Arc::new(ngram),
            userdict,
            config,
        }
    }

    /// Candidate generation from a joined pinyin key.
    ///
    /// This function:
    /// 1. Looks up lexicon candidates for the provided key.
    /// 2. Scores each candidate using the n-gram model and boosts using userdict frequency.
    /// 3. Returns top `limit` results sorted by score descending.
    ///
    /// Note: The mapping from candidate text to token sequence is language-specific.
    /// Here we treat each character as a token for scoring demo purposes.
    pub fn candidates_for_key(&self, key: &str, limit: usize) -> Vec<Candidate> {
        let raw = self.lexicon.lookup(key);
        let mut cands: Vec<Candidate> = raw
            .into_iter()
            .map(|phrase| {
                // Tokenize: for demo, split by char to create tokens.
                let tokens: Vec<String> = phrase.chars().map(|c| c.to_string()).collect();
                let mut score = self.ngram.score_sequence(&tokens, &self.config);

                // Boost with user frequency (log-ish)
                let freq = self.userdict.frequency(&phrase);
                if freq > 0 {
                    // small boost: log(1 + freq)
                    score += (1.0 + (freq as f32)).ln();
                }

                Candidate::new(phrase, score)
            })
            .collect();

        // sort descending
        cands.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        cands.truncate(limit);
        cands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_and_lexicon_demo() {
        let lx = Lexicon::load_demo();
        let mut ng = NGramModel::new();

        // simple unigram log-probabilities for characters used in demo
        ng.insert_unigram("你", -1.0);
        ng.insert_unigram("好", -1.2);
        ng.insert_unigram("号", -2.0);
        ng.insert_unigram("中", -1.1);
        ng.insert_unigram("国", -1.3);
        ng.insert_unigram("华", -2.2);

        let user = UserDict::new();
        user.learn("你好"); // increase frequency of "你好"

        let cfg = Config::default();
        let model = Model::new(lx, ng, user, cfg);

        let cands = model.candidates_for_key("nihao", 10);
        assert!(!cands.is_empty());
        // first candidate should be "你好" because of user learn boost (frequency)
        assert_eq!(cands[0].text, "你好");
    }

    #[test]
    fn ngram_interpolation_behaviour() {
        let mut ng = NGramModel::new();
        // unigram log p
        ng.insert_unigram("a", -1.0);
        ng.insert_unigram("b", -1.5);
        ng.insert_unigram("c", -2.0);
        // bigram log p
        ng.insert_bigram("a", "b", -0.2);
        ng.insert_bigram("b", "c", -0.3);
        // trigram log p
        ng.insert_trigram("a", "b", "c", -0.05);

        let cfg = Config {
            unigram_weight: 0.5,
            bigram_weight: 0.3,
            trigram_weight: 0.2,
            fuzzy: vec![],
        };

        let tokens = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let score = ng.score_sequence(&tokens, &cfg);

        // Score should be finite and dominated by better (higher) n-gram values.
        assert!(score.is_finite());
        // With trigram present, score should be higher (less negative) than pure unigram sum.
        let unigram_sum = -1.0 + -1.5 + -2.0;
        assert!(score > unigram_sum);
    }
}

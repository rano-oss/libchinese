//! libchinese-core
//!
//! Core model, dictionary, n-gram scoring, user dictionary and configuration
//! shared by language-specific crates (libpinyin, libzhuyin).
//!
//! This crate provides production-ready implementations using FST for lexicons,
//! bincode for serialization, and redb for user dictionaries only.
//!
//! Public API:
//! - `Candidate` - Scored text candidate with metadata
//! - `Model` - Complete language model combining all components
//! - `NGramModel` - Statistical language model with backoff smoothing
//! - `Lexicon` - Pinyin/Zhuyin → Hanzi dictionary lookup
//! - `UserDict` - Persistent user learning and frequency adaptation
//! - `Config` - Configuration and feature flags
use serde::{Deserialize, Serialize};
use std::collections::HashMap as AHashMap;
use std::sync::Arc;
use fst::Map;
use std::fs::File;
use std::io::Read;
use bincode;

pub mod ngram;
pub use ngram::{Interpolator, Lambdas, NGramModel};

pub mod trie;
pub use trie::TrieNode;

pub mod fuzzy;
pub use fuzzy::{FuzzyMap, FuzzyRule};

pub mod engine;
pub use engine::{Engine, SyllableParser, SyllableType};

pub mod ime;
pub use ime::{
    ImeEngine, ImeSession, ImeContext, InputMode, KeyEvent, KeyResult,
    PhoneticEditor, PunctuationEditor, SuggestionEditor,
};

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

/// Generic configuration for IME core functionality.
/// 
/// This config contains only language-agnostic fields. Language-specific options
/// (pinyin corrections, zhuyin keyboard layouts, etc.) belong in `PinyinConfig`
/// or `ZhuyinConfig` in their respective crates.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Fuzzy equivalence rules (e.g., "zh=z", "an=ang")
    /// Language crates should populate this with appropriate defaults
    pub fuzzy: Vec<String>,
    
    /// Interpolation weights for n-gram probabilities
    pub unigram_weight: f32,
    pub bigram_weight: f32,
    pub trigram_weight: f32,
    
    // Advanced Ranking Options (similar to upstream libpinyin sort_option_t)
    /// Sort candidates by phrase length (prefer shorter phrases)
    pub sort_by_phrase_length: bool,
    /// Filter out candidates longer than input
    pub sort_without_longer_candidate: bool,
    
    // Prediction Settings (for predict_next feature)
    /// Maximum phrase length for predictions (1-5 characters)
    pub max_prediction_length: usize,
    /// Minimum log probability threshold for predictions (-20.0 to 0.0)
    pub min_prediction_frequency: f64,
    /// Prefer 2-character phrases in prediction ranking
    pub prefer_phrase_predictions: bool,
    
    // Suggestion Mode Settings
    /// Automatically enter suggestion mode after committing text
    pub auto_suggestion: bool,
    /// Minimum committed text length to trigger auto-suggestion (chars)
    pub min_suggestion_trigger_length: usize,
    
    // Cache Management
    /// Maximum number of entries in the input -> candidates cache
    pub max_cache_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Empty fuzzy rules by default - language crates will populate
            fuzzy: vec![],
            unigram_weight: 0.6,
            bigram_weight: 0.3,
            trigram_weight: 0.1,
            // Advanced ranking - disabled by default (score-only sorting)
            sort_by_phrase_length: false,
            sort_without_longer_candidate: false,
            // Prediction settings - favor 2-char phrases, moderate filtering
            max_prediction_length: 3,
            min_prediction_frequency: -15.0,
            prefer_phrase_predictions: true,
            // Suggestion mode - auto-enter after commits of 2+ chars
            auto_suggestion: true,
            min_suggestion_trigger_length: 2,
            // Cache management - 1000 entries is reasonable for most IME use
            max_cache_size: 1000,
        }
    }
}

impl Config {
    /// Load configuration from a TOML file.
    pub fn load_toml<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to a TOML file.
    pub fn save_toml<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Load configuration from TOML string.
    pub fn from_toml_str(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    /// Serialize configuration to TOML string.
    pub fn to_toml_string(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
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

pub mod userdict;
pub use userdict::UserDict;

/// Lexicon entry matching convert_table output format
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LexEntry {
    pub utf8: String,
    pub token: u32,
    pub freq: u32,
}

/// Lookups map a pinyin-sequence key (e.g. "nihao") to a list of Chinese
/// phrases. Uses FST for key indexing and bincode for payload storage.
#[derive(Debug, Clone, Default)]
pub struct Lexicon {
    // In-memory map for dynamic entries
    map: AHashMap<String, Vec<String>>,
    // FST map for key -> index lookups
    fst_map: Option<Map<Vec<u8>>>,
    // Bincode-serialized payload vector (index -> Vec<LexEntry>)
    payloads: Option<Vec<Vec<LexEntry>>>,
}

impl Lexicon {
    pub fn new() -> Self {
        Self {
            map: AHashMap::new(),
            fst_map: None,
            payloads: None,
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
        // Prefer in-memory map entries
        if let Some(v) = self.map.get(key) {
            return v.clone();
        }

        // FST + bincode lookup
        if let (Some(map), Some(payloads)) = (&self.fst_map, &self.payloads) {
            if let Some(idx) = map.get(key) {
                let index = idx as usize;
                if let Some(entries) = payloads.get(index) {
                    return entries.iter().map(|e| e.utf8.clone()).collect();
                }
            }
        }

        Vec::new()
    }

    /// Load lexicon from FST + bincode artifacts.
    /// 
    /// - fst_path: lexicon.fst file mapping keys to indices
    /// - bincode_path: lexicon.bincode file containing Vec<Vec<LexEntry>>
    pub fn load_from_fst_bincode<P: AsRef<std::path::Path>>(fst_path: P, bincode_path: P) -> Result<Self, String> {
        let fst_path = fst_path.as_ref();
        let bincode_path = bincode_path.as_ref();

        // Load FST
        let mut f = File::open(fst_path).map_err(|e| format!("open fst {}: {}", fst_path.display(), e))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| format!("read fst: {}", e))?;
        let map = Map::new(buf).map_err(|e| format!("fst map: {}", e))?;

        // Load bincode payloads
        let mut f = File::open(bincode_path).map_err(|e| format!("open bincode {}: {}", bincode_path.display(), e))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| format!("read bincode: {}", e))?;
        let payloads: Vec<Vec<LexEntry>> = bincode::deserialize(&buf)
            .map_err(|e| format!("deserialize bincode: {}", e))?;

        Ok(Self {
            map: AHashMap::new(),
            fst_map: Some(map),
            payloads: Some(payloads),
        })
    }
}

// UserDict is implemented in `core::userdict` and exported above.

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
    pub fn new(
        lexicon: Lexicon,
        ngram: NGramModel,
        userdict: UserDict,
        config: Config,
    ) -> Self {
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
                let mut score = self.ngram
                    .score_sequence_with_interpolator(&tokens, &self.config, key);

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

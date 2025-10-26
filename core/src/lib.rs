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
//! - `Lexicon` - Pinyin/Zhuyin → Hanzi dictionary lookup
//! - `UserDict` - Persistent user learning and frequency adaptation
//! - `Config` - Configuration and feature flags
use fst::Map;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap as AHashMap;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;

pub mod word_bigram;
pub use word_bigram::WordBigram;

pub mod trie;
pub use trie::TrieNode;

pub mod fuzzy;
pub use fuzzy::{FuzzyMap, FuzzyRule};

pub mod engine;
pub use engine::{Engine, SyllableParser, SyllableType};

pub mod userdict;
pub use userdict::UserDict;

// IME modules (flattened from ime/ subdirectory)
pub mod candidate;
pub use candidate::{Candidate, CandidateList};

pub mod composition;
pub use composition::{Composition, Segment};

pub mod context;
pub use context::ImeContext;

pub mod input_buffer;
pub use input_buffer::InputBuffer;

pub mod session;
pub use session::{ImeSession, InputMode};

pub mod editor;
pub use editor::{Editor, EditorResult, PhoneticEditor, PunctuationEditor, SuggestionEditor};

pub mod ime_engine;
pub use ime_engine::{ImeEngine, KeyEvent, KeyResult};

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

    // Suggestion Mode Settings
    /// Automatically enter suggestion mode after committing text
    pub auto_suggestion: bool,
    /// Minimum committed text length to trigger auto-suggestion (chars)
    pub min_suggestion_trigger_length: usize,

    // Full/Half Width Settings
    /// Enable full-width character conversion (ASCII to full-width)
    pub full_width_enabled: bool,

    // Candidate Selection
    /// Keys for selecting candidates (default: "123456789", alternative: "asdfghjkl")
    /// Must contain at least 1 character. First char selects 1st candidate, etc.
    pub select_keys: String,

    // Phrase Masking
    /// Set of phrases to hide from candidate suggestions
    pub masked_phrases: std::collections::HashSet<String>,

    // Parser Penalty Settings (for fuzzy matching and error correction)
    /// Penalty for correction rules (ue/ve, v/u in pinyin, or keyboard shuffles in zhuyin)
    /// Default: 200. Lower values make corrections more likely to be selected.
    pub correction_penalty: i32,
    /// Penalty multiplier for fuzzy matching rules (z/zh, c/ch, s/sh, etc.)
    /// Default: 100. This value is multiplied by the rule's weight from the fuzzy map.
    pub fuzzy_penalty_multiplier: i32,
    /// Penalty for incomplete syllable matches (partial input like "n" → "ni")
    /// Default: 500. Only applies to pinyin parser with allow_fuzzy enabled.
    pub incomplete_penalty: i32,
    /// Penalty for unknown/unrecognized input characters
    /// Default: 1000. Very high to strongly discourage non-phonetic input.
    pub unknown_penalty: i32,
    /// Cost penalty for unknown segments in cost calculation
    /// Default: 10.0. Added to segment cost for unrecognized characters.
    pub unknown_cost: f32,

    /// Boost (additive) applied to score for exact full-key matches.
    /// Larger values prefer exact dictionary entries over composed alternatives.
    pub full_key_boost: f32,
    /// Lambda parameter for interpolation model (unigram/bigram mixing)
    /// Lambda is the weight for bigram probability: score = λ*P(w2|w1) + (1-λ)*P(w2)
    /// Upstream libpinyin default: 0.293 (trained via deleted interpolation)
    /// Range: [0.0, 1.0], where 0 = pure unigram, 1 = pure bigram
    pub lambda: f32,
    /// Sentence length penalty factor (upstream LONG_SENTENCE_PENALTY)
    /// Applied per word in the path to discourage over-segmentation
    /// Upstream value: ln(1.2) ≈ 0.1823
    pub sentence_length_penalty: f32,
    /// Unigram factor for user learning (upstream unigram_factor)
    /// Multiplier for frequency boost when adding user-learned phrases
    /// Upstream value: 7 for training, 3 for boosting existing entries
    pub unigram_factor: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Empty fuzzy rules by default - language crates will populate
            fuzzy: vec![],
            // Suggestion mode - auto-enter after commits of 2+ chars
            auto_suggestion: true,
            min_suggestion_trigger_length: 2,
            // Full/half width - disabled by default
            full_width_enabled: false,
            // Selection keys - default to numbers 1-9
            select_keys: "123456789".to_string(),
            // Phrase masking - empty by default
            masked_phrases: std::collections::HashSet::new(),
            // Parser penalties - balanced defaults for fuzzy matching
            correction_penalty: 200,
            fuzzy_penalty_multiplier: 100,
            incomplete_penalty: 500,
            unknown_penalty: 1000,
            unknown_cost: 10.0,
            // Exact-match boost: prefer full-key dictionary entries slightly
            full_key_boost: 2.0,
            // Lambda for interpolation: upstream default 0.293 (trained)
            // We'll start with a similar value
            lambda: 0.3,
            // Upstream LONG_SENTENCE_PENALTY = log(1.2) ≈ 0.1823
            sentence_length_penalty: 1.2_f32.ln(),
            // Upstream unigram_factor for user learning boost
            unigram_factor: 3.0,
        }
    }
}

impl Config {
    /// Load configuration from a TOML file.
    pub fn load_toml<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to a TOML file.
    pub fn save_toml<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

    // ========== Full/Half Width Management ==========

    /// Toggle full-width mode on/off.
    pub fn toggle_fullwidth(&mut self) {
        self.full_width_enabled = !self.full_width_enabled;
    }

    /// Set full-width mode explicitly.
    pub fn set_fullwidth(&mut self, enabled: bool) {
        self.full_width_enabled = enabled;
    }

    /// Check if full-width mode is enabled.
    pub fn is_fullwidth(&self) -> bool {
        self.full_width_enabled
    }

    // ========== Phrase Masking API ==========

    /// Add a phrase to the mask list (hide from suggestions).
    pub fn mask_phrase(&mut self, phrase: &str) {
        self.masked_phrases.insert(phrase.to_string());
    }

    /// Remove a phrase from the mask list (allow in suggestions).
    pub fn unmask_phrase(&mut self, phrase: &str) -> bool {
        self.masked_phrases.remove(phrase)
    }

    /// Check if a phrase is masked.
    pub fn is_masked(&self, phrase: &str) -> bool {
        self.masked_phrases.contains(phrase)
    }

    /// Clear all masked phrases.
    pub fn clear_masked_phrases(&mut self) {
        self.masked_phrases.clear();
    }

    /// Get all masked phrases as a sorted vector.
    pub fn get_masked_phrases(&self) -> Vec<String> {
        let mut phrases: Vec<_> = self.masked_phrases.iter().cloned().collect();
        phrases.sort();
        phrases
    }

    // ========== Selection Keys Management ==========

    /// Set the selection keys string.
    ///
    /// # Example
    /// ```
    /// # use libchinese_core::Config;
    /// let mut config = Config::default();
    /// config.set_select_keys("asdfghjkl"); // Use home row keys
    /// ```
    pub fn set_select_keys(&mut self, keys: &str) {
        if !keys.is_empty() {
            self.select_keys = keys.to_string();
        }
    }

    /// Get the current selection keys.
    pub fn get_select_keys(&self) -> &str {
        &self.select_keys
    }

    /// Check if a character is a selection key and return its index (0-based).
    /// Returns None if the character is not a selection key.
    pub fn selection_key_index(&self, ch: char) -> Option<usize> {
        self.select_keys.chars().position(|c| c == ch)
    }

    // ========== Parser Penalty Configuration ==========

    /// Set the correction penalty (ue/ve, v/u, keyboard shuffles).
    /// Lower values make corrections more likely to be selected.
    /// Default: 200
    pub fn set_correction_penalty(&mut self, penalty: i32) {
        self.correction_penalty = penalty;
    }

    /// Get the current correction penalty.
    pub fn get_correction_penalty(&self) -> i32 {
        self.correction_penalty
    }

    /// Set the fuzzy penalty multiplier (z/zh, c/ch, s/sh, etc.).
    /// This is multiplied by the rule's weight from the fuzzy map.
    /// Default: 100
    pub fn set_fuzzy_penalty_multiplier(&mut self, multiplier: i32) {
        self.fuzzy_penalty_multiplier = multiplier;
    }

    /// Get the current fuzzy penalty multiplier.
    pub fn get_fuzzy_penalty_multiplier(&self) -> i32 {
        self.fuzzy_penalty_multiplier
    }

    /// Set the incomplete penalty (partial input like "n" → "ni").
    /// Only applies to pinyin parser with allow_fuzzy enabled.
    /// Default: 500
    pub fn set_incomplete_penalty(&mut self, penalty: i32) {
        self.incomplete_penalty = penalty;
    }

    /// Get the current incomplete penalty.
    pub fn get_incomplete_penalty(&self) -> i32 {
        self.incomplete_penalty
    }

    /// Set the unknown character penalty.
    /// Very high to strongly discourage non-phonetic input.
    /// Default: 1000
    pub fn set_unknown_penalty(&mut self, penalty: i32) {
        self.unknown_penalty = penalty;
    }

    /// Get the current unknown character penalty.
    pub fn get_unknown_penalty(&self) -> i32 {
        self.unknown_penalty
    }

    /// Set the unknown segment cost penalty.
    /// Added to segment cost for unrecognized characters.
    /// Default: 10.0
    pub fn set_unknown_cost(&mut self, cost: f32) {
        self.unknown_cost = cost;
    }

    /// Get the current unknown segment cost.
    pub fn get_unknown_cost(&self) -> f32 {
        self.unknown_cost
    }
}

/// Utility helpers.
pub mod utils {
    /// Normalize input strings (NFC) and trim whitespace.
    pub fn normalize(s: &str) -> String {
        use unicode_normalization::UnicodeNormalization;
        s.nfc().collect::<String>().trim().to_string()
    }

    /// Convert ASCII characters to full-width equivalents.
    ///
    /// This converts:
    /// - ASCII letters (A-Z, a-z) → Full-width letters (Ａ-Ｚ, ａ-ｚ)
    /// - ASCII digits (0-9) → Full-width digits (０-９)
    /// - ASCII space → Ideographic space (　)
    /// - ASCII punctuation → Full-width punctuation
    ///
    /// Non-ASCII characters are passed through unchanged.
    pub fn to_fullwidth(s: &str) -> String {
        s.chars()
            .map(|ch| match ch {
                // Space -> Ideographic space
                ' ' => '\u{3000}',
                // ASCII printable range (0x21-0x7E) -> Full-width (0xFF01-0xFF5E)
                '!'..='~' => {
                    let code = ch as u32;
                    char::from_u32(code - 0x21 + 0xFF01).unwrap_or(ch)
                }
                // Pass through non-ASCII
                _ => ch,
            })
            .collect()
    }

    /// Convert full-width characters back to ASCII (half-width).
    pub fn to_halfwidth(s: &str) -> String {
        s.chars()
            .map(|ch| match ch {
                // Ideographic space -> ASCII space
                '\u{3000}' => ' ',
                // Full-width range (0xFF01-0xFF5E) -> ASCII (0x21-0x7E)
                '\u{FF01}'..='\u{FF5E}' => {
                    let code = ch as u32;
                    char::from_u32(code - 0xFF01 + 0x21).unwrap_or(ch)
                }
                // Pass through non-full-width
                _ => ch,
            })
            .collect()
    }
}

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

    /// Lookup that also returns the lexicon frequency for each phrase (if available).
    ///
    /// For in-memory `map` entries the frequency is unknown (0). For FST/bincode
    /// entries the stored `LexEntry.freq` is returned.
    pub fn lookup_with_freq(&self, key: &str) -> Vec<(String, u32)> {
        // Prefer in-memory map entries
        if let Some(v) = self.map.get(key) {
            return v.iter().cloned().map(|s| (s, 0)).collect();
        }

        // FST + bincode lookup
        if let (Some(map), Some(payloads)) = (&self.fst_map, &self.payloads) {
            if let Some(idx) = map.get(key) {
                let index = idx as usize;
                if let Some(entries) = payloads.get(index) {
                    return entries.iter().map(|e| (e.utf8.clone(), e.freq)).collect();
                }
            }
        }

        Vec::new()
    }

    /// Cheap existence check for a key.
    ///
    /// Returns true if the key exists either in the in-memory `map` or in the
    /// FST index. This avoids deserializing payloads when only existence is
    /// required.
    pub fn has_key(&self, key: &str) -> bool {
        // Check dynamic in-memory entries first
        if self.map.contains_key(key) {
            return true;
        }

        // Check FST index without touching payloads
        if let Some(map) = &self.fst_map {
            return map.get(key).is_some();
        }

        false
    }

    /// Compute total frequency of all lexicon entries (for unigram probability normalization).
    ///
    /// This sums up all frequencies from all payloads. The result is cached in Model.
    pub fn compute_total_frequency(&self) -> u64 {
        let mut total: u64 = 0;

        if let Some(payloads) = &self.payloads {
            for entries in payloads {
                for entry in entries {
                    total += entry.freq as u64;
                }
            }
        }

        total
    }

    /// Load lexicon from FST + bincode artifacts.
    ///
    /// - fst_path: lexicon.fst file mapping keys to indices
    /// - bincode_path: lexicon.bincode file containing Vec<Vec<LexEntry>>
    pub fn load_from_fst_bincode<P: AsRef<std::path::Path>>(
        fst_path: P,
        bincode_path: P,
    ) -> Result<Self, String> {
        let fst_path = fst_path.as_ref();
        let bincode_path = bincode_path.as_ref();

        // Load FST
        let mut f =
            File::open(fst_path).map_err(|e| format!("open fst {}: {}", fst_path.display(), e))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)
            .map_err(|e| format!("read fst: {}", e))?;
        let map = Map::new(buf).map_err(|e| format!("fst map: {}", e))?;

        // Load bincode payloads
        let mut f = File::open(bincode_path)
            .map_err(|e| format!("open bincode {}: {}", bincode_path.display(), e))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)
            .map_err(|e| format!("read bincode: {}", e))?;
        let payloads: Vec<Vec<LexEntry>> =
            bincode::deserialize(&buf).map_err(|e| format!("deserialize bincode: {}", e))?;

        Ok(Self {
            map: AHashMap::new(),
            fst_map: Some(map),
            payloads: Some(payloads),
        })
    }
}

// UserDict is implemented in `core::userdict` and exported above.

/// High-level Model combining lexicon, word bigram model and user dictionary.
///
/// Downstream engine implementations (lang-specific) will use this Model to
/// generate and score candidates.
#[derive(Debug, Clone)]
pub struct Model {
    pub lexicon: Arc<Lexicon>,
    pub word_bigram: Arc<WordBigram>,
    pub userdict: UserDict,
    pub config: RefCell<Config>,
    /// Total frequency of all lexicon entries (for unigram normalization)
    pub total_unigram_freq: u64,
}

impl Model {
    /// Create a new model with defaults.
    pub fn new(
        lexicon: Lexicon,
        word_bigram: WordBigram,
        userdict: UserDict,
        config: Config,
    ) -> Self {
        let total_unigram_freq = lexicon.compute_total_frequency();
        Self {
            lexicon: Arc::new(lexicon),
            word_bigram: Arc::new(word_bigram),
            userdict,
            config: RefCell::new(config),
            total_unigram_freq,
        }
    }
}

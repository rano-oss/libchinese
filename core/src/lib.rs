//! libchinese-core
//!
//! Core model, dictionary, n-gram scoring, user dictionary and configuration
//! shared by language-specific crates (libpinyin, libzhuyin).
//!
//! This crate provides production-ready implementations using FST for lexicons,
//! memory-mapped files for zero-copy access, and redb for user dictionaries.
//!
//! Public API:
//! - `Candidate` - Scored text candidate with metadata
//! - `Model` - Complete language model combining all components
//! - `Lexicon` - Memory-mapped pinyin/zhuyin → hanzi dictionary lookup
//! - `UserDict` - Persistent user learning and frequency adaptation
//! - `Config` - Configuration and feature flags
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::Arc;

pub mod lexicon;
pub use lexicon::Lexicon;

pub mod word_bigram;
pub use word_bigram::WordBigram;

pub mod trie;
pub use trie::TrieNode;

pub mod fuzzy;
pub use fuzzy::FuzzyMap;

pub mod engine;
pub use engine::{Engine, SyllableParser, SyllableType};

pub mod userdict;
pub use userdict::UserDict;

pub mod emoji;
pub use emoji::EmojiDict;

pub mod english;
pub use english::{EnglishDict, MAXIMAL_ENGLISH_CANDIDATES, MINIMAL_ENGLISH_CHARACTERS};

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

pub mod punct;
pub use punct::{is_chinese_punct_key, preferred_chinese_punct};

pub mod helpers;
pub use helpers::{expand_u_input, expand_v_input};

pub mod tone;
pub use tone::ToneMap;

pub mod ime_engine;
pub use ime_engine::{ImeEngine, KeyEvent, KeyResult};

/// How to order the candidate bar after scoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CandidateRankMode {
    /// Longer matched pinyin span first, then score (default; good for partial commit).
    #[default]
    MatchedSpanThenScore,
    /// Longer Chinese phrase first, then score (C `SORT_BY_PHRASE_LENGTH_AND_FREQUENCY`).
    /// Improves top-5 overlap with system libpinyin; short unigrams surface earlier.
    PhraseLengthThenScore,
}

/// High-level IME behaviour profile.
///
/// Defaults favour the best end-user experience (macOS/Windows-class sentence
/// composition, mixed input). Switch to [`ImeProfile::LibpinyinCompat`] when
/// you need candidate-bar behaviour closer to upstream C/C++ libpinyin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ImeProfile {
    /// Best UX: composed sentences in the bar, span-first ranking, mixed-input on.
    #[default]
    BestUx,
    /// Closer to C `pinyin_guess_candidates`: phrase-length sort, no sentence fold-in,
    /// English/emoji off (parity-friendly).
    LibpinyinCompat,
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

    /// Lambda parameter for interpolation model (unigram/bigram mixing).
    /// Upstream: `bigram_lambda = lambda`, `unigram_lambda = 1 - lambda`.
    /// Default matches system `table.conf` (`lambda parameter: 0.312699`).
    pub lambda: f32,
    /// Upstream `LONG_SENTENCE_PENALTY = log(1.2)`.
    /// Used when comparing trellis paths whose character lengths differ by 1,
    /// not subtracted from every word score.
    pub sentence_length_penalty: f32,
    /// Upstream `unigram_factor` (3 when learning): folds into phrase frequency as
    /// `lexicon_freq + user_count * unigram_factor` before normalizing by total_freq.
    pub unigram_factor: f32,
    /// When double-pinyin is active, show raw typed keys in preedit instead of
    /// expanded syllables (e.g. `wd` vs `wo'de`).
    #[serde(default)]
    pub show_raw_double_pinyin: bool,
    /// Emit the best multi-token DP sentence into the candidate list
    /// (folds C `pinyin_get_sentence` into the bar). Needed for long inputs
    /// like `woshinuoweiren` → 我是挪威人. Default: true.
    #[serde(default = "default_true")]
    pub emit_composed_sentences: bool,
    /// Primary candidate ordering after score computation.
    #[serde(default)]
    pub candidate_rank_mode: CandidateRankMode,
    /// Active high-level profile (informational + applied via `Engine::set_profile`).
    #[serde(default)]
    pub ime_profile: ImeProfile,
    /// 以词定字: `[` / `]` pick a character from the highlighted multi-char phrase
    /// (fcitx/Mac-class). Default: true.
    #[serde(default = "default_true")]
    pub choose_char_from_phrase: bool,
    /// Learn from commits (ibus `dynamic-adjust`, fcitx `Learning`). Default: true.
    #[serde(default = "default_true")]
    pub learning_enabled: bool,
    /// Commit Chinese punctuation in 中 mode (Mac/Windows default). Default: true.
    #[serde(default = "default_true")]
    pub chinese_punctuation: bool,
    /// Windows-style `v` helpers (numerals / dates). Default: true.
    #[serde(default = "default_true")]
    pub v_mode_enabled: bool,
    /// Windows-style `u` symbol helpers. Default: true.
    #[serde(default = "default_true")]
    pub u_mode_enabled: bool,
    /// Insert bigram predictions into the candidate bar while composing. Default: true.
    #[serde(default = "default_true")]
    pub inline_prediction: bool,
}

fn default_true() -> bool {
    true
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
            // Lambda for interpolation: match libpinyin table.conf
            lambda: 0.312699,
            // Upstream LONG_SENTENCE_PENALTY = log(1.2)
            sentence_length_penalty: 1.2_f32.ln(),
            // Upstream unigram_factor when adding learned phrase frequency
            unigram_factor: 3.0,
            show_raw_double_pinyin: false,
            emit_composed_sentences: true,
            candidate_rank_mode: CandidateRankMode::default(),
            ime_profile: ImeProfile::default(),
            choose_char_from_phrase: true,
            learning_enabled: true,
            chinese_punctuation: true,
            v_mode_enabled: true,
            u_mode_enabled: true,
            inline_prediction: true,
        }
    }
}

impl ImeProfile {
    /// Apply this profile's ranking / mixed-input defaults onto `config`.
    /// Does not change fuzzy AMB option bits (those stay on `PinyinConfig`).
    pub fn apply_to_config(self, config: &mut Config) {
        config.ime_profile = self;
        match self {
            ImeProfile::BestUx => {
                config.emit_composed_sentences = true;
                config.candidate_rank_mode = CandidateRankMode::MatchedSpanThenScore;
                // Mac/Windows-class association after commit (fcitx desktop off by
                // default; we prefer the desktop OS behaviour users already know).
                config.auto_suggestion = true;
                config.min_suggestion_trigger_length = 1;
                config.chinese_punctuation = true;
                config.v_mode_enabled = true;
                config.u_mode_enabled = true;
                config.inline_prediction = true;
            }
            ImeProfile::LibpinyinCompat => {
                // C keeps sentences on `pinyin_get_sentence`, not the candidate list.
                // Phrase-length sort mirrors SORT_BY_PHRASE_LENGTH_AND_FREQUENCY.
                config.emit_composed_sentences = false;
                config.candidate_rank_mode = CandidateRankMode::PhraseLengthThenScore;
                config.auto_suggestion = false;
            }
        }
    }
}

impl Config {
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
}

/// Utility helpers.
pub mod utils {
    /// Convert ASCII characters to full-width equivalents.
    pub fn to_fullwidth(s: &str) -> String {
        s.chars()
            .map(|ch| match ch {
                ' ' => '\u{3000}',
                '!'..='~' => {
                    let code = ch as u32;
                    char::from_u32(code - 0x21 + 0xFF01).unwrap_or(ch)
                }
                _ => ch,
            })
            .collect()
    }
}

// UserDict is implemented in `core::userdict` and exported above.

/// High-level Model combining lexicon, word bigram model and user dictionary.
///
/// Uses memory-mapped data files for minimal RSS. The mmap types provide the
/// same API as the heap types but with zero-copy access to on-disk data.
#[derive(Clone)]
pub struct Model {
    pub lexicon: RefCell<Arc<Lexicon>>,
    pub word_bigram: RefCell<Arc<WordBigram>>,
    pub userdict: UserDict,
    pub config: RefCell<Config>,
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Model")
            .field("lexicon", &"Lexicon")
            .field("word_bigram", &"WordBigram")
            .finish()
    }
}

impl Model {
    /// Create a new model with mmap-backed data.
    pub fn new(
        lexicon: Lexicon,
        word_bigram: WordBigram,
        userdict: UserDict,
        config: Config,
    ) -> Self {
        Self {
            lexicon: RefCell::new(Arc::new(lexicon)),
            word_bigram: RefCell::new(Arc::new(word_bigram)),
            userdict,
            config: RefCell::new(config),
        }
    }

    /// Swap the lexicon with a new one (e.g., for simplified/traditional toggle).
    pub fn swap_lexicon(&self, lexicon: Lexicon) {
        *self.lexicon.borrow_mut() = Arc::new(lexicon);
    }

    /// Swap the word bigram model (e.g., with lexicon on 简/繁 switch).
    pub fn swap_word_bigram(&self, word_bigram: WordBigram) {
        *self.word_bigram.borrow_mut() = Arc::new(word_bigram);
    }
}

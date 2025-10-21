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
    
    /// Parser options (bitflags-style, similar to libpinyin)
    /// Allow incomplete syllables (e.g., "n" → matches initials)
    pub allow_incomplete: bool,
    /// Correct common ue/ve confusion (e.g., "nue" ↔ "nve")
    pub correct_ue_ve: bool,
    /// Correct v/u confusion (e.g., "nv" ↔ "nu")
    pub correct_v_u: bool,
    /// Correct uen/un confusion (e.g., "juen" ↔ "jun") - PINYIN_CORRECT_UEN_UN
    pub correct_uen_un: bool,
    /// Correct gn/ng confusion (e.g., "bagn" ↔ "bang") - PINYIN_CORRECT_GN_NG
    pub correct_gn_ng: bool,
    /// Correct mg/ng confusion (e.g., "bamg" ↔ "bang") - PINYIN_CORRECT_MG_NG
    pub correct_mg_ng: bool,
    /// Correct iou/iu confusion (e.g., "liou" ↔ "liu") - PINYIN_CORRECT_IOU_IU
    pub correct_iou_iu: bool,
    
    // Zhuyin/Bopomofo corrections
    /// Allow incomplete zhuyin syllables (e.g., "ㄋ" → matches "ㄋㄧ", "ㄋㄚ") - ZHUYIN_INCOMPLETE
    pub zhuyin_incomplete: bool,
    /// Correct medial/final order errors (e.g., "ㄌㄨㄟ" ↔ "ㄌㄩㄟ") - ZHUYIN_CORRECT_SHUFFLE
    pub zhuyin_correct_shuffle: bool,
    /// HSU keyboard layout corrections - ZHUYIN_CORRECT_HSU
    pub zhuyin_correct_hsu: bool,
    /// ETEN26 keyboard layout corrections - ZHUYIN_CORRECT_ETEN26
    pub zhuyin_correct_eten26: bool,
    
    // Double Pinyin (Shuangpin) scheme selection
    /// Double pinyin scheme for alternative input method. None = standard full pinyin.
    /// Popular schemes: Microsoft (most common), ZiRanMa, ZiGuang, ABC, XiaoHe, PinYinPlusPlus
    pub double_pinyin_scheme: Option<String>,
    
    // Advanced Ranking Options (similar to upstream libpinyin sort_option_t)
    /// Sort candidates by phrase length (prefer shorter phrases)
    pub sort_by_phrase_length: bool,
    /// Sort candidates by pinyin length (prefer shorter pinyin)
    pub sort_by_pinyin_length: bool,
    /// Filter out candidates longer than input
    pub sort_without_longer_candidate: bool,
}

/// Sort options for candidate ranking (bitflags-style, similar to upstream)
///
/// Upstream libpinyin uses sort_option_t enum with bitflag values:
/// - SORT_BY_PHRASE_LENGTH: Prefer shorter phrases
/// - SORT_BY_PINYIN_LENGTH: Prefer shorter pinyin representations  
/// - SORT_WITHOUT_LONGER_CANDIDATE: Filter phrases longer than input
///
/// These can be combined to create custom sorting strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOption {
    /// No special sorting (score only)
    None,
    /// Prefer shorter phrases
    ByPhraseLength,
    /// Prefer shorter pinyin
    ByPinyinLength,
    /// Exclude candidates longer than input
    WithoutLongerCandidate,
}

impl SortOption {
    /// Check if this sort option should filter by phrase length
    pub fn filters_by_length(&self) -> bool {
        matches!(self, SortOption::WithoutLongerCandidate)
    }
    
    /// Check if this sort option affects sorting order
    pub fn affects_sort_order(&self) -> bool {
        matches!(self, SortOption::ByPhraseLength | SortOption::ByPinyinLength)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Comprehensive fuzzy rules based on libpinyin upstream
            fuzzy: vec![
                // Initial consonant confusion (shengmu)
                "zh=z".into(), "z=zh".into(),
                "ch=c".into(), "c=ch".into(), 
                "sh=s".into(), "s=sh".into(),
                "l=n".into(), "n=l".into(),
                "l=r".into(), "r=l".into(),
                "f=h".into(), "h=f".into(),
                "k=g".into(), "g=k".into(),
                // Final sound confusion (yunmu)
                "an=ang".into(), "ang=an".into(),
                "en=eng".into(), "eng=en".into(),
                "in=ing".into(), "ing=in".into(),
            ],
            unigram_weight: 0.6,
            bigram_weight: 0.3,
            // Parser options - enable all corrections by default for better UX
            allow_incomplete: true,  // Enable by default for better UX
            correct_ue_ve: true,     // Common typing mistakes
            correct_v_u: true,       // Common typing mistakes
            correct_uen_un: true,    // Common typing mistakes (NEW)
            correct_gn_ng: true,     // Common typing mistakes (NEW)
            correct_mg_ng: true,     // Common typing mistakes (NEW)
            correct_iou_iu: true,    // Common typing mistakes (NEW)
            // Zhuyin corrections - enable by default for better UX
            zhuyin_incomplete: true,        // Allow partial bopomofo input
            zhuyin_correct_shuffle: true,   // Correct medial/final order errors
            zhuyin_correct_hsu: true,       // HSU keyboard corrections
            zhuyin_correct_eten26: true,    // ETEN26 keyboard corrections
            // Double pinyin - None by default (standard full pinyin)
            double_pinyin_scheme: None,
            // Advanced ranking - disabled by default (score-only sorting)
            sort_by_phrase_length: false,
            sort_by_pinyin_length: false,
            sort_without_longer_candidate: false,
            trigram_weight: 0.1,
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
    pub interpolator: Arc<Interpolator>,
}

impl Model {
    /// Create a new model with defaults.
    pub fn new(
        lexicon: Lexicon,
        ngram: NGramModel,
        userdict: UserDict,
        config: Config,
        interpolator: Interpolator,
    ) -> Self {
        Self {
            lexicon: Arc::new(lexicon),
            ngram: Arc::new(ngram),
            userdict,
            config,
            interpolator: Arc::new(interpolator),
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
                    .score_sequence_with_interpolator(&tokens, &self.config, key, &*self.interpolator);

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

//! libchinese-core
//!
//! Core model, dictionary, n-gram scoring, user dictionary and configuration
//! shared by language-specific crates (libpinyin, libzhuyin).
//!
//! This crate provides production-ready implementations using FST for lexicons,
//! redb for user dictionaries, and bincode for serialization.
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
use fst::Streamer;
use redb::{Database, TableDefinition};
use std::fs::File;
use std::io::Read;
use bincode;

pub mod ngram;
pub use ngram::NGramModel;

pub mod trie;
pub use trie::TrieNode;

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

/// SingleGram container and helpers (in-memory test-oriented implementation).
///
/// Implemented in `core::single_gram`. This module mirrors upstream SingleGram
/// semantics used by the lookup and training code and is exported for use by
/// language crates and tests.
pub mod single_gram;
pub use single_gram::SingleGram;

pub mod interpolation;
pub use interpolation::{Interpolator, Lambdas};

pub mod userdict;
pub use userdict::UserDict;

/// Simple in-memory lexicon.
///
/// Metadata for lexicon storage format versioning and compatibility.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LexiconMetadata {
    pub version: String,
    pub created_at: String,
    pub source_tables: Vec<String>,
    pub entry_count: usize,
    pub fst_size_bytes: usize,
    pub db_size_bytes: usize,
}

impl Default for LexiconMetadata {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            created_at: format!("{:?}", std::time::SystemTime::now()),
            source_tables: vec![],
            entry_count: 0,
            fst_size_bytes: 0,
            db_size_bytes: 0,
        }
    }
}

/// Lookups map a pinyin-sequence key (e.g. "nihao") to a list of Chinese
/// phrases. This is intentionally basic; downstream crates can replace this
/// with an `fst`-based implementation for space/time improvements.
#[derive(Debug, Clone, Default)]
pub struct Lexicon {
    // Keyed by a "joined" pinyin sequence. In practice, language crates will
    // choose a canonical joiner (like ""), or join on spaces.
    map: AHashMap<String, Vec<String>>,
    // Optional fst map and redb database for on-demand lookups. If present
    // and the in-memory `map` doesn't contain the key, the Lexicon will use
    // the fst -> redb `phrases` table convention to materialize candidates
    // on demand without loading the whole lexicon into memory.
    fst_map: Option<Map<Vec<u8>>>,
    db: Option<Arc<Database>>,
    // A small mapping from apostrophe-free pinyin keys to fst index. This
    // speeds up lookups for inputs like "nihao" when the fst keys are
    // stored as "ni'hao".
    no_apos_map: Option<AHashMap<String, u64>>,
    // Metadata for the lexicon format
    metadata: LexiconMetadata,
}

// Local type used for deserializing phrase lists stored in the runtime
// redb `phrases` table.
#[derive(Deserialize)]
struct PhraseEntry {
    text: String,
    freq: u64,
}

impl Lexicon {
    pub fn new() -> Self {
        Self {
            map: AHashMap::new(),
            fst_map: None,
            db: None,
            no_apos_map: None,
            metadata: LexiconMetadata::default(),
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

        // Fst + redb on-demand lookup
        if let Some(map) = &self.fst_map {
            if let Some(idx) = map.get(key) {
                let id = idx as u64;
                if let Some(db_arc) = &self.db {
                    if let Ok(rt) = db_arc.begin_read() {
                        let td: TableDefinition<u64, Vec<u8>> = TableDefinition::new("phrases");
                        if let Ok(table) = rt.open_table(td) {
                            if let Ok(Some(val)) = table.get(&id) {
                                let bytes = val.value();
                                if let Ok(list) = bincode::deserialize::<Vec<PhraseEntry>>(&bytes) {
                                    return list.into_iter().map(|pe| pe.text).collect();
                                }
                            }
                        }
                    }
                }
            }
        }

        // Try apostrophe-free mapping if present
        if let Some(map_no) = &self.no_apos_map {
            if let Some(&id) = map_no.get(key) {
                if let Some(db_arc) = &self.db {
                    if let Ok(rt) = db_arc.begin_read() {
                        let td: TableDefinition<u64, Vec<u8>> = TableDefinition::new("phrases");
                        if let Ok(table) = rt.open_table(td) {
                            if let Ok(Some(val)) = table.get(&id) {
                                let bytes = val.value();
                                if let Ok(list) = bincode::deserialize::<Vec<PhraseEntry>>(&bytes) {
                                    return list.into_iter().map(|pe| pe.text).collect();
                                }
                            }
                        }
                    }
                }
            }
        }

        Vec::new()
    }

    /// Load lexicon from runtime fst + redb artifacts without materializing
    /// all phrases. Builds an apostrophe-free key map to support common
    /// joined-pinyin input.
    pub fn load_from_fst_redb<P: AsRef<std::path::Path>>(fst_path: P, redb_path: P) -> Result<Self, String> {
        let fst_path = fst_path.as_ref();
        let redb_path = redb_path.as_ref();

        // load fst
        let mut f = File::open(fst_path).map_err(|e| format!("open fst: {}", e))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| format!("read fst: {}", e))?;
        let map = Map::new(buf).map_err(|e| format!("fst map: {}", e))?;

        // open redb
        let db = Database::open(redb_path).map_err(|e| format!("open redb: {}", e))?;
        let arc_db = Arc::new(db);

        // build no_apos_map by iterating keys that contain apostrophes
        let mut stream = map.stream();
        let mut no_apos = AHashMap::new();
        while let Some((k, v)) = stream.next() {
            if let Ok(s) = std::str::from_utf8(k) {
                if s.contains('\t') {
                    let parts: Vec<&str> = s.splitn(2, '\t').collect();
                    if parts.len() == 2 {
                        let key = parts[1];
                        if key.contains('\'') {
                            let key_no = key.replace('\'', "");
                            // only insert if not present to preserve first mapping
                            no_apos.entry(key_no).or_insert(v as u64);
                        }
                    }
                }
            }
        }

        Ok(Self {
            map: AHashMap::new(),
            fst_map: Some(map),
            db: Some(arc_db),
            no_apos_map: Some(no_apos),
            metadata: LexiconMetadata::default(),
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

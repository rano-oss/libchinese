//! Zhuyin/Bopomofo engine for libzhuyin
//!
//! This engine provides the same interface as libpinyin but specialized for
//! Zhuyin/Bopomofo input method.
//!
//! This is now a thin wrapper around the generic core::Engine<ZhuyinParser>.

use std::error::Error;
use std::sync::Arc;

use crate::parser::ZhuyinParser;
use libchinese_core::{Candidate, Model, Lexicon, NGramModel, UserDict, Interpolator};

/// All standard zhuyin/bopomofo syllables.
/// This is a representative subset - full list would include all tone combinations.
pub const ZHUYIN_SYLLABLES: &[&str] = &[
    "ㄅ", "ㄅㄚ", "ㄅㄛ", "ㄅㄞ", "ㄅㄟ", "ㄅㄠ", "ㄅㄢ", "ㄅㄣ", "ㄅㄤ", "ㄅㄥ",
    "ㄅㄧ", "ㄅㄧㄝ", "ㄅㄧㄠ", "ㄅㄧㄢ", "ㄅㄧㄣ", "ㄅㄧㄥ", "ㄅㄨ",
    "ㄆ", "ㄆㄚ", "ㄆㄛ", "ㄆㄞ", "ㄆㄟ", "ㄆㄠ", "ㄆㄡ", "ㄆㄢ", "ㄆㄣ", "ㄆㄤ", "ㄆㄥ",
    "ㄆㄧ", "ㄆㄧㄝ", "ㄆㄧㄠ", "ㄆㄧㄢ", "ㄆㄧㄣ", "ㄆㄧㄥ", "ㄆㄨ",
    "ㄇ", "ㄇㄚ", "ㄇㄛ", "ㄇㄜ", "ㄇㄞ", "ㄇㄟ", "ㄇㄠ", "ㄇㄡ", "ㄇㄢ", "ㄇㄣ", "ㄇㄤ", "ㄇㄥ",
    "ㄇㄧ", "ㄇㄧㄝ", "ㄇㄧㄠ", "ㄇㄧㄡ", "ㄇㄧㄢ", "ㄇㄧㄣ", "ㄇㄧㄥ", "ㄇㄨ",
    "ㄈ", "ㄈㄚ", "ㄈㄛ", "ㄈㄟ", "ㄈㄡ", "ㄈㄢ", "ㄈㄣ", "ㄈㄤ", "ㄈㄥ", "ㄈㄨ",
    "ㄉ", "ㄉㄚ", "ㄉㄜ", "ㄉㄞ", "ㄉㄟ", "ㄉㄠ", "ㄉㄡ", "ㄉㄢ", "ㄉㄤ", "ㄉㄥ",
    "ㄉㄧ", "ㄉㄧㄝ", "ㄉㄧㄠ", "ㄉㄧㄡ", "ㄉㄧㄢ", "ㄉㄧㄥ", "ㄉㄨ", "ㄉㄨㄛ", "ㄉㄨㄟ", "ㄉㄨㄢ", "ㄉㄨㄣ", "ㄉㄨㄥ",
    "ㄊ", "ㄊㄚ", "ㄊㄜ", "ㄊㄞ", "ㄊㄠ", "ㄊㄡ", "ㄊㄢ", "ㄊㄤ", "ㄊㄥ",
    "ㄊㄧ", "ㄊㄧㄝ", "ㄊㄧㄠ", "ㄊㄧㄢ", "ㄊㄧㄥ", "ㄊㄨ", "ㄊㄨㄛ", "ㄊㄨㄟ", "ㄊㄨㄢ", "ㄊㄨㄣ", "ㄊㄨㄥ",
    "ㄋ", "ㄋㄚ", "ㄋㄜ", "ㄋㄞ", "ㄋㄟ", "ㄋㄠ", "ㄋㄡ", "ㄋㄢ", "ㄋㄣ", "ㄋㄤ", "ㄋㄥ",
    "ㄋㄧ", "ㄋㄧㄝ", "ㄋㄧㄠ", "ㄋㄧㄡ", "ㄋㄧㄢ", "ㄋㄧㄣ", "ㄋㄧㄤ", "ㄋㄧㄥ", "ㄋㄨ", "ㄋㄨㄛ", "ㄋㄨㄟ", "ㄋㄨㄢ", "ㄋㄨㄥ", "ㄋㄩ", "ㄋㄩㄝ",
    "ㄌ", "ㄌㄚ", "ㄌㄛ", "ㄌㄜ", "ㄌㄞ", "ㄌㄟ", "ㄌㄠ", "ㄌㄡ", "ㄌㄢ", "ㄌㄤ", "ㄌㄥ",
    "ㄌㄧ", "ㄌㄧㄚ", "ㄌㄧㄝ", "ㄌㄧㄠ", "ㄌㄧㄡ", "ㄌㄧㄢ", "ㄌㄧㄣ", "ㄌㄧㄤ", "ㄌㄧㄥ", "ㄌㄨ", "ㄌㄨㄛ", "ㄌㄨㄢ", "ㄌㄨㄣ", "ㄌㄨㄥ", "ㄌㄩ", "ㄌㄩㄝ",
    "ㄍ", "ㄍㄚ", "ㄍㄜ", "ㄍㄞ", "ㄍㄟ", "ㄍㄠ", "ㄍㄡ", "ㄍㄢ", "ㄍㄣ", "ㄍㄤ", "ㄍㄥ",
    "ㄍㄨ", "ㄍㄨㄚ", "ㄍㄨㄛ", "ㄍㄨㄞ", "ㄍㄨㄟ", "ㄍㄨㄢ", "ㄍㄨㄣ", "ㄍㄨㄤ", "ㄍㄨㄥ",
    "ㄎ", "ㄎㄚ", "ㄎㄜ", "ㄎㄞ", "ㄎㄠ", "ㄎㄡ", "ㄎㄢ", "ㄎㄣ", "ㄎㄤ", "ㄎㄥ",
    "ㄎㄨ", "ㄎㄨㄚ", "ㄎㄨㄛ", "ㄎㄨㄞ", "ㄎㄨㄟ", "ㄎㄨㄢ", "ㄎㄨㄣ", "ㄎㄨㄤ", "ㄎㄨㄥ",
    "ㄏ", "ㄏㄚ", "ㄏㄜ", "ㄏㄞ", "ㄏㄟ", "ㄏㄠ", "ㄏㄡ", "ㄏㄢ", "ㄏㄣ", "ㄏㄤ", "ㄏㄥ",
    "ㄏㄨ", "ㄏㄨㄚ", "ㄏㄨㄛ", "ㄏㄨㄞ", "ㄏㄨㄟ", "ㄏㄨㄢ", "ㄏㄨㄣ", "ㄏㄨㄤ", "ㄏㄨㄥ",
    "ㄐ", "ㄐㄧ", "ㄐㄧㄚ", "ㄐㄧㄝ", "ㄐㄧㄠ", "ㄐㄧㄡ", "ㄐㄧㄢ", "ㄐㄧㄣ", "ㄐㄧㄤ", "ㄐㄧㄥ",
    "ㄐㄩ", "ㄐㄩㄝ", "ㄐㄩㄢ", "ㄐㄩㄣ", "ㄐㄩㄥ",
    "ㄑ", "ㄑㄧ", "ㄑㄧㄚ", "ㄑㄧㄝ", "ㄑㄧㄠ", "ㄑㄧㄡ", "ㄑㄧㄢ", "ㄑㄧㄣ", "ㄑㄧㄤ", "ㄑㄧㄥ",
    "ㄑㄩ", "ㄑㄩㄝ", "ㄑㄩㄢ", "ㄑㄩㄣ", "ㄑㄩㄥ",
    "ㄒ", "ㄒㄧ", "ㄒㄧㄚ", "ㄒㄧㄝ", "ㄒㄧㄠ", "ㄒㄧㄡ", "ㄒㄧㄢ", "ㄒㄧㄣ", "ㄒㄧㄤ", "ㄒㄧㄥ",
    "ㄒㄩ", "ㄒㄩㄝ", "ㄒㄩㄢ", "ㄒㄩㄣ", "ㄒㄩㄥ",
    "ㄓ", "ㄓㄚ", "ㄓㄜ", "ㄓㄞ", "ㄓㄟ", "ㄓㄠ", "ㄓㄡ", "ㄓㄢ", "ㄓㄣ", "ㄓㄤ", "ㄓㄥ",
    "ㄓㄨ", "ㄓㄨㄚ", "ㄓㄨㄛ", "ㄓㄨㄞ", "ㄓㄨㄟ", "ㄓㄨㄢ", "ㄓㄨㄣ", "ㄓㄨㄤ", "ㄓㄨㄥ",
    "ㄔ", "ㄔㄚ", "ㄔㄜ", "ㄔㄞ", "ㄔㄠ", "ㄔㄡ", "ㄔㄢ", "ㄔㄣ", "ㄔㄤ", "ㄔㄥ",
    "ㄔㄨ", "ㄔㄨㄚ", "ㄔㄨㄛ", "ㄔㄨㄞ", "ㄔㄨㄟ", "ㄔㄨㄢ", "ㄔㄨㄣ", "ㄔㄨㄤ", "ㄔㄨㄥ",
    "ㄕ", "ㄕㄚ", "ㄕㄜ", "ㄕㄞ", "ㄕㄟ", "ㄕㄠ", "ㄕㄡ", "ㄕㄢ", "ㄕㄣ", "ㄕㄤ", "ㄕㄥ",
    "ㄕㄨ", "ㄕㄨㄚ", "ㄕㄨㄛ", "ㄕㄨㄞ", "ㄕㄨㄟ", "ㄕㄨㄢ", "ㄕㄨㄣ", "ㄕㄨㄤ", "ㄕㄨㄥ",
    "ㄖ", "ㄖㄜ", "ㄖㄠ", "ㄖㄡ", "ㄖㄢ", "ㄖㄣ", "ㄖㄤ", "ㄖㄥ",
    "ㄖㄨ", "ㄖㄨㄛ", "ㄖㄨㄞ", "ㄖㄨㄟ", "ㄖㄨㄢ", "ㄖㄨㄣ", "ㄖㄨㄥ",
    "ㄗ", "ㄗㄚ", "ㄗㄜ", "ㄗㄞ", "ㄗㄟ", "ㄗㄠ", "ㄗㄡ", "ㄗㄢ", "ㄗㄣ", "ㄗㄤ", "ㄗㄥ",
    "ㄗㄨ", "ㄗㄨㄛ", "ㄗㄨㄟ", "ㄗㄨㄢ", "ㄗㄨㄣ", "ㄗㄨㄥ",
    "ㄘ", "ㄘㄚ", "ㄘㄜ", "ㄘㄞ", "ㄘㄠ", "ㄘㄡ", "ㄘㄢ", "ㄘㄣ", "ㄘㄤ", "ㄘㄥ",
    "ㄘㄨ", "ㄘㄨㄛ", "ㄘㄨㄟ", "ㄘㄨㄢ", "ㄘㄨㄣ", "ㄘㄨㄥ",
    "ㄙ", "ㄙㄚ", "ㄙㄜ", "ㄙㄞ", "ㄙㄠ", "ㄙㄡ", "ㄙㄢ", "ㄙㄣ", "ㄙㄤ", "ㄙㄥ",
    "ㄙㄨ", "ㄙㄨㄛ", "ㄙㄨㄟ", "ㄙㄨㄢ", "ㄙㄨㄣ", "ㄙㄨㄥ",
    "ㄚ", "ㄛ", "ㄜ", "ㄞ", "ㄟ", "ㄠ", "ㄡ", "ㄢ", "ㄣ", "ㄤ", "ㄥ", "ㄦ",
    "ㄧ", "ㄧㄚ", "ㄧㄛ", "ㄧㄝ", "ㄧㄞ", "ㄧㄠ", "ㄧㄡ", "ㄧㄢ", "ㄧㄣ", "ㄧㄤ", "ㄧㄥ",
    "ㄨ", "ㄨㄚ", "ㄨㄛ", "ㄨㄞ", "ㄨㄟ", "ㄨㄢ", "ㄨㄣ", "ㄨㄤ", "ㄨㄥ",
    "ㄩ", "ㄩㄝ", "ㄩㄢ", "ㄩㄣ", "ㄩㄥ",
];

/// Public engine for libzhuyin.
///
/// This wraps the generic core::Engine<ZhuyinParser> with zhuyin-specific loading logic.
/// All actual IME logic (segmentation, fuzzy matching, caching, scoring) is in core.
/// 
/// The inner engine is wrapped in Arc to allow cheap cloning for sharing across editors.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<libchinese_core::Engine<ZhuyinParser>>,
}

impl Engine {
    /// Construct an Engine from a pre-built Model.
    ///
    /// Uses standard zhuyin fuzzy rules configured in the parser.
    /// Parser is created internally with standard bopomofo syllables.
    pub fn new(model: Model) -> Self {
        let fuzzy_rules = crate::standard_fuzzy_rules();
        let parser = ZhuyinParser::new(fuzzy_rules, ZHUYIN_SYLLABLES);
        Self {
            inner: Arc::new(libchinese_core::Engine::new(model, parser)),
        }
    }
    
    /// Get a cloned Arc to the inner core engine.
    /// 
    /// Useful for sharing the engine with ImeEngine and other components.
    pub fn inner_arc(&self) -> Arc<libchinese_core::Engine<ZhuyinParser>> {
        Arc::clone(&self.inner)
    }

    /// Load an engine from a model directory containing runtime artifacts.
    ///
    /// Expected layout (data-dir):
    ///  - lexicon.fst + lexicon.bincode    (lexicon for zhuyin)
    ///  - ngram.bincode                     (serialized NGramModel)
    ///  - lambdas.fst + lambdas.bincode    (interpolator for zhuyin)
    ///  - userdict.redb                     (persistent user dictionary)
    pub fn from_data_dir<P: AsRef<std::path::Path>>(data_dir: P) -> Result<Self, Box<dyn Error>> {
        let data_dir = data_dir.as_ref();

        // Load lexicon from fst + bincode (required)
        let fst_path = data_dir.join("lexicon.fst");
        let bincode_path = data_dir.join("lexicon.bincode");

        let lex = Lexicon::load_from_fst_bincode(&fst_path, &bincode_path)
            .map_err(|e| format!("failed to load lexicon from {:?} and {:?}: {}", fst_path, bincode_path, e))?;

        // Load interpolator first (required for NGramModel)
        let fst_path = data_dir.join("lambdas.fst");
        let bincode_path = data_dir.join("lambdas.bincode");
        let interp = Interpolator::load(&fst_path, &bincode_path)?;

        // Load ngram model if present
        let ngram = {
            let ng_path = data_dir.join("ngram.bincode");
            let mut ng = if ng_path.exists() {
                match NGramModel::load_bincode(&ng_path) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("warning: failed to load ngram.bincode: {}, using empty model", e);
                        NGramModel::new()
                    }
                }
            } else {
                eprintln!("warning: ngram.bincode not found, using empty model");
                NGramModel::new()
            };
            ng.set_interpolator(interp);
            ng
        };

        // Userdict: use persistent userdict at ~/.zhuyin/userdict.redb
        let userdict = {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".to_string());
            let ud_path = std::path::PathBuf::from(home)
                .join(".zhuyin")
                .join("userdict.redb");
            
            // Create directory if needed
            if let Some(parent) = ud_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            UserDict::new(&ud_path)?
        };

        let model = Model::new(lex, ngram, userdict, libchinese_core::Config::default());

        // Parser is created internally using ZHUYIN_SYLLABLES
        Ok(Self::new(model))
    }

    /// Get cache statistics (hits, misses, hit rate)
    pub fn cache_stats(&self) -> (usize, usize, f64) {
        let (hits, misses) = self.inner.cache_stats();
        let total = hits + misses;
        let hit_rate = if total > 0 { hits as f64 / total as f64 } else { 0.0 };
        (hits, misses, hit_rate)
    }
    
    /// Get current cache size
    pub fn cache_size(&self) -> usize {
        self.inner.cache_size()
    }

    /// Clear the cache
    pub fn clear_cache(&self) {
        self.inner.clear_cache();
    }
    
    /// Get a reference to the ngram model
    pub fn ngram(&self) -> &NGramModel {
        self.inner.ngram()
    }
    
    /// Get a reference to the user dictionary
    pub fn userdict(&self) -> &UserDict {
        self.inner.userdict()
    }
    
    /// Get a reference to the config
    pub fn config(&self) -> &libchinese_core::Config {
        self.inner.config()
    }

    /// Commit a phrase to the user dictionary (learning).
    ///
    /// This increases the frequency/score for the given phrase, allowing the
    /// IME to learn user preferences over time.
    ///
    /// # Example
    /// ```no_run
    /// # use libzhuyin::Engine;
    /// # let mut engine = Engine::from_data_dir("data").unwrap();
    /// let candidates = engine.input("ㄋㄧㄏㄠ");
    /// if let Some(selected) = candidates.first() {
    ///     engine.commit(&selected.text);
    /// }
    /// ```
    pub fn commit(&self, phrase: &str) {
        self.inner.commit(phrase);
    }

    /// Main input API. Returns ranked `Candidate` items for the given raw zhuyin input.
    ///
    /// Delegates to core::Engine which handles:
    /// 1. Parser segmentation into syllable sequences
    /// 2. Fuzzy key generation for all alternatives (NOW WORKING!)
    /// 3. Lexicon lookups and n-gram scoring
    /// 4. Penalty application for fuzzy matches (NOW WORKING!)
    /// 5. Result caching (NOW AVAILABLE!)
    pub fn input(&self, input: &str) -> Vec<Candidate> {
        self.inner.input(input)
    }
}

/// Create an IME engine with HSU keyboard layout fuzzy rules.
///
/// HSU layout is optimized for efficiency with finals on the home row.
/// Common typing errors: ㄓ/ㄐ (j key), ㄔ/ㄑ (q key), ㄕ/ㄒ (x key).
///
/// # Arguments
/// * `data_dir` - Path to directory containing zhuyin data files
/// * `page_size` - Number of candidates to show per page (typically 5-9)
///
/// # Returns
/// `ImeEngine<ZhuyinParser>` configured with HSU fuzzy rules
///
/// # Example
/// ```no_run
/// use libzhuyin::create_ime_engine_hsu;
/// 
/// let ime = create_ime_engine_hsu("data/zhuyin", 9).unwrap();
/// // Now ready to process bopomofo input
/// ```
pub fn create_ime_engine_hsu<P: AsRef<std::path::Path>>(
    data_dir: P,
    page_size: usize,
) -> Result<libchinese_core::ime::ImeEngine<ZhuyinParser>, Box<dyn Error>> {
    let data_dir = data_dir.as_ref();
    
    // Load model from data directory
    let fst_path = data_dir.join("zhuyin.fst");
    let bincode_path = data_dir.join("zhuyin.redb");
    let lex = Lexicon::load_from_fst_bincode(&fst_path, &bincode_path)?;
    
    // Load interpolator
    let fst_path = data_dir.join("zhuyin.lambdas.fst");
    let bincode_path = data_dir.join("zhuyin.lambdas.redb");
    let interp = Interpolator::load(&fst_path, &bincode_path)?;
    
    // Load ngram
    let ng_path = data_dir.join("ngram.bincode");
    let mut ngram = if ng_path.exists() {
        NGramModel::load_bincode(&ng_path)?
    } else {
        NGramModel::new()
    };
    ngram.set_interpolator(interp);
    
    // User dictionary
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let ud_path = std::path::PathBuf::from(home)
        .join(".zhuyin")
        .join("userdict.redb");
    if let Some(parent) = ud_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let userdict = UserDict::new(&ud_path)?;
    
    // Create model with default config
    let model = Model::new(lex, ngram, userdict, libchinese_core::Config::default());
    
    // Create parser with HSU fuzzy rules
    let fuzzy_rules = crate::fuzzy_presets::hsu_fuzzy_rules();
    let parser = ZhuyinParser::new(fuzzy_rules, ZHUYIN_SYLLABLES);
    
    // Create core engine
    let core_engine = Arc::new(libchinese_core::Engine::new(model, parser));
    
    // Create IME engine
    Ok(libchinese_core::ime::ImeEngine::from_arc_with_page_size(core_engine, page_size))
}

/// Create an IME engine with Standard keyboard layout fuzzy rules.
///
/// Standard layout is the most common zhuyin keyboard layout.
/// Common typing errors: nasal finals ㄢ/ㄤ, ㄣ/ㄥ, ㄧㄣ/ㄧㄥ.
///
/// # Arguments
/// * `data_dir` - Path to directory containing zhuyin data files
/// * `page_size` - Number of candidates to show per page (typically 5-9)
///
/// # Returns
/// `ImeEngine<ZhuyinParser>` configured with Standard fuzzy rules
pub fn create_ime_engine_standard<P: AsRef<std::path::Path>>(
    data_dir: P,
    page_size: usize,
) -> Result<libchinese_core::ime::ImeEngine<ZhuyinParser>, Box<dyn Error>> {
    let data_dir = data_dir.as_ref();
    
    // Load model (same as HSU)
    let fst_path = data_dir.join("zhuyin.fst");
    let bincode_path = data_dir.join("zhuyin.redb");
    let lex = Lexicon::load_from_fst_bincode(&fst_path, &bincode_path)?;
    
    let fst_path = data_dir.join("zhuyin.lambdas.fst");
    let bincode_path = data_dir.join("zhuyin.lambdas.redb");
    let interp = Interpolator::load(&fst_path, &bincode_path)?;
    
    let ng_path = data_dir.join("ngram.bincode");
    let mut ngram = if ng_path.exists() {
        NGramModel::load_bincode(&ng_path)?
    } else {
        NGramModel::new()
    };
    ngram.set_interpolator(interp);
    
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let ud_path = std::path::PathBuf::from(home)
        .join(".zhuyin")
        .join("userdict.redb");
    if let Some(parent) = ud_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let userdict = UserDict::new(&ud_path)?;
    
    let model = Model::new(lex, ngram, userdict, libchinese_core::Config::default());
    
    // Create parser with Standard fuzzy rules
    let fuzzy_rules = crate::fuzzy_presets::standard_fuzzy_rules();
    let parser = ZhuyinParser::new(fuzzy_rules, ZHUYIN_SYLLABLES);
    
    let core_engine = Arc::new(libchinese_core::Engine::new(model, parser));
    
    Ok(libchinese_core::ime::ImeEngine::from_arc_with_page_size(core_engine, page_size))
}

/// Create an IME engine with ETEN keyboard layout fuzzy rules.
///
/// ETEN layout is used in ETEN Chinese System (倚天中文系統).
/// Similar error patterns to Standard layout.
///
/// # Arguments
/// * `data_dir` - Path to directory containing zhuyin data files
/// * `page_size` - Number of candidates to show per page (typically 5-9)
///
/// # Returns
/// `ImeEngine<ZhuyinParser>` configured with ETEN fuzzy rules
pub fn create_ime_engine_eten<P: AsRef<std::path::Path>>(
    data_dir: P,
    page_size: usize,
) -> Result<libchinese_core::ime::ImeEngine<ZhuyinParser>, Box<dyn Error>> {
    let data_dir = data_dir.as_ref();
    
    // Load model (same as HSU/Standard)
    let fst_path = data_dir.join("zhuyin.fst");
    let bincode_path = data_dir.join("zhuyin.redb");
    let lex = Lexicon::load_from_fst_bincode(&fst_path, &bincode_path)?;
    
    let fst_path = data_dir.join("zhuyin.lambdas.fst");
    let bincode_path = data_dir.join("zhuyin.lambdas.redb");
    let interp = Interpolator::load(&fst_path, &bincode_path)?;
    
    let ng_path = data_dir.join("ngram.bincode");
    let mut ngram = if ng_path.exists() {
        NGramModel::load_bincode(&ng_path)?
    } else {
        NGramModel::new()
    };
    ngram.set_interpolator(interp);
    
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let ud_path = std::path::PathBuf::from(home)
        .join(".zhuyin")
        .join("userdict.redb");
    if let Some(parent) = ud_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let userdict = UserDict::new(&ud_path)?;
    
    let model = Model::new(lex, ngram, userdict, libchinese_core::Config::default());
    
    // Create parser with ETEN fuzzy rules
    let fuzzy_rules = crate::fuzzy_presets::eten_fuzzy_rules();
    let parser = ZhuyinParser::new(fuzzy_rules, ZHUYIN_SYLLABLES);
    
    let core_engine = Arc::new(libchinese_core::Engine::new(model, parser));
    
    Ok(libchinese_core::ime::ImeEngine::from_arc_with_page_size(core_engine, page_size))
}

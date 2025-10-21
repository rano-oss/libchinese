//! Zhuyin/Bopomofo engine for libzhuyin
//!
//! This engine provides the same interface as libpinyin but specialized for
//! Zhuyin/Bopomofo input method.
//!
//! This is now a thin wrapper around the generic core::Engine<ZhuyinParser>.

use std::error::Error;

use crate::parser::ZhuyinParser;
use libchinese_core::{Candidate, Model, Lexicon, NGramModel, UserDict, Interpolator};

/// Public engine for libzhuyin
pub struct Engine {
    inner: libchinese_core::Engine<ZhuyinParser>,
}

impl Engine {
    /// Construct an Engine from a pre-built Model and ZhuyinParser.
    ///
    /// Uses standard zhuyin fuzzy rules by default.
    pub fn new(model: Model, parser: ZhuyinParser) -> Self {
        let rules = crate::standard_fuzzy_rules();
        Self {
            inner: libchinese_core::Engine::new(model, parser, rules),
        }
    }
    
    /// Construct an Engine with custom fuzzy rules.
    pub fn with_fuzzy_rules(model: Model, parser: ZhuyinParser, fuzzy_rules: Vec<String>) -> Self {
        Self {
            inner: libchinese_core::Engine::new(model, parser, fuzzy_rules),
        }
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

        // Load ngram model if present
        let ngram = {
            let ng_path = data_dir.join("ngram.bincode");
            if ng_path.exists() {
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
            }
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

        // Load interpolator if present
        let interp = {
            let fst_path = data_dir.join("lambdas.fst");
            let bincode_path = data_dir.join("lambdas.bincode");
            if fst_path.exists() && bincode_path.exists() {
                match Interpolator::load(&fst_path, &bincode_path) {
                    Ok(i) => i,
                    Err(e) => {
                        eprintln!("warning: failed to load interpolator: {}, using new", e);
                        Interpolator::new()
                    }
                }
            } else {
                Interpolator::new()
            }
        };

        let model = Model::new(lex, ngram, userdict, libchinese_core::Config::default(), interp);

        // Load parser with syllables from data/zhuyin_syllables.txt
        let parser = {
            let syllables_path = std::path::Path::new("data/zhuyin_syllables.txt");
            if syllables_path.exists() {
                match std::fs::read_to_string(syllables_path) {
                    Ok(content) => {
                        let syllables: Vec<&str> = content.lines()
                            .map(|s| s.trim())
                            .filter(|s| !s.is_empty())
                            .collect();
                        eprintln!("✓ Loaded {} zhuyin syllables", syllables.len());
                        ZhuyinParser::with_syllables(&syllables)
                    }
                    Err(e) => {
                        eprintln!("warning: failed to load zhuyin_syllables.txt: {}", e);
                        eprintln!("using fallback syllable list");
                        ZhuyinParser::with_syllables(&[
                            "ㄋㄧ", "ㄏㄠ", "ㄓㄨㄥ", "ㄍㄨㄛ"
                        ])
                    }
                }
            } else {
                eprintln!("warning: zhuyin_syllables.txt not found at {:?}", syllables_path);
                eprintln!("using fallback syllable list");
                ZhuyinParser::with_syllables(&[
                    "ㄋㄧ", "ㄏㄠ", "ㄓㄨㄥ", "ㄍㄨㄛ"
                ])
            }
        };

        Ok(Self::new(model, parser))
    }

    /// Get cache statistics (hits, misses, hit rate)
    pub fn cache_stats(&self) -> (usize, usize, f64) {
        let (hits, misses) = self.inner.cache_stats();
        let total = hits + misses;
        let hit_rate = if total > 0 { hits as f64 / total as f64 } else { 0.0 };
        (hits, misses, hit_rate)
    }

    /// Clear the cache
    pub fn clear_cache(&mut self) {
        self.inner.clear_cache();
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

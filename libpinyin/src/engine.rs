//! Pinyin input method engine
//!
//! Provides a high-level Engine that combines parser, model, and fuzzy matching
//! into a simple `input(text) -> Vec<Candidate>` API with caching optimization.
//!
//! This is now a thin wrapper around the generic core::Engine<Parser>.

use std::error::Error;

use crate::parser::Parser;
use libchinese_core::{Candidate, Interpolator, Model, Lexicon, NGramModel, UserDict};

/// Public engine for libpinyin.
///
/// This wraps the generic core::Engine<Parser> with pinyin-specific loading logic.
/// All actual IME logic (segmentation, fuzzy matching, caching, scoring) is in core.
pub struct Engine {
    inner: libchinese_core::Engine<Parser>,
}

impl Engine {
    /// Construct an Engine from a pre-built `Model` and a `Parser`.
    ///
    /// Uses standard pinyin fuzzy rules by default.
    pub fn new(model: Model, parser: Parser) -> Self {
        let rules = crate::standard_fuzzy_rules();
        Self {
            inner: libchinese_core::Engine::new(model, parser, rules),
        }
    }

    /// Load an engine from a model directory containing runtime artifacts.
    ///
    /// Expected layout (data-dir):
    ///  - lexicon.fst + lexicon.bincode    (lexicon)
    ///  - ngram.bincode                    (serialized NGramModel)
    ///  - lambdas.fst + lambdas.bincode   (interpolator)
    ///  - userdict.redb                    (persistent user dictionary)
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
                NGramModel::new()
            }
        };

        // Userdict: use persistent userdict at ~/.pinyin/userdict.redb
        let userdict = {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".to_string());
            let ud_path = std::path::PathBuf::from(home)
                .join(".pinyin")
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

        // Load parser with syllables
        let parser = {
            let syllables_path = std::path::Path::new("data/pinyin_syllables.txt");
            if syllables_path.exists() {
                match std::fs::read_to_string(syllables_path) {
                    Ok(content) => {
                        let syllables: Vec<&str> = content.lines()
                            .map(|s| s.trim())
                            .filter(|s| !s.is_empty())
                            .collect();
                        eprintln!("✓ Loaded {} pinyin syllables", syllables.len());
                        Parser::with_syllables(&syllables)
                    }
                    Err(e) => {
                        eprintln!("warning: failed to load pinyin_syllables.txt: {}", e);
                        eprintln!("using fallback syllable list");
                        Parser::with_syllables(&[
                            "a", "ai", "an", "ang", "ao", 
                            "ba", "bai", "ban", "bang", "bao",
                            "ni", "hao", "wo", "shi", "zhong", "guo"
                        ])
                    }
                }
            } else {
                eprintln!("warning: pinyin_syllables.txt not found at {:?}", syllables_path);
                eprintln!("using fallback syllable list");
                Parser::with_syllables(&[
                    "a", "ai", "an", "ang", "ao",
                    "ba", "bai", "ban", "bang", "bao",
                    "ni", "hao", "wo", "shi", "zhong", "guo"
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

    /// Get cache size (number of cached entries)
    pub fn cache_size(&self) -> usize {
        // Note: the core engine doesn't expose cache size directly
        // This is an approximation based on cache stats
        let (hits, misses) = self.inner.cache_stats();
        // Rough estimate: total queries is an upper bound on cache size
        hits + misses
    }

    /// Clear the cache
    pub fn clear_cache(&mut self) {
        self.inner.clear_cache();
    }

    /// Commit a phrase to the user dictionary (learning).
    ///
    /// This increases the frequency/score for the given phrase.
    pub fn commit(&mut self, _phrase: &str) {
        // TODO: Implement user dictionary learning
        // For now, this is a no-op as the core engine doesn't expose
        // model mutation methods yet
    }

    /// Main input API. Returns ranked `Candidate` items for the given raw input.
    ///
    /// Delegates to core::Engine which handles:
    /// 1. Parser segmentation into syllable sequences
    /// 2. Fuzzy key generation for all alternatives
    /// 3. Lexicon lookups and n-gram scoring
    /// 4. Penalty application for fuzzy matches
    /// 5. Result caching
    pub fn input(&self, input: &str) -> Vec<Candidate> {
        self.inner.input(input)
    }
}

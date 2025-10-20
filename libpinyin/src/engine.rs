//! Pinyin input method engine
//!
//! Provides a high-level Engine that combines parser, model, and fuzzy matching
//! into a simple `input(text) -> Vec<Candidate>` API with caching optimization.

use std::collections::HashMap;
use std::error::Error;
use std::cell::RefCell;

use crate::parser::Parser;
use crate::parser::Syllable;
use crate::fuzzy::FuzzyMap;
use libchinese_core::{Candidate, Interpolator, Model, Lexicon, NGramModel, UserDict};

/// Public engine for libpinyin.
///
/// The Engine composes:
///  - a `Parser` (language-specific) for segmentation,
///  - a shared `Model` (lexicon + ngram + userdict + config),
///  - a `FuzzyMap` for phonetic alternates,
///  - helpers for table/model loading (see `tables` module).
///
/// Typical usage:
///  let engine = Engine::new(model, parser);
///  let candidates = engine.input(\"nihao\");
///
pub struct Engine {
    model: Model,
    parser: Parser,
    fuzzy: FuzzyMap,
    /// Maximum candidates to return
    limit: usize,
    /// Cache for input -> candidates mapping
    cache: RefCell<HashMap<String, Vec<Candidate>>>,
    /// Cache statistics
    cache_hits: RefCell<usize>,
    cache_misses: RefCell<usize>,
}

impl Engine {
    /// Construct an Engine from a pre-built `Model` and a `Parser`.
    ///
    /// If the model's config has no fuzzy rules, standard fuzzy rules are used by default.
    pub fn new(model: Model, parser: Parser) -> Self {
        // Always use standard fuzzy rules which include all upstream rules
        // (shengmu, yunmu, corrections, and composed syllables)
        let fuzzy = FuzzyMap::with_standard_rules();
        
        Self {
            model,
            parser,
            fuzzy,
            limit: 8,
            cache: RefCell::new(HashMap::new()),
            cache_hits: RefCell::new(0),
            cache_misses: RefCell::new(0),
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
                match std::fs::read(&ng_path).ok().and_then(|b| bincode::deserialize::<NGramModel>(&b).ok()) {
                    Some(m) => m,
                    None => {
                        eprintln!("warning: failed to deserialize ngram.bincode, using empty model");
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
            
            match UserDict::new(&ud_path) {
                Ok(u) => u,
                Err(e) => {
                    eprintln!("warning: failed to open userdict at {:?}: {}", ud_path, e);
                    // Fallback to temp userdict
                    let temp_path = std::env::temp_dir().join(format!(
                        "libpinyin_userdict_{}.redb",
                        std::process::id()
                    ));
                    UserDict::new(&temp_path).expect("failed to create temp userdict")
                }
            }
        };

        // Load interpolator or create empty one
        let interp = {
            let lf = data_dir.join("lambdas.fst");
            let lb = data_dir.join("lambdas.bincode");
            if lf.exists() && lb.exists() {
                match libchinese_core::Interpolator::load(&lf, &lb) {
                    Ok(i) => i,
                    Err(e) => {
                        eprintln!("warning: failed to load interpolator: {}", e);
                        Interpolator::new()
                    }
                }
            } else {
                Interpolator::new()
            }
        };

        let cfg = libchinese_core::Config::default();

        let model = Model::new(lex, ngram, userdict, cfg, interp);

        // Load pinyin syllables from data/pinyin_syllables.txt
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

    /// Set candidate limit (fluent API).
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize, f64) {
        let hits = *self.cache_hits.borrow();
        let misses = *self.cache_misses.borrow();
        let total = hits + misses;
        let hit_rate = if total > 0 { hits as f64 / total as f64 } else { 0.0 };
        (hits, misses, hit_rate)
    }

    /// Clear the cache
    pub fn clear_cache(&mut self) {
        self.cache.borrow_mut().clear();
        *self.cache_hits.borrow_mut() = 0;
        *self.cache_misses.borrow_mut() = 0;
    }

    /// Get cache size
    pub fn cache_size(&self) -> usize {
        self.cache.borrow().len()
    }

    /// Main input API. Returns ranked `Candidate` items for the given raw input.
    ///
    /// Flow:
    /// 1. Use `parser` to produce top-k segmentations (token sequences).
    /// 2. For each segmentation, generate a canonical key (join syllables).
    /// 3. For fuzzy segmentations, generate alternative keys using syllable-level fuzzy matching.
    /// 4. Query `model.lexicon` for phrase candidates for each key.
    /// 5. Score candidates via `model.ngram` and boost with `model.userdict`.
    /// 6. Merge duplicates from multiple segmentations and return top `limit`.
    pub fn input(&self, input: &str) -> Vec<Candidate> {
        // Check cache first
        if let Some(cached) = self.cache.borrow().get(input) {
            *self.cache_hits.borrow_mut() += 1;
            return cached.clone();
        }

        *self.cache_misses.borrow_mut() += 1;

        // Get top segmentations (k best). Parser returns Vec<Vec<Syllable>>
        let segs = self.parser.segment_top_k(input, 4, true);

        // Map from phrase -> best Candidate (keep highest score)
        let mut best: HashMap<String, Candidate> = HashMap::new();

        for seg in segs.into_iter() {
            // Convert segmentation into a canonical key.
            // Convention: join syllable texts with no separator (e.g. "ni" + "hao" -> "nihao").
            let _key = Self::segmentation_to_key(&seg);

            // Generate fuzzy alternative keys for all segmentations
            // This allows "zi" to match both "zi" and "zhi" candidates
            let keys_to_try = self.generate_fuzzy_key_alternatives_from_segmentation(&seg);

            // Try all alternative keys (exact + fuzzy alternatives)
            let mut candidates = Vec::new();
            for (i, alt_key) in keys_to_try.iter().enumerate() {
                let mut key_candidates = self.model.candidates_for_key(alt_key, self.limit);
                
                // Apply fuzzy penalty if this is not the original key (index 0 is always original)
                if i > 0 {
                    let penalty = self.fuzzy.default_penalty();
                    for c in key_candidates.iter_mut() {
                        c.score -= penalty;
                    }
                }
                
                candidates.append(&mut key_candidates);
            }

            // If this segmentation used parser-level fuzzy matches, apply additional penalty
            let used_parser_fuzzy = seg.iter().any(|s| s.fuzzy);
            if used_parser_fuzzy {
                let penalty = self.fuzzy.default_penalty();
                for c in candidates.iter_mut() {
                    c.score -= penalty;
                }
            }

            // Merge candidate: keep the best score seen for this exact phrase
            for cand in candidates.into_iter() {
                match best.get(&cand.text) {
                    Some(existing) if existing.score >= cand.score => {}
                    _ => {
                        best.insert(cand.text.clone(), cand.clone());
                    }
                }
            }
        }

        // Collect, sort and return top results
        let mut vec: Vec<Candidate> = best.into_values().collect();
        vec.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if vec.len() > self.limit {
            vec.truncate(self.limit);
        }

        // Cache the result
        let cache_size_limit = 1000; // Limit cache size to prevent memory issues
        let mut cache = self.cache.borrow_mut();
        if cache.len() >= cache_size_limit {
            // Simple eviction: clear cache when it gets too large
            cache.clear();
        }
        cache.insert(input.to_string(), vec.clone());

        vec
    }

    /// Commit a phrase selection (user accepted phrase) to the user dictionary.
    /// This increments user learning counts so future queries are biased.
    pub fn commit(&mut self, phrase: &str) {
        // Persist learning to runtime userdict
        self.model.userdict.learn(phrase);
        // Invalidate cache so subsequent input() calls see updated userdict boosts.
        // Simple approach: clear the whole cache on commit. This is acceptable for
        // tests and small workloads; can be optimized to selective invalidation later.
        self.clear_cache();
    }

    /// Persist the in-memory userdict to a redb file at `path`.
    ///
    /// This writes a simple u64 frequency table matching `core::userdict` layout.
    pub fn persist_userdict<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), Box<dyn Error>> {
        let map = self.model.userdict.snapshot();
        let ud = UserDict::new(path).map_err(|e| format!("open userdict redb: {}", e))?;
        for (k, v) in map.into_iter() {
            // write frequencies directly
            ud.learn_with_count(&k, v).map_err(|e| format!("write userdict: {}", e))?;
        }
        Ok(())
    }

    /// Generate fuzzy alternative keys from a segmentation by considering fuzzy alternatives
    /// for each syllable in the segmentation. This provides more precise fuzzy matching.
    ///
    /// Always returns at least the original key. The original key is always first.
    fn generate_fuzzy_key_alternatives_from_segmentation(&self, segmentation: &[Syllable]) -> Vec<String> {
        let mut alternatives = Vec::new();
        
        // Generate all combinations including the original
        self.generate_combinations_recursive(segmentation, 0, String::new(), &mut alternatives);
        
        // Ensure original key is first (for penalty calculation)
        let original_key = Self::segmentation_to_key(segmentation);
        let mut unique_alternatives = vec![original_key.clone()];
        
        // Add other alternatives (deduplicated)
        for alt in alternatives {
            if alt != original_key && !unique_alternatives.contains(&alt) {
                unique_alternatives.push(alt);
            }
        }
        
        unique_alternatives
    }
    
    /// Recursively generate all combinations of syllable alternatives
    fn generate_combinations_recursive(
        &self, 
        segmentation: &[Syllable], 
        position: usize, 
        current: String, 
        results: &mut Vec<String>
    ) {
        if position >= segmentation.len() {
            if !current.is_empty() {
                results.push(current);
            }
            return;
        }
        
        let syllable = &segmentation[position];
        
        // Get alternatives for this syllable - always use fuzzy map to get all alternatives
        let alternatives = self.fuzzy.alternative_strings(&syllable.text);
        
        // For each alternative, recurse to the next position
        for alt in alternatives {
            let new_current = format!("{}{}", current, alt);
            self.generate_combinations_recursive(segmentation, position + 1, new_current, results);
        }
    }

    /// Helper: convert a segmentation (Vec<Syllable>) into a canonical lookup key.
    /// Convert a syllable segmentation to an FST lookup key.
    /// 
    /// Joins syllables with apostrophes to match the format in the lexicon FST.
    /// For example: ["ni", "hao"] -> "ni'hao"
    fn segmentation_to_key(seg: &[Syllable]) -> String {
        seg.iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<&str>>()
            .join("'")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libchinese_core::{Config, Lexicon, Model, NGramModel, UserDict};

    #[test]
    fn engine_smoke_end_to_end() {
        // Build a tiny model for smoke testing
        let mut lex = Lexicon::new();
        // core::Lexicon::insert currently accepts (key, phrase) pairs only.
        // Phase‑2 production lexicons may include frequencies in a separate step.
        lex.insert("nihao", "你好");
        lex.insert("nihao", "你号");
        let mut ng = NGramModel::new();
        ng.insert_unigram("你", -1.0);
        ng.insert_unigram("好", -1.0);
        let temp_path = std::env::temp_dir().join(format!(
            "libpinyin_test_userdict_{}.redb",
            std::process::id()
        ));
        let user = UserDict::new(&temp_path).expect("create test userdict");
        let cfg = Config::default();
        let model = Model::new(lex, ng, user, cfg, Interpolator::new());

        // Create parser seeded with syllables
        let parser = crate::parser::Parser::with_syllables(&["ni", "hao"]);

        let engine = Engine::new(model, parser);
        let cands = engine.input("nihao");
        // Expect at least one candidate and that top candidate is "你好"
        assert!(!cands.is_empty());
        assert_eq!(cands[0].text, "你好");
    }
}

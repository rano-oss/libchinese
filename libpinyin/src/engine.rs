/*
libchinese/libpinyin/src/engine.rs

Engine and supporting skeletons for libpinyin.

Responsibilities:
- Provide a high-level `Engine` that wires parser + core model + fuzzy + tables
  into a simple `input(&str) -> Vec<Candidate>` API.
- Provide lightweight skeleton modules for `fuzzy` and `tables` inside this file
  so the crate has a clear place to continue implementing language-specific
  functionality (phase 2 -> phase 5).
*/

use std::collections::HashMap;
use std::path::Path;
use std::error::Error;
use std::cell::RefCell;

use crate::parser::Parser;
use crate::parser::Syllable;
use libchinese_core::{Candidate, Model, Lexicon, NGramModel, UserDict, Interpolator};

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
    fuzzy: fuzzy::FuzzyMap,
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
    pub fn new(model: Model, parser: Parser) -> Self {
        let fuzzy = fuzzy::FuzzyMap::from_config(&model.config);
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
    ///  - pinyin.fst + pinyin.redb         (lexicon)
    ///  - ngram.bincode                    (serialized NGramModel)
    ///  - pinyin.lambdas.fst + .redb       (optional interpolator)
    ///  - userdict.redb                     (optional persistent user dictionary)
    pub fn from_data_dir<P: AsRef<std::path::Path>>(data_dir: P) -> Result<Self, Box<dyn Error>> {
        let data_dir = data_dir.as_ref();

        // Attempt to load lexicon from fst + redb (on-demand lookup). Fallback to demo lexicon.
        let fst_path = data_dir.join("pinyin.fst");
        let redb_path = data_dir.join("pinyin.redb");

        let lex = if fst_path.exists() && redb_path.exists() {
            match Lexicon::load_from_fst_redb(&fst_path, &redb_path) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("warning: failed to load lexicon from artifacts: {}", e);
                    Lexicon::load_demo()
                }
            }
        } else {
            Lexicon::load_demo()
        };

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

        // Userdict: prefer persistent userdict.redb if present
        let userdict = {
            let ud_path = data_dir.join("userdict.redb");
            if ud_path.exists() {
                match UserDict::new_redb(&ud_path) {
                    Ok(u) => u,
                    Err(e) => {
                        eprintln!("warning: failed to open userdict.redb: {} — using ephemeral userdict", e);
                        UserDict::new()
                    }
                }
            } else {
                UserDict::new()
            }
        };

        // Optional interpolator
        let interp = {
            let lf = data_dir.join("pinyin.lambdas.fst");
            let lr = data_dir.join("pinyin.lambdas.redb");
            if lf.exists() && lr.exists() {
                match libchinese_core::Interpolator::load(&lf, &lr) {
                    Ok(i) => Some(std::sync::Arc::new(i)),
                    Err(e) => { eprintln!("warning: failed to load interpolator: {}", e); None }
                }
            } else { None }
        };

        let cfg = libchinese_core::Config::default();

        let model = Model::new(lex, ngram, userdict, cfg, interp);

        // Build a parser stub; for parity you may want to load upstream pinyin table.
        let parser = Parser::with_syllables(&[
            "a", "ai", "an", "ang", "ao", "ba", "bai", "ban", "bang", "bao",
        ]);

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
            let key = Self::segmentation_to_key(&seg);

            // Generate fuzzy alternative keys if segmentation contains fuzzy matches
            // The key difference: we generate alternatives for each syllable in the segmentation
            let keys_to_try = if seg.iter().any(|s| s.fuzzy) {
                self.generate_fuzzy_key_alternatives_from_segmentation(&seg)
            } else {
                vec![key.clone()]
            };

            // Try all alternative keys (exact + fuzzy alternatives)
            let mut candidates = Vec::new();
            for alt_key in keys_to_try {
                let mut key_candidates = self.model.candidates_for_key(&alt_key, self.limit);
                candidates.append(&mut key_candidates);
            }

            // Also always try the exact key to ensure we don't miss direct matches
            let exact_key_already_tried = candidates.iter().any(|c| {
                // Check if exact key was already tried by seeing if we have candidates
                // This is a simple check - we could be more precise
                true 
            });
            if !exact_key_already_tried {
                let mut exact_candidates = self.model.candidates_for_key(&key, self.limit);
                candidates.append(&mut exact_candidates);
            }

            // If this segmentation used any fuzzy matches, apply an additional penalty
            // to the candidates produced for this segmentation. This preserves the
            // centralized scoring while allowing parser-level fuzzy signals to influence
            // ranking.
            let used_fuzzy = seg.iter().any(|s| s.fuzzy);
            if used_fuzzy {
                let penalty = self.fuzzy.penalty();
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
        // TODO: schedule background persistence (redb flush) if backend demands it.
    }

    /// Persist the in-memory userdict to a redb file at `path`.
    ///
    /// This writes a simple u64 frequency table matching `core::userdict` layout.
    pub fn persist_userdict<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), Box<dyn Error>> {
        let map = self.model.userdict.snapshot();
        let ud = UserDict::new_redb(path).map_err(|e| format!("open userdict redb: {}", e))?;
        for (k, v) in map.into_iter() {
            // write frequencies directly
            ud.learn_with_count(&k, v).map_err(|e| format!("write userdict: {}", e))?;
        }
        Ok(())
    }

    /// Generate fuzzy alternative keys from a segmentation by considering fuzzy alternatives
    /// for each syllable in the segmentation. This provides more precise fuzzy matching.
    fn generate_fuzzy_key_alternatives_from_segmentation(&self, segmentation: &[Syllable]) -> Vec<String> {
        let mut alternatives = Vec::new();
        
        // Always include the original key
        let original_key = Self::segmentation_to_key(segmentation);
        alternatives.push(original_key);
        
        // For segmentations with fuzzy syllables, we need to generate alternatives
        // by trying fuzzy alternatives for each syllable position
        self.generate_combinations_recursive(segmentation, 0, String::new(), &mut alternatives);
        
        // Remove duplicates while preserving order
        let mut unique_alternatives = Vec::new();
        for alt in alternatives {
            if !unique_alternatives.contains(&alt) {
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
        
        // Get alternatives for this syllable
        let alternatives = if syllable.fuzzy {
            // For fuzzy syllables, use the fuzzy map to get alternatives
            self.fuzzy.alternatives(&syllable.text)
        } else {
            // For exact syllables, only use the syllable itself
            vec![syllable.text.clone()]
        };
        
        // For each alternative, recurse to the next position
        for alt in alternatives {
            let new_current = format!("{}{}", current, alt);
            self.generate_combinations_recursive(segmentation, position + 1, new_current, results);
        }
    }

    /// Generate fuzzy alternative keys for a given key using the fuzzy map.
    /// This allows the engine to find candidates for fuzzy matches.
    /// NOTE: This method is deprecated in favor of generate_fuzzy_key_alternatives_from_segmentation
    fn generate_fuzzy_key_alternatives(&self, key: &str) -> Vec<String> {
        let mut alternatives = vec![key.to_string()];
        
        // For each syllable in the key, generate fuzzy alternatives
        // This is a simplified approach - in practice we'd need to parse the key back to syllables
        // For now, we'll try common fuzzy substitutions on the whole key
        let fuzzy_alts = self.fuzzy.alternatives(key);
        for alt in fuzzy_alts {
            if alt != key && !alternatives.contains(&alt) {
                alternatives.push(alt);
            }
        }
        
        alternatives
    }

    /// Helper: convert a segmentation (Vec<Syllable>) into a canonical lookup key.
    fn segmentation_to_key(seg: &[Syllable]) -> String {
        seg.iter().map(|s| s.text.as_str()).collect::<String>()
    }

    /// Convenience: load a `Model` and instantiate Engine using table helpers.
    ///
    /// Example (conceptual): Engine::from_model_file(\"model.bin\")
    pub fn from_model_file<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Use the tables module to read a bincode/fst model.
        let model = tables::load_model_bincode(path)?;
        // Create a default parser (in practice the parser is language-specific and must be loaded)
        let parser = Parser::with_syllables(&[
            "a", "ai", "an", "ang", "ao", "ba", "bai", "ban", "bang", "bao", "bei", "ben", "beng",
        ]);
        Ok(Self::new(model, parser))
    }
}

/// Fuzzy and tables modules are included here as lightweight skeletons so the
/// libpinyin crate has a clear place to continue implementing language-specific
/// behavior without adding many files at once.
///
/// These implementations are intentionally minimal and documented with TODOs for
/// the next phases.
pub mod fuzzy {
    use libchinese_core::Config;
    use std::collections::HashMap;

    /// Public fuzzy map used by the Engine.
    ///
    /// In the upstream project this can be more nuanced (per-syllable penalties,
    /// asymmetric mappings, user-configurable rules). Here we provide a simple
    /// canonicalization / alternative list generator and a placeholder penalty.
    #[derive(Debug, Clone)]
    pub struct FuzzyMap {
        // mapping from canonical syllable -> alternate syllables
        map: HashMap<String, Vec<String>>,
    }

    impl FuzzyMap {
        /// Build from a `Config` (which may contain textual fuzzy pairs like "zh=z").
        pub fn from_config(cfg: &Config) -> Self {
            let mut fm = FuzzyMap {
                map: HashMap::new(),
            };
            for pair in cfg.fuzzy.iter() {
                if let Some((a, b)) = pair.split_once('=') {
                    let a = a.trim().to_ascii_lowercase();
                    let b = b.trim().to_ascii_lowercase();
                    fm.map.entry(a.clone()).or_default().push(b.clone());
                    fm.map.entry(b).or_default().push(a);
                }
            }
            fm
        }

        /// Get alternatives for a given syllable (including itself).
        /// This method generates composed alternatives by applying fuzzy rules to syllable components.
        pub fn alternatives(&self, syllable: &str) -> Vec<String> {
            let mut out = Vec::new();
            out.push(syllable.to_string());
            
            // Direct lookup for whole syllable
            if let Some(alts) = self.map.get(&syllable.to_ascii_lowercase()) {
                out.extend(alts.clone());
            }
            
            // Generate composed alternatives by applying rules to syllable parts
            // This handles cases like "zi" -> "zhi" by recognizing "z" can become "zh"
            let syllable_lower = syllable.to_ascii_lowercase();
            self.generate_composed_alternatives(&syllable_lower, &mut out);
            
            // Remove duplicates while preserving order
            let mut unique_out = Vec::new();
            for alt in out {
                if !unique_out.contains(&alt) {
                    unique_out.push(alt);
                }
            }
            
            unique_out
        }
        
        /// Generate alternatives by composing fuzzy rules with syllable components.
        /// For example, "zi" can become "zhi" because "z" -> "zh".
        fn generate_composed_alternatives(&self, syllable: &str, out: &mut Vec<String>) {
            // Common pinyin syllable patterns for fuzzy matching
            // This is a simplified approach - a full implementation would parse syllable structure
            
            // Handle common initial consonant patterns
            if syllable.starts_with("zh") && syllable.len() > 2 {
                if let Some(z_alts) = self.map.get("zh") {
                    for alt_init in z_alts {
                        let new_syllable = format!("{}{}", alt_init, &syllable[2..]);
                        out.push(new_syllable);
                    }
                }
            } else if syllable.starts_with("ch") && syllable.len() > 2 {
                if let Some(c_alts) = self.map.get("ch") {
                    for alt_init in c_alts {
                        let new_syllable = format!("{}{}", alt_init, &syllable[2..]);
                        out.push(new_syllable);
                    }
                }
            } else if syllable.starts_with("sh") && syllable.len() > 2 {
                if let Some(s_alts) = self.map.get("sh") {
                    for alt_init in s_alts {
                        let new_syllable = format!("{}{}", alt_init, &syllable[2..]);
                        out.push(new_syllable);
                    }
                }
            } else if syllable.starts_with('z') && !syllable.starts_with("zh") {
                // "z" -> "zh" case (like "zi" -> "zhi")
                if let Some(z_alts) = self.map.get("z") {
                    for alt_init in z_alts {
                        let new_syllable = format!("{}{}", alt_init, &syllable[1..]);
                        out.push(new_syllable);
                    }
                }
            } else if syllable.starts_with('c') && !syllable.starts_with("ch") {
                // "c" -> "ch" case
                if let Some(c_alts) = self.map.get("c") {
                    for alt_init in c_alts {
                        let new_syllable = format!("{}{}", alt_init, &syllable[1..]);
                        out.push(new_syllable);
                    }
                }
            } else if syllable.starts_with('s') && !syllable.starts_with("sh") {
                // "s" -> "sh" case
                if let Some(s_alts) = self.map.get("s") {
                    for alt_init in s_alts {
                        let new_syllable = format!("{}{}", alt_init, &syllable[1..]);
                        out.push(new_syllable);
                    }
                }
            }
            
            // Handle final sound patterns (an/ang, en/eng, in/ing)
            if syllable.ends_with("an") && !syllable.ends_with("ang") {
                if let Some(an_alts) = self.map.get("an") {
                    for alt_final in an_alts {
                        let prefix = &syllable[..syllable.len()-2];
                        let new_syllable = format!("{}{}", prefix, alt_final);
                        out.push(new_syllable);
                    }
                }
            } else if syllable.ends_with("ang") {
                if let Some(ang_alts) = self.map.get("ang") {
                    for alt_final in ang_alts {
                        let prefix = &syllable[..syllable.len()-3];
                        let new_syllable = format!("{}{}", prefix, alt_final);
                        out.push(new_syllable);
                    }
                }
            } else if syllable.ends_with("en") && !syllable.ends_with("eng") {
                if let Some(en_alts) = self.map.get("en") {
                    for alt_final in en_alts {
                        let prefix = &syllable[..syllable.len()-2];
                        let new_syllable = format!("{}{}", prefix, alt_final);
                        out.push(new_syllable);
                    }
                }
            } else if syllable.ends_with("eng") {
                if let Some(eng_alts) = self.map.get("eng") {
                    for alt_final in eng_alts {
                        let prefix = &syllable[..syllable.len()-3];
                        let new_syllable = format!("{}{}", prefix, alt_final);
                        out.push(new_syllable);
                    }
                }
            } else if syllable.ends_with("in") && !syllable.ends_with("ing") {
                if let Some(in_alts) = self.map.get("in") {
                    for alt_final in in_alts {
                        let prefix = &syllable[..syllable.len()-2];
                        let new_syllable = format!("{}{}", prefix, alt_final);
                        out.push(new_syllable);
                    }
                }
            } else if syllable.ends_with("ing") {
                if let Some(ing_alts) = self.map.get("ing") {
                    for alt_final in ing_alts {
                        let prefix = &syllable[..syllable.len()-3];
                        let new_syllable = format!("{}{}", prefix, alt_final);
                        out.push(new_syllable);
                    }
                }
            }
        }

        /// Return a simple penalty associated with fuzzy matches. This is a
        /// placeholder; the real penalty can be per-rule and learned/tuned.
        pub fn penalty(&self) -> f32 {
            // default lightweight penalty; engine may combine this with userdict / ngram deltas
            1.0
        }
    }
}

pub mod tables {
    use libchinese_core::Model;
    use std::path::Path;

    /// Load a bincode-serialized `Model` produced by the Rust builder.
    ///
    /// Note: Full Model serialization is complex due to Arc types and database connections.
    /// In production, use Engine::from_data_dir() to load components separately.
    pub fn load_model_bincode<P: AsRef<Path>>(
        _path: P,
    ) -> Result<Model, Box<dyn std::error::Error>> {
        Err(Box::from(
            "load_model_bincode: Use Engine::from_data_dir() for production model loading",
        ))
    }

    /// Save model as bincode (used by builder CLI).
    ///
    /// Note: Full Model serialization is complex due to Arc types and database connections.
    /// In production, save components separately using their individual save methods.
    pub fn save_model_bincode<P: AsRef<Path>>(
        _model: &Model,
        _path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err(Box::from(
            "save_model_bincode: Use individual component save methods for production",
        ))
    }

    // TODOs / future responsibilities:
    // - Implement `load_legacy_phrase_table` to read libpinyin phrase binary formats and
    //   convert them into the new Model layout (or produce an fst + phrase blob).
    // - Provide incremental loaders for extremely large models (memory-mapped lexicon + lazy phrase fetch).
    // - Provide utilities to read `pinyin_parser_table.h`-like generated tables if needed for parity.
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
    let user = UserDict::new();
        let cfg = Config::default();
    let model = Model::new(lex, ng, user, cfg, None);

        // Create parser seeded with syllables
        let parser = crate::parser::Parser::with_syllables(&["ni", "hao"]);

        let engine = Engine::new(model, parser);
        let cands = engine.input("nihao");
        // Expect at least one candidate and that top candidate is "你好"
        assert!(!cands.is_empty());
        assert_eq!(cands[0].text, "你好");
    }
}

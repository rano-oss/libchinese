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

use crate::parser::Parser;
use crate::parser::Syllable;
use libchinese_core::{Candidate, Model};

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
        }
    }

    /// Set candidate limit (fluent API).
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Main input API. Returns ranked `Candidate` items for the given raw input.
    ///
    /// Flow:
    /// 1. Use `parser` to produce top-k segmentations (token sequences).
    /// 2. For each segmentation, generate a canonical key (join syllables).
    /// 3. Query `model.lexicon` for phrase candidates for each key.
    /// 4. Score candidates via `model.ngram` and boost with `model.userdict`.
    /// 5. Merge duplicates from multiple segmentations and return top `limit`.
    pub fn input(&self, input: &str) -> Vec<Candidate> {
        // Get top segmentations (k best). Parser returns Vec<Vec<Syllable>>
        let segs = self.parser.segment_top_k(input, 4, true);

        // Map from phrase -> best Candidate (keep highest score)
        let mut best: HashMap<String, Candidate> = HashMap::new();

        for seg in segs.into_iter() {
            // Convert segmentation into a canonical key.
            // Convention: join syllable texts with no separator (e.g. "ni" + "hao" -> "nihao").
            // Language crates may change this joiner if appropriate.
            let key = Self::segmentation_to_key(&seg);

            // Use unified Model.candidates_for_key to obtain scored candidates for this key.
            // This centralizes lexicon lookup, n-gram scoring and userdict boosting.
            let mut candidates = self.model.candidates_for_key(&key, self.limit);

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
        vec
    }

    /// Commit a phrase selection (user accepted phrase) to the user dictionary.
    /// This increments user learning counts so future queries are biased.
    pub fn commit(&mut self, phrase: &str) {
        // Persist learning to runtime userdict
        self.model.userdict.learn(phrase);
        // TODO: schedule background persistence (redb flush) if backend demands it.
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
        pub fn alternatives(&self, syllable: &str) -> Vec<String> {
            let mut out = Vec::new();
            out.push(syllable.to_string());
            if let Some(alts) = self.map.get(&syllable.to_ascii_lowercase()) {
                out.extend(alts.clone());
            }
            out
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
    /// This loader is intentionally simple. Later we will:
    ///  - support fst-backed lexicon + separate ngram files
    ///  - support reading legacy libpinyin binary formats (via conversion tool)
    pub fn load_model_bincode<P: AsRef<Path>>(
        _path: P,
    ) -> Result<Model, Box<dyn std::error::Error>> {
        // Model loader not implemented in this minimal crate build. The builder
        // CLI or a higher-level consumer should provide proper model loading
        // (e.g. fst + separate ngram files). Returning an explicit error keeps
        // compilation possible without adding a bincode dependency here.
        Err(Box::from(
            "load_model_bincode is not implemented in this build",
        ))
    }

    /// Save model as bincode (used by builder CLI).
    pub fn save_model_bincode<P: AsRef<Path>>(
        _model: &Model,
        _path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Saver not implemented in this minimal crate build. Use a dedicated
        // builder CLI in the workspace to produce models for the engine.
        Err(Box::from(
            "save_model_bincode is not implemented in this build",
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
        let model = Model::new(lex, ng, user, cfg);

        // Create parser seeded with syllables
        let parser = crate::parser::Parser::with_syllables(&["ni", "hao"]);

        let engine = Engine::new(model, parser);
        let cands = engine.input("nihao");
        // Expect at least one candidate and that top candidate is "你好"
        assert!(!cands.is_empty());
        assert_eq!(cands[0].text, "你好");
    }
}

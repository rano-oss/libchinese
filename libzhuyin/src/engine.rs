//! Zhuyin/Bopomofo engine for libzhuyin
//!
//! This engine provides the same interface as libpinyin but specialized for
//! Zhuyin/Bopomofo input method.

use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use crate::parser::{ZhuyinParser, ZhuyinSyllable};
use libchinese_core::{Candidate, Model, Lexicon, NGramModel, UserDict, Config, Interpolator};
use std::sync::Arc;

/// Public engine for libzhuyin
pub struct Engine {
    model: Model,
    parser: ZhuyinParser,
    /// Maximum candidates to return
    limit: usize,
}

impl Engine {
    /// Construct an Engine from a pre-built Model and ZhuyinParser
    pub fn new(model: Model, parser: ZhuyinParser) -> Self {
        Self {
            model,
            parser,
            limit: 8,
        }
    }
    
    /// Load an engine from a zhuyin data directory
    pub fn from_data_dir<P: AsRef<Path>>(data_dir: P) -> Result<Self, Box<dyn Error>> {
        let data_dir = data_dir.as_ref();
        
        // Load lexicon
        let fst_path = data_dir.join("zhuyin.fst");
        let redb_path = data_dir.join("zhuyin.redb");
        
        let lex = if fst_path.exists() && redb_path.exists() {
            match Lexicon::load_from_fst_redb(&fst_path, &redb_path) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("warning: failed to load zhuyin lexicon: {}", e);
                    Lexicon::new()
                }
            }
        } else {
            Lexicon::new()
        };
        
        // Load n-gram model
        let ngram_path = data_dir.join("ngram.bincode");
        let ngram = if ngram_path.exists() {
            match std::fs::read(&ngram_path)
                .ok()
                .and_then(|b| bincode::deserialize::<NGramModel>(&b).ok())
            {
                Some(m) => m,
                None => {
                    eprintln!("warning: failed to load zhuyin ngram model");
                    NGramModel::new()
                }
            }
        } else {
            NGramModel::new()
        };
        
        // Load user dictionary
        let userdict_path = data_dir.join("userdict.redb");
        let userdict = if userdict_path.exists() {
            match UserDict::new_redb(&userdict_path) {
                Ok(u) => u,
                Err(e) => {
                    eprintln!("warning: failed to load zhuyin userdict: {}", e);
                    UserDict::new()
                }
            }
        } else {
            UserDict::new()
        };
        
        // Load interpolator
        let interp_fst = data_dir.join("zhuyin.lambdas.fst");
        let interp_redb = data_dir.join("zhuyin.lambdas.redb");
        let interp = if interp_fst.exists() && interp_redb.exists() {
            match Interpolator::load(&interp_fst, &interp_redb) {
                Ok(i) => Some(Arc::new(i)),
                Err(e) => {
                    eprintln!("warning: failed to load zhuyin interpolator: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let cfg = Config::default();
        let model = Model::new(lex, ngram, userdict, cfg, interp);
        
        // Create parser with comprehensive Bopomofo syllables
        let parser = ZhuyinParser::with_syllables(&[
            "ㄅㄚ", "ㄅㄞ", "ㄅㄢ", "ㄅㄤ", "ㄅㄠ", "ㄆㄚ", "ㄆㄞ", "ㄆㄢ",
            "ㄇㄚ", "ㄇㄞ", "ㄇㄢ", "ㄇㄤ", "ㄇㄠ", "ㄈㄚ", "ㄈㄞ", "ㄈㄢ",
            "ㄉㄚ", "ㄉㄞ", "ㄉㄢ", "ㄉㄤ", "ㄉㄠ", "ㄊㄚ", "ㄊㄞ", "ㄊㄢ",
            "ㄋㄚ", "ㄋㄞ", "ㄋㄢ", "ㄋㄤ", "ㄋㄠ", "ㄌㄚ", "ㄌㄞ", "ㄌㄢ",
            "ㄍㄚ", "ㄍㄞ", "ㄍㄢ", "ㄍㄤ", "ㄍㄠ", "ㄎㄚ", "ㄎㄞ", "ㄎㄢ",
            "ㄏㄚ", "ㄏㄞ", "ㄏㄢ", "ㄏㄤ", "ㄏㄠ", "ㄐㄧ", "ㄐㄧㄚ", "ㄐㄧㄢ",
            "ㄑㄧ", "ㄑㄧㄚ", "ㄑㄧㄢ", "ㄒㄧ", "ㄒㄧㄚ", "ㄒㄧㄢ",
            "ㄓ", "ㄓㄚ", "ㄓㄞ", "ㄓㄢ", "ㄓㄤ", "ㄓㄠ", "ㄓㄨ",
            "ㄔ", "ㄔㄚ", "ㄔㄞ", "ㄔㄢ", "ㄔㄤ", "ㄔㄠ", "ㄔㄨ",
            "ㄕ", "ㄕㄚ", "ㄕㄞ", "ㄕㄢ", "ㄕㄤ", "ㄕㄠ", "ㄕㄨ",
            "ㄖ", "ㄖㄚ", "ㄖㄞ", "ㄖㄢ", "ㄖㄤ", "ㄖㄠ", "ㄖㄨ",
            "ㄗ", "ㄗㄚ", "ㄗㄞ", "ㄗㄢ", "ㄗㄤ", "ㄗㄠ", "ㄗㄨ",
            "ㄘ", "ㄘㄚ", "ㄘㄞ", "ㄘㄢ", "ㄘㄤ", "ㄘㄠ", "ㄘㄨ",
            "ㄙ", "ㄙㄚ", "ㄙㄞ", "ㄙㄢ", "ㄙㄤ", "ㄙㄠ", "ㄙㄨ",
            // With tones
            "ㄋㄧˇ", "ㄋㄧˊ", "ㄋㄧˋ", "ㄋㄧ˙",
            "ㄏㄠˇ", "ㄏㄠˊ", "ㄏㄠˋ", "ㄏㄠ˙",
            "ㄓㄨㄥ", "ㄍㄨㄛˊ",
        ]);
        
        Ok(Self::new(model, parser))
    }
    
    /// Set candidate limit (fluent API)
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
    
    /// Main input API - convert Bopomofo input to Chinese candidates
    pub fn input(&self, input: &str) -> Vec<Candidate> {
        // Get segmentations using the zhuyin parser
        let segs = self.parser.segment_top_k(input, 4, true);
        
        // Map from phrase -> best Candidate
        let mut best: HashMap<String, Candidate> = HashMap::new();
        
        for seg in segs.into_iter() {
            // Convert segmentation to lookup key
            let key = Self::segmentation_to_key(&seg);
            
            // Generate candidates for this key
            let mut candidates = self.model.candidates_for_key(&key, self.limit * 2);
            
            // Apply fuzzy penalty if any syllables were fuzzy matched
            let used_fuzzy = seg.iter().any(|s| s.fuzzy);
            if used_fuzzy {
                let penalty = 1.0; // Simple fuzzy penalty
                for c in candidates.iter_mut() {
                    c.score -= penalty;
                }
            }
            
            // Merge candidates (keep best score per phrase)
            for cand in candidates.into_iter() {
                match best.get(&cand.text) {
                    Some(existing) if existing.score >= cand.score => {}
                    _ => {
                        best.insert(cand.text.clone(), cand);
                    }
                }
            }
        }
        
        // Sort by score and return top results
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
    
    /// Commit a phrase selection to user dictionary
    pub fn commit(&mut self, phrase: &str) {
        self.model.userdict.learn(phrase);
    }
    
    /// Convert segmentation to lookup key
    fn segmentation_to_key(seg: &[ZhuyinSyllable]) -> String {
        seg.iter().map(|s| &s.text).cloned().collect::<Vec<_>>().join("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libchinese_core::{Lexicon, NGramModel, UserDict, Config};
    
    #[test]
    fn engine_zhuyin_basic() {
        // Create a simple test model
        let mut lex = Lexicon::new();
        lex.insert("ㄋㄧˇㄏㄠˇ", "你好");
        lex.insert("ㄓㄨㄥㄍㄨㄛˊ", "中国");
        
        let mut ng = NGramModel::new();
        ng.insert_unigram("你", -1.0);
        ng.insert_unigram("好", -1.2);
        ng.insert_unigram("中", -1.1);
        ng.insert_unigram("国", -1.3);
        
        let user = UserDict::new();
        let cfg = Config::default();
        let model = Model::new(lex, ng, user, cfg, None);
        
        let parser = ZhuyinParser::with_syllables(&["ㄋㄧˇ", "ㄏㄠˇ"]);
        let engine = Engine::new(model, parser);
        
        let cands = engine.input("ㄋㄧˇㄏㄠˇ");
        assert!(!cands.is_empty());
        assert_eq!(cands[0].text, "你好");
    }
}
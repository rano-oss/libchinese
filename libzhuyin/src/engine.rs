//! Zhuyin/Bopomofo engine for libzhuyin
//!
//! This engine provides the same interface as libpinyin but specialized for
//! Zhuyin/Bopomofo input method.

use std::collections::HashMap;

use crate::parser::{ZhuyinParser, ZhuyinSyllable};
use libchinese_core::{Candidate, Interpolator, Model};

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
        
        let temp_path = std::env::temp_dir().join(format!(
            "libzhuyin_test_userdict_{}.redb",
            std::process::id()
        ));
        let user = UserDict::new(&temp_path).expect("create test userdict");
        let cfg = Config::default();
        let model = Model::new(lex, ng, user, cfg, Interpolator::new());
        
        let parser = ZhuyinParser::with_syllables(&["ㄋㄧˇ", "ㄏㄠˇ"]);
        let engine = Engine::new(model, parser);
        
        let cands = engine.input("ㄋㄧˇㄏㄠˇ");
        assert!(!cands.is_empty());
        assert_eq!(cands[0].text, "你好");
    }
}
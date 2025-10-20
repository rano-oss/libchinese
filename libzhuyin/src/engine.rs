//! Zhuyin/Bopomofo engine for libzhuyin
//!
//! This engine provides the same interface as libpinyin but specialized for
//! Zhuyin/Bopomofo input method.

use std::collections::HashMap;
use std::error::Error;

use crate::parser::{ZhuyinParser, ZhuyinSyllable};
use libchinese_core::{Candidate, Model, FuzzyMap, Lexicon, NGramModel, UserDict, Config, Interpolator};

/// Public engine for libzhuyin
pub struct Engine {
    model: Model,
    parser: ZhuyinParser,
    fuzzy: FuzzyMap,
    /// Maximum candidates to return
    limit: usize,
}

impl Engine {
    /// Construct an Engine from a pre-built Model and ZhuyinParser.
    ///
    /// Uses standard zhuyin fuzzy rules by default.
    pub fn new(model: Model, parser: ZhuyinParser) -> Self {
        // Use standard zhuyin fuzzy rules from config
        let rules = crate::standard_fuzzy_rules();
        let fuzzy = FuzzyMap::from_rules(&rules);
        
        Self {
            model,
            parser,
            fuzzy,
            limit: 8,
        }
    }
    
    /// Construct an Engine with custom fuzzy rules.
    pub fn with_fuzzy_rules(model: Model, parser: ZhuyinParser, fuzzy_rules: Vec<String>) -> Self {
        let fuzzy = FuzzyMap::from_rules(&fuzzy_rules);
        
        Self {
            model,
            parser,
            fuzzy,
            limit: 8,
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
            
            match UserDict::new(&ud_path) {
                Ok(u) => u,
                Err(e) => {
                    eprintln!("warning: failed to open userdict at {:?}: {}", ud_path, e);
                    // Fallback to temp userdict
                    let temp_path = std::env::temp_dir().join(format!(
                        "libzhuyin_userdict_{}.redb",
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
                match Interpolator::load(&lf, &lb) {
                    Ok(i) => i,
                    Err(e) => {
                        eprintln!("warning: failed to load lambdas: {}, using empty interpolator", e);
                        Interpolator::new()
                    }
                }
            } else {
                Interpolator::new()
            }
        };

        let cfg = Config::default();
        let model = Model::new(lex, ngram, userdict, cfg, interp);

        // Build parser with zhuyin syllables (all bopomofo syllables)
        let parser = ZhuyinParser::new();

        Ok(Self::new(model, parser))
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
    use libchinese_core::{Lexicon, NGramModel, UserDict, Config, Interpolator};
    
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
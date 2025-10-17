//! Fuzzy matching utilities for libpinyin
//!
//! This module implements a comprehensive fuzzy matching system compatible with
//! upstream libpinyin's fuzzy matching capabilities.
//!
//! Responsibilities:
//! - Parse fuzzy rules from `libchinese_core::Config` (string pairs like `a=b`).
//! - Provide alternatives for a given syllable/token including the original.
//! - Support per-rule penalties for fine-grained scoring control.
//! - Provide standard preset rules matching upstream libpinyin.
//!
//! Fuzzy Rule Categories (from upstream):
//! - Shengmu (initials): zh/z, ch/c, sh/s, n/l, f/h, r/l, k/g
//! - Yunmu (finals): an/ang, en/eng, in/ing, ian/iang
//! - Corrections: ng/gn, ng/mg, iu/iou, ui/uei, un/uen, ue/ve, v/u, ong/on
use std::collections::HashMap;

use libchinese_core::Config;

/// A single fuzzy rule with penalty.
#[derive(Debug, Clone)]
pub struct FuzzyRule {
    /// The canonical (correct) form
    pub from: String,
    /// The alternative (fuzzy) form
    pub to: String,
    /// Penalty for using this fuzzy match (higher = more penalty)
    pub penalty: f32,
}

impl FuzzyRule {
    pub fn new(from: &str, to: &str, penalty: f32) -> Self {
        Self {
            from: from.to_string(),
            to: to.to_string(),
            penalty,
        }
    }
}

/// Represents fuzzy alternatives for phonetic units (syllables).
///
/// For each canonical syllable (lowercased), stores a vector of alternative
/// syllables with their associated penalties.
#[derive(Debug, Clone, Default)]
pub struct FuzzyMap {
    /// Mapping from syllable to (alternative, penalty) pairs
    map: HashMap<String, Vec<(String, f32)>>,
    /// Default penalty for rules without explicit penalty
    default_penalty: f32,
}

impl FuzzyMap {
    /// Create a new empty FuzzyMap with default penalty.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            default_penalty: 1.0,
        }
    }

    /// Create a FuzzyMap with standard upstream libpinyin fuzzy rules.
    ///
    /// This includes the complete set of fuzzy rules from upstream:
    /// - Shengmu confusions (zh/z, ch/c, sh/s, n/l, f/h, r/l, k/g)
    /// - Yunmu confusions (an/ang, en/eng, in/ing)
    /// - Common corrections (ng/gn, iu/iou, ui/uei, un/uen, etc.)
    /// - Composed syllable rules (zi/zhi, si/shi, ci/chi, etc.)
    pub fn with_standard_rules() -> Self {
        let mut fm = Self::new();
        
        // Shengmu (initial) fuzzy rules - lighter penalty (1.0)
        let shengmu_rules = vec![
            ("c", "ch"), ("ch", "c"),
            ("z", "zh"), ("zh", "z"),
            ("s", "sh"), ("sh", "s"),
            ("l", "n"), ("n", "l"),
            ("f", "h"), ("h", "f"),
            ("l", "r"), ("r", "l"),
            ("k", "g"), ("g", "k"),
        ];
        for (a, b) in shengmu_rules {
            fm.add_rule(a, b, 1.0);
        }
        
        // Composed syllable fuzzy rules derived from shengmu rules
        // These handle cases like "zi" <-> "zhi", "si" <-> "shi", etc.
        let composed_rules = vec![
            // z/zh group
            ("zi", "zhi"), ("zhi", "zi"),
            ("za", "zha"), ("zha", "za"),
            ("ze", "zhe"), ("zhe", "ze"),
            ("zu", "zhu"), ("zhu", "zu"),
            ("zai", "zhai"), ("zhai", "zai"),
            ("zei", "zhei"), ("zhei", "zei"),
            ("zao", "zhao"), ("zhao", "zao"),
            ("zou", "zhou"), ("zhou", "zou"),
            ("zan", "zhan"), ("zhan", "zan"),
            ("zen", "zhen"), ("zhen", "zen"),
            ("zang", "zhang"), ("zhang", "zang"),
            ("zeng", "zheng"), ("zheng", "zeng"),
            ("zong", "zhong"), ("zhong", "zong"),
            ("zuan", "zhuan"), ("zhuan", "zuan"),
            ("zun", "zhun"), ("zhun", "zun"),
            ("zui", "zhui"), ("zhui", "zui"),
            ("zuo", "zhuo"), ("zhuo", "zuo"),
            
            // c/ch group
            ("ci", "chi"), ("chi", "ci"),
            ("ca", "cha"), ("cha", "ca"),
            ("ce", "che"), ("che", "ce"),
            ("cu", "chu"), ("chu", "cu"),
            ("cai", "chai"), ("chai", "cai"),
            ("cao", "chao"), ("chao", "cao"),
            ("cou", "chou"), ("chou", "cou"),
            ("can", "chan"), ("chan", "can"),
            ("cen", "chen"), ("chen", "cen"),
            ("cang", "chang"), ("chang", "cang"),
            ("ceng", "cheng"), ("cheng", "ceng"),
            ("cong", "chong"), ("chong", "cong"),
            ("cuan", "chuan"), ("chuan", "cuan"),
            ("cun", "chun"), ("chun", "cun"),
            ("cui", "chui"), ("chui", "cui"),
            ("cuo", "chuo"), ("chuo", "cuo"),
            
            // s/sh group
            ("si", "shi"), ("shi", "si"),
            ("sa", "sha"), ("sha", "sa"),
            ("se", "she"), ("she", "se"),
            ("su", "shu"), ("shu", "su"),
            ("sai", "shai"), ("shai", "sai"),
            ("sao", "shao"), ("shao", "sao"),
            ("sou", "shou"), ("shou", "sou"),
            ("san", "shan"), ("shan", "san"),
            ("sen", "shen"), ("shen", "sen"),
            ("sang", "shang"), ("shang", "sang"),
            ("seng", "sheng"), ("sheng", "seng"),
            ("song", "shong"), ("shong", "song"),
            ("suan", "shuan"), ("shuan", "suan"),
            ("sun", "shun"), ("shun", "sun"),
            ("sui", "shui"), ("shui", "sui"),
            ("suo", "shuo"), ("shuo", "suo"),
        ];
        for (a, b) in composed_rules {
            fm.add_rule(a, b, 1.0);
        }
        
        // Yunmu (final) fuzzy rules - lighter penalty (1.0)
        let yunmu_rules = vec![
            ("an", "ang"), ("ang", "an"),
            ("en", "eng"), ("eng", "en"),
            ("in", "ing"), ("ing", "in"),
            ("ian", "iang"), ("iang", "ian"),
        ];
        for (a, b) in yunmu_rules {
            fm.add_rule(a, b, 1.0);
        }
        
        // Composed syllable rules for an/ang confusion
        let an_ang_rules = vec![
            ("ban", "bang"), ("bang", "ban"),
            ("pan", "pang"), ("pang", "pan"),
            ("man", "mang"), ("mang", "man"),
            ("fan", "fang"), ("fang", "fan"),
            ("dan", "dang"), ("dang", "dan"),
            ("tan", "tang"), ("tang", "tan"),
            ("nan", "nang"), ("nang", "nan"),
            ("lan", "lang"), ("lang", "lan"),
            ("gan", "gang"), ("gang", "gan"),
            ("kan", "kang"), ("kang", "kan"),
            ("han", "hang"), ("hang", "han"),
            ("ran", "rang"), ("rang", "ran"),
            ("zan", "zang"), ("zang", "zan"),
            ("can", "cang"), ("cang", "can"),
            ("san", "sang"), ("sang", "san"),
            ("zhan", "zhang"), ("zhang", "zhan"),
            ("chan", "chang"), ("chang", "chan"),
            ("shan", "shang"), ("shang", "shan"),
            ("yan", "yang"), ("yang", "yan"),
            ("wan", "wang"), ("wang", "wan"),
        ];
        for (a, b) in an_ang_rules {
            fm.add_rule(a, b, 1.0);
        }
        
        // Composed syllable rules for en/eng confusion
        let en_eng_rules = vec![
            ("ben", "beng"), ("beng", "ben"),
            ("pen", "peng"), ("peng", "pen"),
            ("men", "meng"), ("meng", "men"),
            ("fen", "feng"), ("feng", "fen"),
            ("den", "deng"), ("deng", "den"),
            ("ten", "teng"), ("teng", "ten"),
            ("nen", "neng"), ("neng", "nen"),
            ("len", "leng"), ("leng", "len"),
            ("gen", "geng"), ("geng", "gen"),
            ("ken", "keng"), ("keng", "ken"),
            ("hen", "heng"), ("heng", "hen"),
            ("ren", "reng"), ("reng", "ren"),
            ("zen", "zeng"), ("zeng", "zen"),
            ("cen", "ceng"), ("ceng", "cen"),
            ("sen", "seng"), ("seng", "sen"),
            ("zhen", "zheng"), ("zheng", "zhen"),
            ("chen", "cheng"), ("cheng", "chen"),
            ("shen", "sheng"), ("sheng", "shen"),
            ("wen", "weng"), ("weng", "wen"),
        ];
        for (a, b) in en_eng_rules {
            fm.add_rule(a, b, 1.0);
        }
        
        // Composed syllable rules for in/ing confusion
        let in_ing_rules = vec![
            ("bin", "bing"), ("bing", "bin"),
            ("pin", "ping"), ("ping", "pin"),
            ("min", "ming"), ("ming", "min"),
            ("din", "ding"), ("ding", "din"),
            ("tin", "ting"), ("ting", "tin"),
            ("nin", "ning"), ("ning", "nin"),
            ("lin", "ling"), ("ling", "lin"),
            ("jin", "jing"), ("jing", "jin"),
            ("qin", "qing"), ("qing", "qin"),
            ("xin", "xing"), ("xing", "xin"),
            ("yin", "ying"), ("ying", "yin"),
        ];
        for (a, b) in in_ing_rules {
            fm.add_rule(a, b, 1.0);
        }
        
        // Correction rules - medium penalty (1.5)
        let correction_rules = vec![
            ("ng", "gn"), ("ng", "mg"),
            ("iu", "iou"), ("ui", "uei"),
            ("un", "uen"), ("ue", "ve"),
            ("ve", "ue"), ("ong", "on"),
        ];
        for (a, b) in correction_rules {
            fm.add_rule(a, b, 1.5);
        }
        
        // V/U correction - slightly higher penalty (2.0) as it's less common
        let vu_rules = vec![
            ("ju", "jv"), ("qu", "qv"), ("xu", "xv"), ("yu", "yv"),
            ("jue", "jve"), ("que", "qve"), ("xue", "xve"), ("yue", "yve"),
            ("juan", "jvan"), ("quan", "qvan"), ("xuan", "xvan"), ("yuan", "yvan"),
            ("jun", "jvn"), ("qun", "qvn"), ("xun", "xvn"), ("yun", "yvn"),
        ];
        for (a, b) in vu_rules {
            fm.add_rule(a, b, 2.0);
        }
        
        fm
    }

    /// Build a `FuzzyMap` from configuration.
    ///
    /// The config is expected to contain textual fuzzy pairs in `cfg.fuzzy`
    /// like `"zh=z"` or `"zh=z:1.5"` (with optional penalty).
    /// Pairs are inserted bidirectionally by default.
    pub fn from_config(cfg: &Config) -> Self {
        let mut fm = FuzzyMap {
            map: HashMap::new(),
            default_penalty: 1.0,
        };

        for pair in cfg.fuzzy.iter() {
            // Parse formats: "a=b" or "a=b:penalty"
            if let Some((rule, penalty_str)) = pair.split_once(':') {
                // Has explicit penalty
                let penalty = penalty_str.trim().parse::<f32>().unwrap_or(fm.default_penalty);
                if let Some((a, b)) = rule.split_once('=') {
                    let a = a.trim().to_ascii_lowercase();
                    let b = b.trim().to_ascii_lowercase();
                    if !a.is_empty() && !b.is_empty() {
                        fm.add_rule(&a, &b, penalty);
                    }
                }
            } else if let Some((a, b)) = pair.split_once('=') {
                // No explicit penalty, use default
                let a = a.trim().to_ascii_lowercase();
                let b = b.trim().to_ascii_lowercase();
                if !a.is_empty() && !b.is_empty() {
                    fm.add_rule(&a, &b, fm.default_penalty);
                }
            } else {
                // Single token - just ensure it has an entry
                let token = pair.trim().to_ascii_lowercase();
                if !token.is_empty() {
                    fm.map.entry(token).or_default();
                }
            }
        }

        fm
    }

    /// Add a fuzzy rule (bidirectional by default).
    pub fn add_rule(&mut self, from: &str, to: &str, penalty: f32) {
        let from = from.to_ascii_lowercase();
        let to = to.to_ascii_lowercase();
        
        // Add both directions
        self.map.entry(from.clone())
            .or_default()
            .push((to.clone(), penalty));
        self.map.entry(to)
            .or_default()
            .push((from, penalty));
    }

    /// Add a unidirectional fuzzy rule (only from -> to, not reverse).
    pub fn add_rule_unidirectional(&mut self, from: &str, to: &str, penalty: f32) {
        let from = from.to_ascii_lowercase();
        let to = to.to_ascii_lowercase();
        
        self.map.entry(from)
            .or_default()
            .push((to, penalty));
    }

    /// Return alternatives for a syllable including the syllable itself (lowercased).
    ///
    /// Returns a vector of (alternative, penalty) pairs.
    /// The original syllable is always included with penalty 0.0.
    pub fn alternatives(&self, syllable: &str) -> Vec<(String, f32)> {
        let key = syllable.trim().to_ascii_lowercase();
        let mut out = Vec::new();
        
        // Always include the original with zero penalty
        out.push((key.clone(), 0.0));
        
        if let Some(alts) = self.map.get(&key) {
            for (alt, penalty) in alts.iter() {
                // Avoid duplicates
                if !out.iter().any(|(s, _)| s == alt) {
                    out.push((alt.clone(), *penalty));
                }
            }
        }
        out
    }

    /// Get just the alternative strings without penalties (for compatibility).
    pub fn alternative_strings(&self, syllable: &str) -> Vec<String> {
        self.alternatives(syllable)
            .into_iter()
            .map(|(s, _)| s)
            .collect()
    }

    /// Query whether two syllables are considered fuzzy-equivalent (directly).
    ///
    /// Returns Some(penalty) if they are equivalent, None otherwise.
    pub fn is_equivalent(&self, a: &str, b: &str) -> Option<f32> {
        let a = a.trim().to_ascii_lowercase();
        let b = b.trim().to_ascii_lowercase();
        
        if a == b {
            return Some(0.0);
        }
        
        if let Some(alts) = self.map.get(&a) {
            for (alt, penalty) in alts.iter() {
                if alt == &b {
                    return Some(*penalty);
                }
            }
        }
        
        None
    }

    /// Get the penalty for a specific fuzzy match.
    ///
    /// Returns the penalty if `from` can fuzzy match to `to`, otherwise returns None.
    pub fn get_penalty(&self, from: &str, to: &str) -> Option<f32> {
        self.is_equivalent(from, to)
    }

    /// Get the default penalty for rules without explicit penalty.
    pub fn default_penalty(&self) -> f32 {
        self.default_penalty
    }

    /// Set the default penalty.
    pub fn set_default_penalty(&mut self, penalty: f32) {
        self.default_penalty = penalty;
    }

    /// Convenience: apply fuzzy expansion to a sequence of syllables,
    /// producing a list of sequences with per-syllable alternatives and total penalties.
    ///
    /// Returns a vector of (sequence, total_penalty) tuples.
    ///
    /// Example:
    ///   input ["zhong", "guo"] -> output like:
    ///     [(["zhong","guo"], 0.0), (["zong","guo"], 1.0), ...]
    ///
    /// This returns up to `limit` expansions (breadth-first). `limit == 0`
    /// means no limit.
    pub fn expand_sequence(&self, seq: &[String], limit: usize) -> Vec<(Vec<String>, f32)> {
        if seq.is_empty() {
            return vec![];
        }

        // Start with one empty sequence with zero penalty.
        let mut results: Vec<(Vec<String>, f32)> = vec![(Vec::new(), 0.0)];

        for tok in seq.iter() {
            let alts = self.alternatives(tok);
            let mut next: Vec<(Vec<String>, f32)> = Vec::new();
            
            for (r, current_penalty) in results.iter() {
                for (alt, alt_penalty) in alts.iter() {
                    let mut nr = r.clone();
                    nr.push(alt.clone());
                    let total_penalty = current_penalty + alt_penalty;
                    next.push((nr, total_penalty));
                    
                    if limit > 0 && next.len() >= limit {
                        break;
                    }
                }
                if limit > 0 && next.len() >= limit {
                    break;
                }
            }
            
            results = next;
            if limit > 0 && results.len() >= limit {
                results.truncate(limit);
            }
        }

        // Sort by penalty (lower penalties first)
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        results
    }

    /// Simpler version that returns just the sequences without penalties (for compatibility).
    pub fn expand_sequence_strings(&self, seq: &[String], limit: usize) -> Vec<Vec<String>> {
        self.expand_sequence(seq, limit)
            .into_iter()
            .map(|(s, _)| s)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libchinese_core::Config;

    fn cfg_with_pairs(pairs: Vec<&str>) -> Config {
        Config {
            fuzzy: pairs.into_iter().map(|s| s.to_string()).collect(),
            unigram_weight: 0.6,
            bigram_weight: 0.3,
            trigram_weight: 0.1,
        }
    }

    #[test]
    fn test_standard_rules() {
        let fm = FuzzyMap::with_standard_rules();
        
        // Test shengmu rules
        let alts = fm.alternative_strings("zh");
        assert!(alts.contains(&"zh".to_string()));
        assert!(alts.contains(&"z".to_string()));
        
        // Test yunmu rules
        let alts = fm.alternative_strings("an");
        assert!(alts.contains(&"an".to_string()));
        assert!(alts.contains(&"ang".to_string()));
        
        // Test composed syllable rules - an/ang
        let alts = fm.alternative_strings("fan");
        assert!(alts.contains(&"fan".to_string()));
        assert!(alts.contains(&"fang".to_string()), "fan should have fang as alternative");
        
        // Test v/u correction
        let alts = fm.alternative_strings("ju");
        assert!(alts.contains(&"ju".to_string()));
        assert!(alts.contains(&"jv".to_string()));
    }

    #[test]
    fn test_per_rule_penalties() {
        let mut fm = FuzzyMap::new();
        
        // Add rules with different penalties
        fm.add_rule("zh", "z", 1.0);
        fm.add_rule("an", "ang", 1.5);
        fm.add_rule("ju", "jv", 2.0);
        
        // Check penalties
        assert_eq!(fm.get_penalty("zh", "z"), Some(1.0));
        assert_eq!(fm.get_penalty("z", "zh"), Some(1.0)); // bidirectional
        assert_eq!(fm.get_penalty("an", "ang"), Some(1.5));
        assert_eq!(fm.get_penalty("ju", "jv"), Some(2.0));
        assert_eq!(fm.get_penalty("x", "y"), None);
        
        // Check alternatives include penalties
        let alts = fm.alternatives("zh");
        assert_eq!(alts.len(), 2); // "zh" (0.0) and "z" (1.0)
        assert!(alts.iter().any(|(s, p)| s == "zh" && *p == 0.0));
        assert!(alts.iter().any(|(s, p)| s == "z" && *p == 1.0));
    }

    #[test]
    fn test_config_parsing() {
        // Test basic format
        let cfg = cfg_with_pairs(vec!["zh=z", "ch=c", "sh=s"]);
        let fm = FuzzyMap::from_config(&cfg);

        let alts = fm.alternative_strings("zh");
        assert!(alts.contains(&"zh".to_string()));
        assert!(alts.contains(&"z".to_string()));

        assert_eq!(fm.is_equivalent("zh", "z"), Some(1.0)); // default penalty
        assert_eq!(fm.is_equivalent("z", "zh"), Some(1.0));
        assert_eq!(fm.is_equivalent("zh", "x"), None);
    }

    #[test]
    fn test_config_with_penalties() {
        // Test format with explicit penalties
        let cfg = cfg_with_pairs(vec!["zh=z:1.5", "an=ang:2.0"]);
        let fm = FuzzyMap::from_config(&cfg);

        assert_eq!(fm.get_penalty("zh", "z"), Some(1.5));
        assert_eq!(fm.get_penalty("an", "ang"), Some(2.0));
    }

    #[test]
    fn test_expand_sequence_with_penalties() {
        let mut fm = FuzzyMap::new();
        fm.add_rule("zh", "z", 1.0);
        fm.add_rule("an", "ang", 1.5);
        
        let seq = vec!["zh".to_string(), "an".to_string()];
        let expansions = fm.expand_sequence(&seq, 0);
        
        // Should have 4 combinations:
        // ["zh", "an"] - penalty 0.0
        // ["zh", "ang"] - penalty 1.5
        // ["z", "an"] - penalty 1.0
        // ["z", "ang"] - penalty 2.5
        assert_eq!(expansions.len(), 4);
        
        // Check that penalties are correct (sorted by penalty)
        assert_eq!(expansions[0].1, 0.0); // Original
        assert_eq!(expansions[1].1, 1.0); // One fuzzy
        assert_eq!(expansions[2].1, 1.5); // One fuzzy
        assert_eq!(expansions[3].1, 2.5); // Two fuzzy
    }

    #[test]
    fn test_expand_sequence_strings() {
        let cfg = cfg_with_pairs(vec!["zh=z", "l=n"]);
        let fm = FuzzyMap::from_config(&cfg);
        let seq = vec!["zh".to_string(), "l".to_string()];
        
        let ex = fm.expand_sequence_strings(&seq, 0);
        
        // Should have 4 combinations
        assert_eq!(ex.len(), 4);
        assert!(ex.iter().any(|s| s == &vec!["zh".to_string(), "l".to_string()]));
        assert!(ex.iter().any(|s| s == &vec!["z".to_string(), "l".to_string()]));
        assert!(ex.iter().any(|s| s == &vec!["zh".to_string(), "n".to_string()]));
        assert!(ex.iter().any(|s| s == &vec!["z".to_string(), "n".to_string()]));
    }

    #[test]
    fn test_unidirectional_rules() {
        let mut fm = FuzzyMap::new();
        
        // Add unidirectional rule
        fm.add_rule_unidirectional("zi", "zhi", 1.0);
        
        // zi -> zhi should work
        assert_eq!(fm.get_penalty("zi", "zhi"), Some(1.0));
        
        // But zhi -> zi should not
        assert_eq!(fm.get_penalty("zhi", "zi"), None);
    }

    #[test]
    fn test_default_penalty() {
        let mut fm = FuzzyMap::new();
        assert_eq!(fm.default_penalty(), 1.0);
        
        fm.set_default_penalty(2.5);
        assert_eq!(fm.default_penalty(), 2.5);
    }
}

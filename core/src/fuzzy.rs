//! Fuzzy matching for phonetic similarity (pinyin/zhuyin).
use std::collections::HashMap;

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

    /// Build a `FuzzyMap` from a list of fuzzy rule strings.
    ///
    /// The rules are expected to be textual fuzzy pairs like `"zh=z"` or
    /// `"zh=z:1.5"` (with optional penalty).
    /// Pairs are inserted bidirectionally by default.
    pub fn from_rules(rules: &[String]) -> Self {
        let mut fm = FuzzyMap {
            map: HashMap::new(),
            default_penalty: 1.0,
        };

        for pair in rules.iter() {
            // Parse formats: "a=b" or "a=b:penalty"
            if let Some((rule, penalty_str)) = pair.split_once(':') {
                // Has explicit penalty
                let penalty = penalty_str
                    .trim()
                    .parse::<f32>()
                    .unwrap_or(fm.default_penalty);
                if let Some((a, b)) = rule.split_once('=') {
                    let a = a.trim().to_string();
                    let b = b.trim().to_string();
                    if !a.is_empty() && !b.is_empty() {
                        fm.add_rule(&a, &b, penalty);
                    }
                }
            } else if let Some((a, b)) = pair.split_once('=') {
                // No explicit penalty, use default
                let a = a.trim().to_string();
                let b = b.trim().to_string();
                if !a.is_empty() && !b.is_empty() {
                    fm.add_rule(&a, &b, fm.default_penalty);
                }
            } else {
                // Single token - just ensure it has an entry
                let token = pair.trim().to_string();
                if !token.is_empty() {
                    fm.map.entry(token).or_default();
                }
            }
        }

        fm
    }

    /// Add a fuzzy rule (bidirectional by default).
    pub fn add_rule(&mut self, from: &str, to: &str, penalty: f32) {
        let from = from.to_string();
        let to = to.to_string();

        // Add both directions
        self.map
            .entry(from.clone())
            .or_default()
            .push((to.clone(), penalty));
        self.map.entry(to).or_default().push((from, penalty));
    }

    /// Add a unidirectional fuzzy rule (only from -> to, not reverse).
    pub fn add_rule_unidirectional(&mut self, from: &str, to: &str, penalty: f32) {
        let from = from.to_string();
        let to = to.to_string();

        self.map.entry(from).or_default().push((to, penalty));
    }

    /// Return alternatives for a syllable including the syllable itself.
    ///
    /// Returns a vector of (alternative, penalty) pairs.
    /// The original syllable is always included with penalty 0.0.
    pub fn alternatives(&self, syllable: &str) -> Vec<(String, f32)> {
        let key = syllable.trim().to_string();
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
        let a = a.trim().to_string();
        let b = b.trim().to_string();

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

//! N-gram model skeleton for libchinese-core.
//!
//! Responsibilities:
//! - Store unigram / bigram / trigram log-probabilities.
//! - Provide scoring helpers (interpolated log-prob scoring).
//! - Provide simple training helper to convert counts -> log-probs.
//! - Provide (basic) serialization helpers for bincode-based model IO.
//!
//! Reference upstream files for behavior and algorithms:
//! - libpinyin: `src/storage/ngram.cpp`, `utils/training/*`
//!
//! Notes:
//! - This is an intentionally simple and clear implementation to serve as a
//!   correctness-first reference. It can be optimized (use `ahash`, packed
//!   binary formats, memory-mapped data structures, or more advanced smoothing)
//!   in later phases.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

/// Lightweight container holding ln(probabilities) for 1/2/3-grams.
///
/// Probabilities are stored as natural logarithms (ln). The model is generic
/// in that it stores arbitrary string tokens — language crates are responsible
/// for tokenizing phrases into tokens appropriate for the n-gram model
/// (characters, words, or segmented tokens).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NGramModel {
    /// unigram: log P(w)
    unigram: HashMap<String, f32>,

    /// bigram: log P(w2 | w1) stored keyed by (w1, w2)
    bigram: HashMap<(String, String), f32>,

    /// trigram: log P(w3 | w1, w2) keyed by (w1, w2, w3)
    trigram: HashMap<(String, String, String), f32>,
}

impl NGramModel {
    /// Create an empty model.
    pub fn new() -> Self {
        Self {
            unigram: HashMap::new(),
            bigram: HashMap::new(),
            trigram: HashMap::new(),
        }
    }

    /// Insert a unigram ln(probability).
    pub fn insert_unigram(&mut self, w: impl Into<String>, log_p: f32) {
        self.unigram.insert(w.into(), log_p);
    }

    /// Insert a bigram ln(probability).
    pub fn insert_bigram(&mut self, w1: impl Into<String>, w2: impl Into<String>, log_p: f32) {
        self.bigram.insert((w1.into(), w2.into()), log_p);
    }

    /// Insert a trigram ln(probability).
    pub fn insert_trigram(
        &mut self,
        w1: impl Into<String>,
        w2: impl Into<String>,
        w3: impl Into<String>,
        log_p: f32,
    ) {
        self.trigram
            .insert((w1.into(), w2.into(), w3.into()), log_p);
    }

    /// Get unigram ln-prob if present.
    pub fn get_unigram(&self, w: &str) -> Option<f32> {
        self.unigram.get(w).copied()
    }

    /// Get bigram ln-prob if present.
    pub fn get_bigram(&self, w1: &str, w2: &str) -> Option<f32> {
        self.bigram.get(&(w1.to_string(), w2.to_string())).copied()
    }

    /// Get trigram ln-prob if present.
    pub fn get_trigram(&self, w1: &str, w2: &str, w3: &str) -> Option<f32> {
        self.trigram
            .get(&(w1.to_string(), w2.to_string(), w3.to_string()))
            .copied()
    }

    /// Score a token sequence using linear interpolation of 1/2/3-gram ln-probabilities.
    ///
    /// Parameters:
    /// - `tokens`: tokenized sequence (e.g. characters or words)
    /// - `unigram_weight`, `bigram_weight`, `trigram_weight`: interpolation weights.
    ///
    /// Behavior:
    /// For each token position i, we compute:
    ///   u = ln P(w_i) or floor
    ///   b = ln P(w_i | w_{i-1}) or fallback to u
    ///   t = ln P(w_i | w_{i-2}, w_{i-1}) or fallback to b
    /// score += unigram_weight * u + bigram_weight * b + trigram_weight * t
    ///
    /// Missing probabilities fall back to lower-order models or a floor ln-prob.
    pub fn score_sequence(
        &self,
        tokens: &[String],
        unigram_weight: f32,
        bigram_weight: f32,
        trigram_weight: f32,
    ) -> f32 {
        // Defensive: if no tokens, return negative infinity to indicate impossibility.
        if tokens.is_empty() {
            return std::f32::NEG_INFINITY;
        }

        // floor ln-probability for OOV: very small probability in ln-space.
        let floor = -20.0f32; // ~= 2e-9

        let mut score = 0f32;

        for i in 0..tokens.len() {
            let u = self.get_unigram(&tokens[i]).unwrap_or(floor);

            let b = if i >= 1 {
                self.get_bigram(&tokens[i - 1], &tokens[i]).unwrap_or(u)
            } else {
                u
            };

            let t = if i >= 2 {
                self.get_trigram(&tokens[i - 2], &tokens[i - 1], &tokens[i])
                    .unwrap_or(b)
            } else {
                b
            };

            score += unigram_weight * u + bigram_weight * b + trigram_weight * t;
        }

        score
    }

    // --- Training helper utilities (counts -> log-probabilities) ---

    /// Convert unigram counts to ln(probabilities) using add-k smoothing.
    ///
    /// `counts` is a map token -> count. `k` is the smoothing constant (default 0.0 = no smoothing).
    /// Returns a HashMap token -> ln(prob).
    pub fn counts_to_unigram_logprob(
        counts: &HashMap<String, u64>,
        k: f32,
    ) -> HashMap<String, f32> {
        let mut out: HashMap<String, f32> = HashMap::new();
        let mut total: f32 = 0.0;
        for (_tok, &c) in counts.iter() {
            total += c as f32;
        }
        // With add-k smoothing, effective total is total + k * V
        let v = counts.len() as f32;
        let denom = total + k * v;
        for (tok, &c) in counts.iter() {
            let p = (c as f32 + k) / denom;
            out.insert(tok.clone(), p.ln());
        }
        out
    }

    /// Convert conditional bigram counts (count(w1,w2)) and unigram counts (count(w1))
    /// into ln P(w2 | w1) using add-k smoothing.
    ///
    /// - `bigram_counts`: map (w1,w2) -> count
    /// - `unigram_counts`: map w1 -> count (denominator)
    /// - `k`: smoothing constant
    pub fn counts_to_bigram_logprob(
        bigram_counts: &HashMap<(String, String), u64>,
        unigram_counts: &HashMap<String, u64>,
        k: f32,
    ) -> HashMap<(String, String), f32> {
        // Build denominator map from unigram_counts
        let mut out: HashMap<(String, String), f32> = HashMap::new();

        // For each bigram (w1,w2): p = (count(w1,w2) + k) / (count(w1) + k * Vw1)
        // where Vw1 is number of distinct continuations for w1; here we approximate Vw1 by
        // the number of distinct bigrams with that w1 found in bigram_counts.
        let mut cont_count: HashMap<&String, usize> = HashMap::new();
        for ((w1, _w2), _c) in bigram_counts.iter() {
            *cont_count.entry(w1).or_insert(0) += 1;
        }

        for ((w1, w2), &c) in bigram_counts.iter() {
            let denom_count = unigram_counts.get(w1).copied().unwrap_or(0) as f32;
            let v = cont_count.get(w1).copied().unwrap_or(0) as f32;
            // fallback if v==0 (shouldn't happen): use 1
            let v = if v < 1.0 { 1.0 } else { v };
            let denom = denom_count + k * v;
            let p = (c as f32 + k) / denom;
            out.insert((w1.clone(), w2.clone()), p.ln());
        }

        out
    }

    /// Convert trigram counts into ln conditional probabilities P(w3 | w1, w2)
    /// using a simple add-k smoothing approach analogous to bigrams.
    pub fn counts_to_trigram_logprob(
        trigram_counts: &HashMap<(String, String, String), u64>,
        bigram_counts: &HashMap<(String, String), u64>,
        k: f32,
    ) -> HashMap<(String, String, String), f32> {
        let mut out: HashMap<(String, String, String), f32> = HashMap::new();

        // compute continuation counts for each bigram prefix (w1,w2)
        let mut cont_count: HashMap<(&String, &String), usize> = HashMap::new();
        for ((w1, w2, _w3), _c) in trigram_counts.iter() {
            *cont_count.entry((w1, w2)).or_insert(0) += 1;
        }

        for ((w1, w2, w3), &c) in trigram_counts.iter() {
            let denom_count = bigram_counts
                .get(&(w1.clone(), w2.clone()))
                .copied()
                .unwrap_or(0) as f32;
            let v = cont_count.get(&(w1, w2)).copied().unwrap_or(0) as f32;
            let v = if v < 1.0 { 1.0 } else { v };
            let denom = denom_count + k * v;
            let p = (c as f32 + k) / denom;
            out.insert((w1.clone(), w2.clone(), w3.clone()), p.ln());
        }

        out
    }

    /// Estimate interpolation lambda (simple EM-like iterative estimator).
    ///
    /// This function implements a lightweight adaptation of the interpolation
    /// estimation logic used by libpinyin's `estimate_interpolation.cpp`.
    /// It accepts:
    /// - `deleted_bigram_counts`: counts for the deleted_bigram table (token -> count)
    ///   represented here by a map from (w1, w2) -> count.
    /// - `unigram_counts`: unigram counts map w -> count
    /// - `bigram_counts`: bigram conditional counts map (w1, w2) -> count
    ///
    /// Returns a lambda in [0.0, 1.0]. The algorithm iterates until convergence.
    pub fn estimate_interpolation(
        deleted_bigram_counts: &HashMap<(String, String), u64>,
        unigram_counts: &HashMap<String, u64>,
        bigram_counts: &HashMap<(String, String), u64>,
    ) -> f32 {
        // convert totals to floats
        let total_deleted: f32 = deleted_bigram_counts.values().map(|&v| v as f32).sum();
        if total_deleted <= 0.0 {
            return 0.0;
        }

        let total_unigram: f32 = unigram_counts.values().map(|&v| v as f32).sum();
        let total_bigram: f32 = bigram_counts.values().map(|&v| v as f32).sum();

        // start with an initial lambda (common default)
        let mut lambda: f32 = 0.6;
        let epsilon: f32 = 1e-4;
        let mut next_lambda: f32 = lambda;

        // iterate
        for _iter in 0..1000 {
            lambda = next_lambda;
            let mut accum: f32 = 0.0;

            for (k, &deleted_count) in deleted_bigram_counts.iter() {
                // signature: k = (w1, w2)
                let (_w1, w2) = (k.0.clone(), k.1.clone());

                // estimate bigram continuation probability for this (w1,w2)
                let bigram_count = *bigram_counts.get(k).unwrap_or(&0) as f32;
                // rough normalization: if total_bigram is zero, treat as zero prob
                let elem_bigram = if total_bigram > 0.0 {
                    bigram_count / total_bigram
                } else {
                    0.0
                };

                // unigram probability for w2
                let unigram_count = *unigram_counts.get(&w2).unwrap_or(&0) as f32;
                let elem_unigram = if total_unigram > 0.0 {
                    unigram_count / total_unigram
                } else {
                    0.0
                };

                // numerator/denominator as in compute_interpolation
                let numerator = lambda * elem_bigram;
                let denom_part = (1.0 - lambda) * elem_unigram;

                let denom = numerator + denom_part;
                if denom <= 0.0 {
                    continue;
                }
                accum += (deleted_count as f32) * (numerator / denom);
            }

            // normalize by total deleted counts to get next_lambda estimate
            if total_deleted > 0.0 {
                next_lambda = accum / total_deleted;
            } else {
                next_lambda = lambda;
            }

            if (next_lambda - lambda).abs() < epsilon {
                break;
            }
        }

        // clamp to [0,1]
        if next_lambda.is_nan() {
            0.0
        } else if next_lambda < 0.0 {
            0.0
        } else if next_lambda > 1.0 {
            1.0
        } else {
            next_lambda
        }
    }

    // --- Serialization helpers ---

    /// Save the model to the given path using bincode.
    pub fn save_bincode<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        bincode::serialize_into(&mut writer, self)?;
        Ok(())
    }

    /// Load the model from bincode file.
    pub fn load_bincode<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let model: Self = bincode::deserialize_from(reader)?;
        Ok(model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_sequence_basic() {
        let mut m = NGramModel::new();
        // simple unigram ln-probs (higher is better: less negative)
        m.insert_unigram("你", -1.0);
        m.insert_unigram("好", -1.2);
        m.insert_unigram("中", -1.1);
        m.insert_unigram("国", -1.3);

        // bigram for "你 好"
        m.insert_bigram("你", "好", -0.2);
        // trigram example (not used in 2-token sequence)
        m.insert_trigram("x", "y", "z", -0.05);

        let tokens = vec!["你".to_string(), "好".to_string()];
        // use weights that favor bigram
        let score = m.score_sequence(&tokens, 0.3, 0.6, 0.1);

        // compute expected: for token 0:
        // u0 = lnP(你) = -1.0, b0 = u0, t0 = b0 => contribution = 0.3*(-1.0)+0.6*(-1.0)+0.1*(-1.0) = -1.0
        // token1:
        // u1 = -1.2, b1 = lnP(好|你) = -0.2, t1 = b1
        // contribution = 0.3*(-1.2) + 0.6*(-0.2) + 0.1*(-0.2) = -0.36 -0.12 -0.02 = -0.5
        // total expected = -1.0 + -0.5 = -1.5
        assert!((score - (-1.5)).abs() < 1e-4);
    }

    #[test]
    fn counts_to_unigram_logprob_behaviour() {
        let mut counts: HashMap<String, u64> = HashMap::new();
        counts.insert("a".to_string(), 10);
        counts.insert("b".to_string(), 30);
        let res = NGramModel::counts_to_unigram_logprob(&counts, 0.0);
        // prob(a) = 10/40 = 0.25 -> ln = ln(0.25)
        assert!((res["a"] - (0.25f32).ln()).abs() < 1e-6);
        assert!((res["b"] - (0.75f32).ln()).abs() < 1e-6);
    }

    #[test]
    fn estimate_interpolation_basic() {
        // Construct tiny synthetic data mirroring deleted_bigram / unigram / bigram shapes.
        let mut deleted: HashMap<(String, String), u64> = HashMap::new();
        deleted.insert(("x".to_string(), "a".to_string()), 2);
        deleted.insert(("x".to_string(), "b".to_string()), 6);

        let mut unigram: HashMap<String, u64> = HashMap::new();
        unigram.insert("a".to_string(), 10);
        unigram.insert("b".to_string(), 30);

        let mut bigram: HashMap<(String, String), u64> = HashMap::new();
        bigram.insert(("x".to_string(), "a".to_string()), 5);
        bigram.insert(("x".to_string(), "b".to_string()), 15);

        let lambda = NGramModel::estimate_interpolation(&deleted, &unigram, &bigram);
        // sanity checks: lambda is finite and within [0,1]
        assert!(lambda.is_finite());
        assert!(lambda >= 0.0 && lambda <= 1.0);
    }
}

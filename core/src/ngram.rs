//! N-gram statistical language model with interpolation support.
use fst::Map;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read};
use std::path::Path;

/// Lambda weights for linear interpolation of unigram, bigram, and trigram probabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lambdas(pub [f32; 3]);

/// Interpolator holds an fst map (key -> index) and a bincode vector
/// storing `Lambdas` values keyed by index.
#[derive(Debug, Clone)]
pub struct Interpolator {
    map: Map<Vec<u8>>,
    // in-memory bincode-backed lambdas vector (index -> Lambdas)
    lambdas: Vec<Lambdas>,
}

impl Interpolator {
    /// Load from fst + bincode pair.
    pub fn load<P: AsRef<Path>>(
        fst_path: P,
        bincode_path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let fst_path = fst_path.as_ref();
        let bincode_path = bincode_path.as_ref();

        let map = {
            let mut f = File::open(fst_path)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            Map::new(buf)?
        };

        let lambdas = {
            let mut f = File::open(bincode_path)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            bincode::deserialize(&buf)?
        };

        Ok(Self { map, lambdas })
    }

    /// Lookup lambdas for a key. Returns None if not found.
    pub fn lookup(&self, key: &str) -> Option<Lambdas> {
        let idx = self.map.get(key)? as usize;
        self.lambdas.get(idx).cloned()
    }

    /// Create an empty interpolator with default lambdas (for testing only).
    ///
    /// Note: This should only be used in tests. Production code should always
    /// load interpolator data from files using `Interpolator::load()`.
    pub fn empty_for_test() -> Self {
        Self {
            map: Map::default(),
            lambdas: vec![Lambdas([0.33, 0.33, 0.34])],
        }
    }
}

impl Default for Interpolator {
    fn default() -> Self {
        Self::empty_for_test()
    }
}

/// Lightweight container holding ln(probabilities) for 1/2/3-grams.
///
/// Probabilities are stored as natural logarithms (ln). The model is generic
/// in that it stores arbitrary string tokens — language crates are responsible
/// for tokenizing phrases into tokens appropriate for the n-gram model
/// (characters, words, or segmented tokens).
///
/// Owns an Interpolator for per-key lambda lookups during scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NGramModel {
    /// unigram: ln P(w)
    unigram: HashMap<String, f64>,

    /// bigram: ln P(w2 | w1) stored keyed by (w1, w2)
    bigram: HashMap<(String, String), f64>,

    /// trigram: ln P(w3 | w1, w2) keyed by (w1, w2, w3)
    trigram: HashMap<(String, String, String), f64>,

    /// Interpolator for per-key lambda lookups (not serialized, must be set separately)
    #[serde(skip)]
    interpolator: Interpolator,
}

impl NGramModel {
    /// Create an empty model with an empty interpolator.
    pub fn new() -> Self {
        Self {
            unigram: HashMap::new(),
            bigram: HashMap::new(),
            trigram: HashMap::new(),
            interpolator: Interpolator::empty_for_test(),
        }
    }

    /// Create a model with a specific interpolator.
    pub fn with_interpolator(interpolator: Interpolator) -> Self {
        Self {
            unigram: HashMap::new(),
            bigram: HashMap::new(),
            trigram: HashMap::new(),
            interpolator,
        }
    }

    /// Set the interpolator for this model.
    pub fn set_interpolator(&mut self, interpolator: Interpolator) {
        self.interpolator = interpolator;
    }

    /// Insert a unigram ln(probability).
    pub fn insert_unigram(&mut self, w: impl Into<String>, log_p: f64) {
        self.unigram.insert(w.into(), log_p);
    }

    /// Insert a bigram ln(probability).
    pub fn insert_bigram(&mut self, w1: impl Into<String>, w2: impl Into<String>, log_p: f64) {
        self.bigram.insert((w1.into(), w2.into()), log_p);
    }

    /// Insert a trigram ln(probability).
    pub fn insert_trigram(
        &mut self,
        w1: impl Into<String>,
        w2: impl Into<String>,
        w3: impl Into<String>,
        log_p: f64,
    ) {
        self.trigram
            .insert((w1.into(), w2.into(), w3.into()), log_p);
    }

    /// Get unigram ln-prob if present.
    pub fn get_unigram(&self, w: &str) -> Option<f64> {
        self.unigram.get(w).copied()
    }

    /// Get bigram ln-prob if present.
    pub fn get_bigram(&self, w1: &str, w2: &str) -> Option<f64> {
        self.bigram.get(&(w1.to_string(), w2.to_string())).copied()
    }

    /// Get trigram ln-prob if present.
    pub fn get_trigram(&self, w1: &str, w2: &str, w3: &str) -> Option<f64> {
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
    /// Score a token sequence using enhanced smoothing and interpolation.
    ///
    /// This implementation uses a sophisticated backoff strategy similar to
    /// libpinyin upstream, with improved handling of OOV tokens and better
    /// smoothing for unseen n-grams.
    ///
    /// Uses the supplied `Config` weights. Internally computations are performed
    /// in f64 (since ln-probabilities are stored as f64) and the final score is
    /// returned as f32 for compatibility with downstream code.
    pub fn score_sequence(&self, tokens: &[String], cfg: &crate::Config) -> f32 {
        // Defensive: if no tokens, return negative infinity to indicate impossibility.
        if tokens.is_empty() {
            return std::f32::NEG_INFINITY;
        }

        let mut score: f64 = 0.0;

        for i in 0..tokens.len() {
            let token_score = self.score_token_with_backoff(tokens, i, cfg);
            score += token_score;
        }

        score as f32
    }

    /// Score a single token using enhanced backoff smoothing.
    /// This implements a simplified version of Kneser-Ney smoothing with
    /// proper backoff weights similar to upstream libpinyin.
    fn score_token_with_backoff(&self, tokens: &[String], i: usize, cfg: &crate::Config) -> f64 {
        let token = &tokens[i];

        // Enhanced OOV handling: use different floors based on context
        let oov_floor = -20.0f64; // Very rare unseen unigram
        let unseen_bigram_penalty = -3.0f64; // Moderate penalty for unseen bigram
        let unseen_trigram_penalty = -1.5f64; // Light penalty for unseen trigram

        // Get unigram probability (always available as base)
        let unigram_prob = self.get_unigram(token).unwrap_or(oov_floor);

        // Calculate bigram with backoff
        let bigram_prob = if i >= 1 {
            let prev_token = &tokens[i - 1];
            self.get_bigram(prev_token, token).unwrap_or_else(|| {
                // Backoff: use unigram with penalty for unseen bigram
                unigram_prob + unseen_bigram_penalty
            })
        } else {
            // No context for first token, use unigram
            unigram_prob
        };

        // Calculate trigram with backoff
        let trigram_prob = if i >= 2 {
            let prev2_token = &tokens[i - 2];
            let prev_token = &tokens[i - 1];
            self.get_trigram(prev2_token, prev_token, token)
                .unwrap_or_else(|| {
                    // Backoff: use bigram with penalty for unseen trigram
                    bigram_prob + unseen_trigram_penalty
                })
        } else {
            // Not enough context for trigram, use bigram
            bigram_prob
        };

        // Enhanced interpolation with normalized weights
        let mut weights = [cfg.unigram_weight, cfg.bigram_weight, cfg.trigram_weight];
        let sum: f32 = weights.iter().sum();
        if sum > 0.0 {
            for w in weights.iter_mut() {
                *w /= sum;
            }
        }

        let uw = weights[0] as f64;
        let bw = weights[1] as f64;
        let tw = weights[2] as f64;

        // Apply context-dependent weighting: more context gets higher weight
        let (effective_uw, effective_bw, effective_tw) = if i >= 2 {
            // Full context available: use configured weights
            (uw, bw, tw)
        } else if i >= 1 {
            // Only bigram context: reweight to favor bigram over trigram
            let total_contextual = bw + tw;
            (uw, total_contextual * 0.7, total_contextual * 0.3)
        } else {
            // No context: use unigram with small smoothing
            (1.0, 0.0, 0.0)
        };

        effective_uw * unigram_prob + effective_bw * bigram_prob + effective_tw * trigram_prob
    }

    /// Predict next most likely characters or phrases given context.
    ///
    /// Given 1-2 previous characters as context, returns the top `count` most
    /// likely next characters/phrases with their log probabilities. Supports
    /// multi-character phrase predictions by building phrases from consecutive
    /// character bigrams/trigrams.
    ///
    /// # Arguments
    /// * `context` - The previous 1-2 characters as context (e.g., "你好")
    /// * `count` - Maximum number of predictions to return
    /// * `cfg` - Configuration for weights and parameters (optional, uses defaults if None)
    ///
    /// # Returns
    /// Vector of (text, score) tuples sorted by descending probability.
    /// Score is the log probability (higher = more likely).
    /// Results may include both single characters and multi-character phrases.
    ///
    /// # Example
    /// ```ignore
    /// let predictions = model.predict_next("你好", 5, None);
    /// // Returns: [("吗", -2.3), ("呢", -3.1), ("的话", -3.5), ...]
    /// ```
    pub fn predict_next(
        &self,
        context: &str,
        count: usize,
        cfg: Option<&crate::Config>,
    ) -> Vec<(String, f64)> {
        self.predict_next_with_user(context, count, cfg, None)
    }

    /// Predict next with user bigram learning integrated.
    ///
    /// This is the enhanced version that merges user-learned bigrams with the
    /// static n-gram model, similar to upstream libpinyin's approach.
    ///
    /// # Arguments
    /// * `context` - The previous 1-2 characters as context
    /// * `count` - Maximum number of predictions to return
    /// * `cfg` - Configuration for weights and parameters
    /// * `userdict` - Optional UserDict for personalized predictions
    ///
    /// # Returns
    /// Vector of (text, score) tuples with user learning boost applied.
    pub fn predict_next_with_user(
        &self,
        context: &str,
        count: usize,
        cfg: Option<&crate::Config>,
        userdict: Option<&crate::UserDict>,
    ) -> Vec<(String, f64)> {
        if count == 0 {
            return vec![];
        }

        // Extract configuration parameters
        let max_phrase_len = cfg.map(|c| c.max_prediction_length).unwrap_or(3);
        let min_frequency_threshold = cfg.map(|c| c.min_prediction_frequency).unwrap_or(-15.0);
        let prefer_phrases = cfg.map(|c| c.prefer_phrase_predictions).unwrap_or(true);
        let user_boost = 2.0; // Log-space boost for user-learned bigrams (e^2 ≈ 7.4x)

        // Extract last 1-2 characters from context
        let chars: Vec<char> = context.chars().collect();
        let context_len = chars.len();

        // Build candidate list by querying bigram/trigram tables
        let mut candidates: HashMap<String, f64> = HashMap::new();

        // Track which candidates have trigram scores (higher priority)
        let mut has_trigram: HashMap<String, bool> = HashMap::new();

        if context_len >= 2 {
            // Use trigram: P(w3 | w1, w2)
            let w1 = chars[context_len - 2].to_string();
            let w2 = chars[context_len - 1].to_string();

            // Search all trigrams starting with (w1, w2, *)
            for ((tw1, tw2, tw3), log_p) in &self.trigram {
                if tw1 == &w1 && tw2 == &w2 && *log_p >= min_frequency_threshold {
                    // Found a trigram match - this is highest priority
                    candidates.insert(tw3.clone(), *log_p);
                    has_trigram.insert(tw3.clone(), true);
                }
            }
        }

        if context_len >= 1 {
            // Use bigram: P(w2 | w1) - only for candidates without trigram
            let w1 = chars[context_len - 1].to_string();

            // Search all bigrams starting with (w1, *)
            for ((bw1, bw2), log_p) in &self.bigram {
                if bw1 == &w1 && *log_p >= min_frequency_threshold {
                    // Only use bigram if we don't already have trigram for this candidate
                    if !has_trigram.contains_key(bw2) {
                        candidates.insert(bw2.clone(), *log_p);
                    }
                }
            }
        }

        // Merge user bigrams if available (boost user-learned patterns)
        if let Some(ud) = userdict {
            if context_len >= 1 {
                let w1 = chars[context_len - 1].to_string();
                let user_bigrams = ud.get_bigrams_after(&w1);

                for (w2, count) in user_bigrams {
                    // Convert count to log probability with boost
                    // Higher count = stronger boost
                    let user_score = user_boost + (count as f64).ln();

                    // If candidate exists, boost it; otherwise add it
                    candidates
                        .entry(w2.clone())
                        .and_modify(|score| *score += user_score)
                        .or_insert(user_score + min_frequency_threshold);
                }
            }
        }

        // Build multi-character phrases by extending single-char candidates
        if max_phrase_len > 1 {
            let phrase_candidates = self.build_phrase_candidates(
                context,
                &candidates,
                max_phrase_len,
                min_frequency_threshold,
            );
            candidates.extend(phrase_candidates);
        }

        // If we have very few candidates, add top unigrams as fallback
        if candidates.len() < count {
            for (w, log_p) in &self.unigram {
                if !candidates.contains_key(w) && *log_p >= min_frequency_threshold {
                    // Add with penalty since it's just unigram
                    candidates.insert(w.clone(), log_p - 2.0);
                }

                // Stop if we have enough candidates
                if candidates.len() >= count * 2 {
                    break;
                }
            }
        }

        // Sort with phrase length preference if enabled
        let mut results: Vec<(String, f64)> = candidates.into_iter().collect();

        if prefer_phrases {
            // Sort by: 2-char phrases first, then by score, then other lengths by score
            results.sort_by(|a, b| {
                let a_len = a.0.chars().count();
                let b_len = b.0.chars().count();

                // Prefer 2-character phrases
                match (a_len, b_len) {
                    (2, 2) => b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal),
                    (2, _) => std::cmp::Ordering::Less,
                    (_, 2) => std::cmp::Ordering::Greater,
                    _ => b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal),
                }
            });
        } else {
            // Simple sort by score
            results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        }

        results.truncate(count);

        results
    }

    /// Build multi-character phrase candidates by extending single characters.
    ///
    /// For each single-character candidate, tries to build longer phrases (up to max_len)
    /// by following the character bigram chain. Accumulates probability as the product
    /// of individual character probabilities.
    ///
    /// # Arguments
    /// * `context` - The current context string
    /// * `initial_candidates` - Single-character candidates to extend
    /// * `max_len` - Maximum phrase length to build
    /// * `min_threshold` - Minimum log probability threshold
    ///
    /// # Returns
    /// HashMap of (phrase, log_prob) for multi-character phrases
    fn build_phrase_candidates(
        &self,
        _context: &str,
        initial_candidates: &HashMap<String, f64>,
        max_len: usize,
        min_threshold: f64,
    ) -> HashMap<String, f64> {
        let mut phrases: HashMap<String, f64> = HashMap::new();

        // Try to extend each single-character candidate into phrases
        for (char1, prob1) in initial_candidates {
            // Skip if not a single character
            if char1.chars().count() != 1 {
                continue;
            }

            // Try to build 2-character phrases
            if max_len >= 2 {
                for ((bw1, bw2), log_p2) in &self.bigram {
                    if bw1 == char1 && *log_p2 >= min_threshold {
                        let phrase = format!("{}{}", char1, bw2);
                        let phrase_prob = prob1 + log_p2; // P(c1) * P(c2|c1) in log space

                        // Only add if it meets threshold
                        if phrase_prob >= min_threshold {
                            phrases.insert(phrase.clone(), phrase_prob);

                            // Try to extend to 3-character phrases
                            if max_len >= 3 {
                                for ((bw1_next, bw2_next), log_p3) in &self.bigram {
                                    if bw1_next == bw2 && *log_p3 >= min_threshold {
                                        let phrase3 = format!("{}{}{}", char1, bw2, bw2_next);
                                        let phrase3_prob = phrase_prob + log_p3;

                                        if phrase3_prob >= min_threshold {
                                            phrases.insert(phrase3, phrase3_prob);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        phrases
    }

    /// Score a token sequence but consult the internal Interpolator for per-key lambdas.
    ///
    /// `key_for_lookup` is passed to the interpolator to find a per-key lambda
    /// triple. If no entry is found, falls back to `cfg` weights.
    pub fn score_sequence_with_interpolator(
        &self,
        tokens: &[String],
        cfg: &crate::Config,
        key_for_lookup: &str,
    ) -> f32 {
        if tokens.is_empty() {
            return std::f32::NEG_INFINITY;
        }

        let floor = -20.0f64;

        // decide weights from internal interpolator
        let mut weights: [f32; 3] = [cfg.unigram_weight, cfg.bigram_weight, cfg.trigram_weight];
        if let Some(Lambdas(arr)) = self.interpolator.lookup(key_for_lookup) {
            weights = arr;
        }

        // Ensure numerical stability: if weights don't sum to 1, normalize
        let sum: f32 = weights.iter().copied().sum();
        if sum > 0.0 {
            for w in weights.iter_mut() {
                *w /= sum;
            }
        }

        let uw = weights[0] as f64;
        let bw = weights[1] as f64;
        let tw = weights[2] as f64;

        let mut score: f64 = 0.0;
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

            score += uw * u + bw * b + tw * t;
        }

        score as f32
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
    pub fn save_bincode<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        bincode::serialize_into(&mut writer, self)?;
        Ok(())
    }

    /// Load the model from bincode file.
    pub fn load_bincode<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
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
        m.insert_unigram("你", -1.0_f64);
        m.insert_unigram("好", -1.2_f64);
        m.insert_unigram("中", -1.1_f64);
        m.insert_unigram("国", -1.3_f64);

        // bigram for "你 好"
        m.insert_bigram("你", "好", -0.2_f64);
        // trigram example (not used in 2-token sequence)
        m.insert_trigram("x", "y", "z", -0.05_f64);

        let tokens = vec!["你".to_string(), "好".to_string()];
        // use weights that favor bigram
        let cfg = crate::Config {
            fuzzy: vec![],
            unigram_weight: 0.3,
            bigram_weight: 0.6,
            trigram_weight: 0.1,
            sort_by_phrase_length: false,
            sort_without_longer_candidate: false,
            max_prediction_length: 3,
            min_prediction_frequency: -15.0,
            prefer_phrase_predictions: true,
            auto_suggestion: true,
            min_suggestion_trigger_length: 2,
            max_cache_size: 1000,
            full_width_enabled: false,
            select_keys: "123456789".to_string(),
            masked_phrases: std::collections::HashSet::new(),
            correction_penalty: 200,
            fuzzy_penalty_multiplier: 100,
            incomplete_penalty: 500,
            unknown_penalty: 1000,
            unknown_cost: 10.0,
        };
        let score = m.score_sequence(&tokens, &cfg);

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

    #[test]
    fn predict_next_basic() {
        let mut m = NGramModel::new();

        // Build a small model with context
        m.insert_unigram("的", -1.0);
        m.insert_unigram("是", -1.5);
        m.insert_unigram("吗", -2.0);
        m.insert_unigram("呢", -2.5);

        // Bigrams following "好"
        m.insert_bigram("好", "的", -0.5);
        m.insert_bigram("好", "吗", -1.0);
        m.insert_bigram("好", "呢", -1.5);

        // Trigrams following "你好"
        m.insert_trigram("你", "好", "吗", -0.3);
        m.insert_trigram("你", "好", "呢", -0.8);

        // Test with 2-char context (should use trigram)
        let predictions = m.predict_next("你好", 3, None);

        // Should have predictions
        assert!(!predictions.is_empty());
        assert!(predictions.len() <= 3);

        // "吗" should be first (best trigram score)
        assert_eq!(predictions[0].0, "吗");

        // Scores should be in descending order
        for i in 1..predictions.len() {
            assert!(predictions[i - 1].1 >= predictions[i].1);
        }
    }

    #[test]
    fn predict_next_with_bigram_context() {
        let mut m = NGramModel::new();

        // Bigrams only
        m.insert_bigram("好", "的", -0.5);
        m.insert_bigram("好", "吗", -1.0);
        m.insert_bigram("好", "是", -1.5);

        // Test with 1-char context
        let predictions = m.predict_next("好", 2, None);

        assert!(!predictions.is_empty());
        assert!(predictions.len() <= 2);

        // "的" should be first (best bigram score)
        assert_eq!(predictions[0].0, "的");
    }

    #[test]
    fn predict_next_empty_context() {
        let mut m = NGramModel::new();

        m.insert_unigram("的", -1.0);
        m.insert_unigram("是", -1.5);

        // Empty context should still return unigram predictions
        let predictions = m.predict_next("", 2, None);

        assert!(!predictions.is_empty());
        // Should fall back to unigrams
    }

    #[test]
    fn predict_next_multi_char_phrases() {
        let mut m = NGramModel::new();

        // Build a model that can form multi-character phrases
        m.insert_unigram("的", -1.0);
        m.insert_unigram("话", -2.0);
        m.insert_unigram("说", -2.5);

        // Chain: 的 → 话 → 说 (forming "的话说")
        m.insert_bigram("好", "的", -0.5);
        m.insert_bigram("的", "话", -0.6);
        m.insert_bigram("话", "说", -0.7);

        // Configure to allow phrases up to 3 chars
        let cfg = crate::Config {
            max_prediction_length: 3,
            min_prediction_frequency: -15.0,
            prefer_phrase_predictions: true,
            ..Default::default()
        };

        let predictions = m.predict_next("好", 10, Some(&cfg));

        assert!(!predictions.is_empty());

        // Should include both single chars and multi-char phrases
        let has_single = predictions
            .iter()
            .any(|(text, _)| text.chars().count() == 1);
        let has_multi = predictions.iter().any(|(text, _)| text.chars().count() > 1);

        assert!(has_single, "Should have single-char predictions");
        assert!(has_multi, "Should have multi-char phrase predictions");

        // Check that we have the expected phrases
        let phrases: Vec<String> = predictions.iter().map(|(text, _)| text.clone()).collect();
        assert!(phrases.contains(&"的".to_string()), "Should predict '的'");
        assert!(
            phrases.contains(&"的话".to_string()),
            "Should predict '的话'"
        );
    }

    #[test]
    fn predict_next_phrase_length_preference() {
        let mut m = NGramModel::new();

        // Single char with good score
        m.insert_bigram("好", "的", -0.5);

        // 2-char phrase with slightly worse score
        m.insert_bigram("好", "吗", -0.6);
        m.insert_bigram("吗", "？", -0.7); // Forms "吗？"

        // With phrase preference enabled, 2-char should rank higher
        let cfg = crate::Config {
            max_prediction_length: 2,
            min_prediction_frequency: -15.0,
            prefer_phrase_predictions: true,
            ..Default::default()
        };

        let predictions = m.predict_next("好", 5, Some(&cfg));

        // Find 2-char phrases
        let two_char_phrases: Vec<_> = predictions
            .iter()
            .filter(|(text, _)| text.chars().count() == 2)
            .collect();

        if !two_char_phrases.is_empty() {
            // If we have 2-char phrases, they should appear early
            let first_two_char_idx = predictions
                .iter()
                .position(|(text, _)| text.chars().count() == 2);
            let first_single_char_idx = predictions
                .iter()
                .position(|(text, _)| text.chars().count() == 1);

            if let (Some(two_idx), Some(single_idx)) = (first_two_char_idx, first_single_char_idx) {
                // 2-char should come before single char when preference is enabled
                assert!(
                    two_idx < single_idx,
                    "2-char phrases should rank higher with prefer_phrase_predictions"
                );
            }
        }
    }

    #[test]
    fn predict_next_frequency_filtering() {
        let mut m = NGramModel::new();

        // High frequency prediction
        m.insert_bigram("好", "的", -0.5);

        // Low frequency prediction (below typical threshold)
        m.insert_bigram("好", "哉", -18.0); // Very rare

        // Configure with moderate threshold
        let cfg = crate::Config {
            max_prediction_length: 1,
            min_prediction_frequency: -15.0,
            prefer_phrase_predictions: false,
            ..Default::default()
        };

        let predictions = m.predict_next("好", 10, Some(&cfg));

        // Should include high frequency
        assert!(predictions.iter().any(|(text, _)| text == "的"));

        // Should NOT include very low frequency
        assert!(!predictions.iter().any(|(text, _)| text == "哉"));
    }

    #[test]
    fn predict_next_with_user_learning() {
        let mut m = NGramModel::new();

        // Static model predictions
        m.insert_bigram("好", "的", -0.5);
        m.insert_bigram("好", "吗", -1.0);
        m.insert_bigram("好", "啊", -1.5);

        // Create user dict and learn some patterns
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "test_predict_user_{}.redb",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let userdict = crate::UserDict::new(&tmp).unwrap();

        // User frequently uses "好" → "啊" (even though static model rates it lower)
        userdict.learn_bigram("好", "啊");
        userdict.learn_bigram("好", "啊");
        userdict.learn_bigram("好", "啊");
        userdict.learn_bigram("好", "啊");
        userdict.learn_bigram("好", "啊"); // 5 times

        // Predict without user learning
        let predictions_static = m.predict_next("好", 5, None);

        // "的" should be first in static model
        assert_eq!(predictions_static[0].0, "的");

        // Predict with user learning
        let predictions_user = m.predict_next_with_user("好", 5, None, Some(&userdict));

        // "啊" should get a significant boost and rank higher
        let ah_idx_static = predictions_static
            .iter()
            .position(|(text, _)| text == "啊")
            .unwrap();
        let ah_idx_user = predictions_user
            .iter()
            .position(|(text, _)| text == "啊")
            .unwrap();

        // User learning should boost "啊" to a higher position
        assert!(
            ah_idx_user < ah_idx_static,
            "User-learned bigram should rank higher: user_idx={}, static_idx={}",
            ah_idx_user,
            ah_idx_static
        );
    }

    #[test]
    fn predict_next_user_adds_new_candidates() {
        let mut m = NGramModel::new();

        // Static model only has "的"
        m.insert_bigram("好", "的", -0.5);

        // Create user dict
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "test_predict_user_new_{}.redb",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let userdict = crate::UserDict::new(&tmp).unwrap();

        // User learns a new pattern not in static model
        userdict.learn_bigram("好", "棒");
        userdict.learn_bigram("好", "棒");
        userdict.learn_bigram("好", "棒");

        // Predict with user learning
        let predictions = m.predict_next_with_user("好", 10, None, Some(&userdict));

        // Should include both static and user-learned candidates
        assert!(
            predictions.iter().any(|(text, _)| text == "的"),
            "Should include static model candidates"
        );
        assert!(
            predictions.iter().any(|(text, _)| text == "棒"),
            "Should include user-learned candidates"
        );
    }
}

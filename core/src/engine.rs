// core/src/engine.rs
//
// Generic IME engine that works with any syllable parser.
// This eliminates code duplication between libpinyin and libzhuyin.

use crate::{Candidate, Model};
use std::cell::RefCell;
use std::collections::HashMap;

/// Trait that syllable parsers must implement to work with the generic Engine.
pub trait SyllableParser {
    /// The syllable type this parser produces (e.g., Syllable, ZhuyinSyllable)
    type Syllable: SyllableType;

    /// Segment input into top-k best syllable sequences
    fn segment_top_k(&self, input: &str, k: usize, allow_fuzzy: bool) -> Vec<Vec<Self::Syllable>>;
}

/// Trait for syllable types that engines can work with.
pub trait SyllableType {
    /// Get the text of this syllable (e.g., "ni", "hao", "ㄋㄧˇ")
    fn text(&self) -> &str;

    /// Whether this syllable was matched via fuzzy matching
    fn is_fuzzy(&self) -> bool;
}

/// Generic IME engine that combines parser and model for candidate generation.
///
/// Type parameter P is the parser type (e.g., Parser for pinyin, ZhuyinParser for zhuyin).
///
/// Note: Fuzzy matching is handled by the parser during segmentation. The engine
/// works with the segmentations provided by the parser.
pub struct Engine<P> {
    model: Model,
    parser: P,
    limit: usize,
    cache: RefCell<lru::LruCache<String, Vec<Candidate>>>,
    cache_hits: RefCell<usize>,
    cache_misses: RefCell<usize>,
}

impl<P: SyllableParser> Engine<P> {
    /// Create a new engine with the given model and parser.
    pub fn new(model: Model, parser: P) -> Self {
        let cache_capacity = model.config.borrow().max_cache_size;

        Self {
            model,
            parser,
            limit: 8,
            cache: RefCell::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(cache_capacity)
                    .unwrap_or(std::num::NonZeroUsize::new(1000).unwrap()),
            )),
            cache_hits: RefCell::new(0),
            cache_misses: RefCell::new(0),
        }
    }

    /// Process input and return ranked candidates.
    ///
    /// This implements the full IME pipeline:
    /// 1. Check cache for previous result
    /// 2. Parse input into syllable segmentations (parser handles fuzzy matching)
    /// 3. For each segmentation:
    ///    - Convert to lexicon key
    ///    - Look up candidates in lexicon
    ///    - Apply penalty if segmentation used fuzzy matching
    /// 4. Merge and rank candidates
    /// 5. Cache the result
    pub fn input(&self, input: &str) -> Vec<Candidate> {
        // Check cache first (LRU automatically updates access time)
        if let Some(cached) = self.cache.borrow_mut().get(&input.to_string()) {
            *self.cache_hits.borrow_mut() += 1;
            return cached.clone();
        }

        *self.cache_misses.borrow_mut() += 1;

        // Get top segmentations from parser (parser already applied fuzzy matching)
        // Use an adaptive k computed from Config and input length to balance
        // recall vs CPU work. Parser internally uses dynamic beam width scaling
        // (see parser.rs:840-842) so k has a non-linear effect on parser cost.
        let input_len = input.len();
        let cfg = self.model.config.borrow();
        let short_k = cfg.segment_k_short;
        let long_k = cfg.segment_k_long;
        let max_k = cfg.segment_k_max;
        drop(cfg);

        // Heuristic: keep small inputs low, increase gradually for longer inputs,
        // but clamp to a max. This mirrors upstream piecewise/proportional rules.
        let k = if input_len <= 6 {
            short_k
        } else {
            // add one extra segmentation per ~4 extra chars beyond 6
            let extra = (input_len.saturating_sub(6)) / 4;
            let computed = long_k.saturating_add(extra);
            std::cmp::min(computed, max_k)
        };

        let segs = self.parser.segment_top_k(input, k, true);

        // Map from phrase -> best Candidate (keep highest score)
        let mut best: HashMap<String, Candidate> = HashMap::new();

        for seg in segs.into_iter() {
            // For each segmentation, generate candidates by trying all possible word boundaries
            // e.g., [ni,hao,wo,shi] can be: "你好"+"我是", "你"+"好"+"我是", etc.
            let candidates = self.generate_candidates_from_segmentation(&seg);

            // Merge candidates: keep the best score seen for this exact phrase
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

        // Filter out masked phrases
        let config = self.model.config.borrow();
        if !config.masked_phrases.is_empty() {
            vec.retain(|c| !config.is_masked(&c.text));
        }
        drop(config);

        // Apply advanced ranking options from config
        vec = self.apply_advanced_ranking(vec, input);

        // Final sort by score (primary key) and phrase length (secondary, if enabled)
        self.sort_candidates(&mut vec);

        if vec.len() > self.limit {
            vec.truncate(self.limit);
        }

        // Cache the result (LRU automatically handles eviction)
        self.cache.borrow_mut().put(input.to_string(), vec.clone());

        vec
    }

    /// Apply advanced ranking options based on Config settings.
    ///
    /// Implements upstream libpinyin sort_option_t behavior:
    /// - SORT_BY_PHRASE_LENGTH: Prefer shorter phrases (adjusts score)
    /// - SORT_BY_PINYIN_LENGTH: Prefer shorter pinyin (adjusts score)
    /// - SORT_WITHOUT_LONGER_CANDIDATE: Filter out phrases longer than input
    fn apply_advanced_ranking(
        &self,
        mut candidates: Vec<Candidate>,
        input: &str,
    ) -> Vec<Candidate> {
        let cfg = self.model.config.borrow();

        // Filter: Remove candidates longer than input
        if cfg.sort_without_longer_candidate {
            let input_char_count = input.chars().count();
            candidates.retain(|c| {
                let phrase_char_count = c.text.chars().count();
                phrase_char_count <= input_char_count
            });
        }

        // Adjust scores based on phrase length preference
        if cfg.sort_by_phrase_length {
            drop(cfg); // Release borrow before mutable iteration
            for cand in candidates.iter_mut() {
                let phrase_len = cand.text.chars().count();
                // Penalize each extra character beyond 1
                let length_penalty = (phrase_len.saturating_sub(1)) as f32 * 0.5;
                cand.score -= length_penalty;
            }
        }

        candidates
    }

    /// Sort candidates by score (primary) and optionally by phrase length (secondary).
    fn sort_candidates(&self, candidates: &mut [Candidate]) {
        let sort_by_length = self.model.config.borrow().sort_by_phrase_length;

        candidates.sort_by(|a, b| {
            // Primary: score (higher is better)
            match b.score.partial_cmp(&a.score) {
                Some(std::cmp::Ordering::Equal) if sort_by_length => {
                    // Secondary: phrase length (shorter is better)
                    let a_len = a.text.chars().count();
                    let b_len = b.text.chars().count();
                    a_len.cmp(&b_len)
                }
                ordering => ordering.unwrap_or(std::cmp::Ordering::Equal),
            }
        });
    }

    /// Commit a phrase to user learning.
    ///
    /// Records user selection to boost future rankings.
    /// Clears cache to reflect updated frequencies immediately.
    pub fn commit(&self, phrase: &str) {
        // Learn the phrase in the user dictionary (increments frequency by 1)
        self.model.userdict.learn(phrase);

        // Clear cache so updated frequencies are reflected immediately
        self.clear_cache();
    }

    /// Generate candidates from a segmentation by trying all possible word combinations.
    ///
    /// Uses dynamic programming to find valid word sequences that cover the entire segmentation.
    /// For each valid word sequence, looks up candidates and scores them.
    fn generate_candidates_from_segmentation(&self, seg: &[P::Syllable]) -> Vec<Candidate> {
        let n = seg.len();
        if n == 0 {
            return Vec::new();
        }

        // Result accumulator
        let mut results: Vec<Candidate> = Vec::new();

        // First: try the FULL segmentation as a single lexicon key (supports long dictionary entries)
        let full_key = seg
            .iter()
            .map(|s| s.text())
            .collect::<Vec<&str>>()
            .join("'");
        let full_entries = self.model.lexicon.lookup_with_freq(&full_key);
        if !full_entries.is_empty() {
            // Score full-key matches using the same word-level unigram/bigram scoring as DP paths
            for (phrase, _) in full_entries.into_iter() {
                let config = self.model.config.borrow();

                // Get unigram probability from word_bigram model (from interpolation2.text)
                let unigram_prob = self.model.word_bigram.get_unigram_probability(&phrase);

                let lambda = config.lambda;
                let sentence_length_penalty = config.sentence_length_penalty;
                let unigram_factor = config.unigram_factor;
                let full_key_boost = config.full_key_boost;
                drop(config);

                // For full-key matches, we have no context (start of sentence)
                // Use pure unigram: log(P(w) * unigram_lambda)
                let safe_prob = (unigram_prob * (1.0 - lambda)).max(1e-10);
                let mut score = safe_prob.ln();

                // Apply sentence length penalty (one word)
                score -= sentence_length_penalty;

                // Userdict boost
                let user_freq = self.model.userdict.frequency(&phrase);
                if user_freq > 0 {
                    score += unigram_factor * (1.0 + (user_freq as f32)).ln();
                }

                // Apply full-key boost to prefer exact dictionary matches
                score += full_key_boost;

                results.push(Candidate::new(phrase, score));
            }
            // If a full dictionary match exists, include it but continue to also try composed variants
        }

        // DP: best_path[i] = best candidate sequence covering syllables [0..i)
        // Each entry is a Vec of (phrase, score) tuples
        let mut best_path: Vec<Option<Vec<(String, f32)>>> = vec![None; n + 1];
        best_path[0] = Some(Vec::new()); // empty path at start

        // Maximum short-word length to compose cheaply; longer lengths will only be tried if an exact lexicon lookup exists
        const MAX_SHORT_SYLLABLES: usize = 4;
        const MAX_LONG_LOOKUP_SYLLABLES: usize = 10; // allow occasional long-word lookups if present in lexicon

        // A small per-input cache for existence checks to avoid repeated FST probes
        let mut existence_cache: std::collections::HashMap<String, bool> =
            std::collections::HashMap::new();

        // Try all possible word lengths at each position
        for i in 0..n {
            if best_path[i].is_none() {
                continue; // no valid path to position i
            }

            // First, cheap composition for common shorter words
            for len in 1..=std::cmp::min(MAX_SHORT_SYLLABLES, n - i) {
                // Build lexicon key for syllables [i..i+len)
                let word_key: String = seg[i..i + len]
                    .iter()
                    .map(|s| s.text())
                    .collect::<Vec<&str>>()
                    .join("'");

                // Look up this word in lexicon with frequencies
                let candidates = self.model.lexicon.lookup_with_freq(&word_key);

                for (word_text, _) in candidates {
                    // Use word-level unigram/bigram scoring (matching upstream libpinyin)
                    // Upstream formula: log((λ * P(w2|w1) + (1-λ) * P(w2)) * P(pinyin)) - sentence_length_penalty
                    // Sentence length penalty discourages over-segmentation

                    let config = self.model.config.borrow();

                    // Get unigram probability from word_bigram model (from interpolation2.text)
                    // This is the correct P(w2) for the interpolation formula
                    let unigram_prob = self.model.word_bigram.get_unigram_probability(&word_text);

                    let lambda = config.lambda;
                    let sentence_length_penalty = config.sentence_length_penalty;
                    let unigram_factor = config.unigram_factor;
                    drop(config);

                    let mut word_score: f32;

                    let current_path = best_path[i].as_ref().unwrap();
                    if let Some((prev_word, _)) = current_path.last() {
                        // We have context: use interpolated bigram
                        // Upstream: log((bigram_lambda * P(w2|w1) + unigram_lambda * P(w2)) * pinyin_poss)
                        let bigram_prob = self
                            .model
                            .word_bigram
                            .get_probability(prev_word, &word_text);
                        let interpolated_prob =
                            lambda * bigram_prob + (1.0 - lambda) * unigram_prob;
                        let safe_prob = interpolated_prob.max(1e-10);
                        word_score = safe_prob.ln();
                    } else {
                        // No context: use pure unigram with lambda scaling
                        // Upstream: log(P(w) * pinyin_poss * unigram_lambda)
                        let safe_prob = (unigram_prob * (1.0 - lambda)).max(1e-10);
                        word_score = safe_prob.ln();
                    }

                    // Apply sentence length penalty (upstream LONG_SENTENCE_PENALTY)
                    // This discourages paths with many words
                    word_score -= sentence_length_penalty;

                    // Userdict boost: upstream modifies lexicon frequencies directly with unigram_factor
                    // We use a separate userdict, so multiply by unigram_factor to match upstream effect
                    let user_freq = self.model.userdict.frequency(&word_text);
                    if user_freq > 0 {
                        let boost = unigram_factor * (1.0 + (user_freq as f32)).ln();
                        word_score += boost;
                    }

                    let mut new_path = current_path.clone();
                    new_path.push((word_text, word_score));

                    // Update best_path[i+len] if this is better
                    let new_end = i + len;
                    match &best_path[new_end] {
                        None => {
                            best_path[new_end] = Some(new_path);
                        }
                        Some(existing) => {
                            let new_total: f32 = new_path.iter().map(|(_, s)| s).sum();
                            let existing_total: f32 = existing.iter().map(|(_, s)| s).sum();
                            if new_total > existing_total {
                                best_path[new_end] = Some(new_path);
                            }
                        }
                    }
                }
            }

            // Additionally, probe for longer exact lexicon keys (rare but supported)
            for len in (MAX_SHORT_SYLLABLES + 1)..=std::cmp::min(MAX_LONG_LOOKUP_SYLLABLES, n - i) {
                let long_key: String = seg[i..i + len]
                    .iter()
                    .map(|s| s.text())
                    .collect::<Vec<&str>>()
                    .join("'");

                // Cheap existence check first (avoid deserializing payloads)
                let exists = *existence_cache
                    .entry(long_key.clone())
                    .or_insert_with(|| self.model.lexicon.has_key(&long_key));
                if !exists {
                    continue; // skip expensive processing when nothing exists
                }
                let long_candidates = self.model.lexicon.lookup_with_freq(&long_key);

                for (word_text, _) in long_candidates {
                    // Use word-level unigram/bigram scoring (matching upstream)
                    let config = self.model.config.borrow();

                    // Get unigram probability from word_bigram model (from interpolation2.text)
                    let unigram_prob = self.model.word_bigram.get_unigram_probability(&word_text);

                    let lambda = config.lambda;
                    let sentence_length_penalty = config.sentence_length_penalty;
                    let unigram_factor = config.unigram_factor;
                    drop(config);

                    let mut word_score: f32;

                    let current_path = best_path[i].as_ref().unwrap();
                    if let Some((prev_word, _)) = current_path.last() {
                        // Interpolated bigram scoring
                        let bigram_prob = self
                            .model
                            .word_bigram
                            .get_probability(prev_word, &word_text);
                        let interpolated_prob =
                            lambda * bigram_prob + (1.0 - lambda) * unigram_prob;
                        let safe_prob = interpolated_prob.max(1e-10);
                        word_score = safe_prob.ln();
                    } else {
                        // Pure unigram with lambda scaling
                        let safe_prob = (unigram_prob * (1.0 - lambda)).max(1e-10);
                        word_score = safe_prob.ln();
                    }

                    // Apply sentence length penalty (upstream LONG_SENTENCE_PENALTY)
                    word_score -= sentence_length_penalty;

                    // Userdict boost: use unigram_factor from config to match upstream
                    let user_freq = self.model.userdict.frequency(&word_text);
                    if user_freq > 0 {
                        word_score += unigram_factor * (1.0 + (user_freq as f32)).ln();
                    }

                    let mut new_path = current_path.clone();
                    new_path.push((word_text, word_score));

                    let new_end = i + len;
                    match &best_path[new_end] {
                        None => best_path[new_end] = Some(new_path),
                        Some(existing) => {
                            let new_total: f32 = new_path.iter().map(|(_, s)| s).sum();
                            let existing_total: f32 = existing.iter().map(|(_, s)| s).sum();
                            if new_total > existing_total {
                                best_path[new_end] = Some(new_path);
                            }
                        }
                    }
                }
            }
        }

        // Extract candidates from the best path that reaches the end and include them
        if let Some(final_path) = &best_path[n] {
            let full_text: String = final_path.iter().map(|(t, _)| t.as_str()).collect();
            let total_score: f32 = final_path.iter().map(|(_, s)| s).sum();
            results.push(Candidate::new(full_text, total_score));
        }

        results
    }

    /// Get cache statistics for monitoring.
    ///
    /// Returns (hits, misses) tuple.
    pub fn cache_stats(&self) -> (usize, usize) {
        (*self.cache_hits.borrow(), *self.cache_misses.borrow())
    }

    /// Get cache hit rate as a percentage (0.0 to 100.0).
    ///
    /// Returns None if no cache accesses have been made yet.
    pub fn cache_hit_rate(&self) -> Option<f32> {
        let hits = *self.cache_hits.borrow();
        let misses = *self.cache_misses.borrow();
        let total = hits + misses;

        if total == 0 {
            None
        } else {
            Some((hits as f32 / total as f32) * 100.0)
        }
    }

    /// Get current cache size (number of entries).
    pub fn cache_size(&self) -> usize {
        self.cache.borrow().len()
    }

    /// Get cache capacity (maximum entries).
    pub fn cache_capacity(&self) -> usize {
        self.cache.borrow().cap().get()
    }

    /// Clear the cache (useful for testing or memory management).
    pub fn clear_cache(&self) {
        self.cache.borrow_mut().clear();
        *self.cache_hits.borrow_mut() = 0;
        *self.cache_misses.borrow_mut() = 0;
    }

    /// Get reference to the user dictionary.
    ///
    /// Provides access to user-learned data including user bigrams
    /// for personalized predictions.
    pub fn userdict(&self) -> &crate::UserDict {
        &self.model.userdict
    }

    /// Get reference to the model.
    ///
    /// Provides access to lexicon, word_bigram, and other model components.
    pub fn model(&self) -> &crate::Model {
        &self.model
    }

    /// Get reference to the configuration.
    pub fn config(&self) -> std::cell::Ref<'_, crate::Config> {
        self.model.config.borrow()
    }

    /// Get mutable reference to the configuration.
    pub fn config_mut(&self) -> std::cell::RefMut<'_, crate::Config> {
        self.model.config.borrow_mut()
    }
}

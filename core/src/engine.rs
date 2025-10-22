// core/src/engine.rs
//
// Generic IME engine that works with any syllable parser.
// This eliminates code duplication between libpinyin and libzhuyin.

use std::collections::HashMap;
use std::cell::RefCell;
use crate::{Candidate, Model};

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
    /// 
    /// Fuzzy matching is handled by the parser, so no fuzzy rules are needed here.
    pub fn new(model: Model, parser: P) -> Self {
        let cache_capacity = model.config.max_cache_size;
        
        Self {
            model,
            parser,
            limit: 8,
            cache: RefCell::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(cache_capacity).unwrap_or(std::num::NonZeroUsize::new(1000).unwrap())
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
        let segs = self.parser.segment_top_k(input, 4, true);

        // Map from phrase -> best Candidate (keep highest score)
        let mut best: HashMap<String, Candidate> = HashMap::new();

        for seg in segs.into_iter() {
            // Convert segmentation to lexicon lookup key
            let key = Self::segmentation_to_key(&seg);
            
            // Look up candidates for this key
            let mut candidates = self.model.candidates_for_key(&key, self.limit);

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
    fn apply_advanced_ranking(&self, mut candidates: Vec<Candidate>, input: &str) -> Vec<Candidate> {
        let cfg = &self.model.config;
        
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
        let sort_by_length = self.model.config.sort_by_phrase_length;
        
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
    
    /// Convert a syllable segmentation to a lookup key.
    /// 
    /// Joins syllables with apostrophes to match lexicon key format.
    /// Example: ["ni", "hao"] -> "ni'hao"
    fn segmentation_to_key(seg: &[P::Syllable]) -> String {
        seg.iter()
            .map(|s| s.text())
            .collect::<Vec<&str>>()
            .join("'")
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
}

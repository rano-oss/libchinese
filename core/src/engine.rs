// core/src/engine.rs
//
// Generic IME engine that works with any syllable parser.
// This eliminates code duplication between libpinyin and libzhuyin.

use std::collections::HashMap;
use std::cell::RefCell;
use crate::{Candidate, Model, FuzzyMap};

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

/// Generic IME engine that combines parser, model, and fuzzy matching.
///
/// Type parameter P is the parser type (e.g., Parser for pinyin, ZhuyinParser for zhuyin).
pub struct Engine<P> {
    model: Model,
    parser: P,
    fuzzy: FuzzyMap,
    limit: usize,
    cache: RefCell<HashMap<String, Vec<Candidate>>>,
    cache_hits: RefCell<usize>,
    cache_misses: RefCell<usize>,
}

impl<P: SyllableParser> Engine<P> {
    /// Create a new engine with the given model, parser, and fuzzy rules.
    pub fn new(model: Model, parser: P, fuzzy_rules: Vec<String>) -> Self {
        let fuzzy = FuzzyMap::from_rules(&fuzzy_rules);
        
        Self {
            model,
            parser,
            fuzzy,
            limit: 8,
            cache: RefCell::new(HashMap::new()),
            cache_hits: RefCell::new(0),
            cache_misses: RefCell::new(0),
        }
    }
    
    /// Process input and return ranked candidates.
    ///
    /// This implements the full IME pipeline:
    /// 1. Check cache for previous result
    /// 2. Parse input into syllable segmentations
    /// 3. For each segmentation:
    ///    - Generate fuzzy key alternatives
    ///    - Look up candidates in lexicon
    ///    - Apply fuzzy penalties
    /// 4. Merge and rank candidates
    /// 5. Cache the result
    pub fn input(&self, input: &str) -> Vec<Candidate> {
        // Check cache first
        if let Some(cached) = self.cache.borrow().get(input) {
            *self.cache_hits.borrow_mut() += 1;
            return cached.clone();
        }

        *self.cache_misses.borrow_mut() += 1;

        // Get top segmentations (k best)
        let segs = self.parser.segment_top_k(input, 4, true);

        // Map from phrase -> best Candidate (keep highest score)
        let mut best: HashMap<String, Candidate> = HashMap::new();

        for seg in segs.into_iter() {
            // Generate fuzzy alternative keys for this segmentation
            let keys_to_try = self.generate_fuzzy_key_alternatives(&seg);

            // Try all alternative keys (exact + fuzzy alternatives)
            let mut candidates = Vec::new();
            for (i, alt_key) in keys_to_try.iter().enumerate() {
                let mut key_candidates = self.model.candidates_for_key(alt_key, self.limit);
                
                // Apply fuzzy penalty if this is not the original key (index 0 is always original)
                if i > 0 {
                    let penalty = self.fuzzy.default_penalty();
                    for c in key_candidates.iter_mut() {
                        c.score -= penalty;
                    }
                }
                
                candidates.append(&mut key_candidates);
            }

            // If this segmentation used parser-level fuzzy matches, apply additional penalty
            let used_parser_fuzzy = seg.iter().any(|s| s.is_fuzzy());
            if used_parser_fuzzy {
                let penalty = self.fuzzy.default_penalty();
                for c in candidates.iter_mut() {
                    c.score -= penalty;
                }
            }

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

        // Cache the result
        let cache_size_limit = 1000;
        let mut cache = self.cache.borrow_mut();
        if cache.len() >= cache_size_limit {
            cache.clear();
        }
        cache.insert(input.to_string(), vec.clone());

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
        
        // Adjust scores based on length preferences
        if cfg.sort_by_phrase_length || cfg.sort_by_pinyin_length {
            for cand in candidates.iter_mut() {
                let phrase_len = cand.text.chars().count();
                
                // Apply phrase length penalty (longer = lower score)
                if cfg.sort_by_phrase_length {
                    // Penalize each extra character beyond 1
                    let length_penalty = (phrase_len.saturating_sub(1)) as f32 * 0.5;
                    cand.score -= length_penalty;
                }
                
                // Apply pinyin length penalty
                // Note: In full implementation, this would use the actual pinyin length
                // For now, we approximate with phrase length as a proxy
                if cfg.sort_by_pinyin_length {
                    let pinyin_len_estimate = phrase_len; // Simplified
                    let length_penalty = (pinyin_len_estimate.saturating_sub(1)) as f32 * 0.3;
                    cand.score -= length_penalty;
                }
            }
        }
        
        candidates
    }
    
    /// Sort candidates by score (primary) and optionally by phrase length (secondary).
    fn sort_candidates(&self, candidates: &mut [Candidate]) {
        let sort_by_length = self.model.config.sort_by_phrase_length || self.model.config.sort_by_pinyin_length;
        
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
    /// This records that the user selected the given phrase, incrementing its
    /// frequency in the user dictionary. This enables the IME to learn user
    /// preferences over time.
    ///
    /// After committing, the cache is cleared to ensure updated frequencies
    /// are reflected in future candidate rankings.
    ///
    /// # Arguments
    /// * `phrase` - The phrase text that the user selected
    ///
    /// # Example
    /// ```no_run
    /// # use libchinese_core::{Engine, Model, Lexicon, NGramModel, UserDict, Config, Interpolator};
    /// # use libchinese_core::engine::{SyllableParser, SyllableType};
    /// # struct DummyParser;
    /// # struct DummySyllable(String);
    /// # impl SyllableType for DummySyllable {
    /// #     fn text(&self) -> &str { &self.0 }
    /// #     fn is_fuzzy(&self) -> bool { false }
    /// # }
    /// # impl SyllableParser for DummyParser {
    /// #     type Syllable = DummySyllable;
    /// #     fn segment_top_k(&self, _: &str, _: usize, _: bool) -> Vec<Vec<Self::Syllable>> { vec![] }
    /// # }
    /// # let model = Model::new(
    /// #     Lexicon::new(),
    /// #     NGramModel::new(),
    /// #     UserDict::new(":memory:").unwrap(),
    /// #     Config::default(),
    /// #     Interpolator::empty_for_test(),
    /// # );
    /// # let engine = Engine::new(model, DummyParser, vec![]);
    /// // User selects a phrase
    /// let candidates = engine.input("nihao");
    /// let selected = &candidates[0].text;
    ///
    /// // Record the selection for learning
    /// engine.commit(selected);
    /// ```
    pub fn commit(&self, phrase: &str) {
        // Learn the phrase in the user dictionary (increments frequency by 1)
        self.model.userdict.learn(phrase);
        
        // Clear cache so updated frequencies are reflected immediately
        self.clear_cache();
    }
    
    /// Generate all fuzzy key alternatives for a syllable segmentation.
    ///
    /// Returns a Vec where the first element is the original key,
    /// followed by all fuzzy alternatives.
    fn generate_fuzzy_key_alternatives(&self, segmentation: &[P::Syllable]) -> Vec<String> {
        let mut alternatives = Vec::new();
        
        // Generate all combinations including the original
        self.generate_combinations_recursive(segmentation, 0, String::new(), &mut alternatives);
        
        // Ensure original key is first (for penalty calculation)
        let original_key = Self::segmentation_to_key(segmentation);
        let mut unique_alternatives = vec![original_key.clone()];
        
        // Add other alternatives (deduplicated)
        for alt in alternatives {
            if alt != original_key && !unique_alternatives.contains(&alt) {
                unique_alternatives.push(alt);
            }
        }
        
        unique_alternatives
    }
    
    /// Recursively generate all combinations of syllable alternatives.
    fn generate_combinations_recursive(
        &self, 
        segmentation: &[P::Syllable], 
        position: usize, 
        current: String, 
        results: &mut Vec<String>
    ) {
        if position >= segmentation.len() {
            if !current.is_empty() {
                results.push(current);
            }
            return;
        }
        
        let syllable = &segmentation[position];
        
        // Get alternatives for this syllable from fuzzy map
        let alternatives = self.fuzzy.alternative_strings(syllable.text());
        
        // For each alternative, recurse to the next position
        for alt in alternatives {
            let new_current = format!("{}{}", current, alt);
            self.generate_combinations_recursive(segmentation, position + 1, new_current, results);
        }
    }

    /// Convert a syllable segmentation to a lookup key.
    fn segmentation_to_key(seg: &[P::Syllable]) -> String {
        seg.iter()
            .map(|s| s.text())
            .collect::<Vec<&str>>()
            .join("")
    }
    
    /// Get cache statistics for monitoring.
    pub fn cache_stats(&self) -> (usize, usize) {
        (*self.cache_hits.borrow(), *self.cache_misses.borrow())
    }
    
    /// Clear the cache (useful for testing or memory management).
    pub fn clear_cache(&self) {
        self.cache.borrow_mut().clear();
    }
}

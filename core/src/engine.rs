// core/src/engine.rs
//
// Generic IME engine that works with any syllable parser.
// This eliminates code duplication between libpinyin and libzhuyin.

use crate::{Candidate, EmojiDict, EnglishDict, Model};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Trait that syllable parsers must implement to work with the generic Engine.
pub trait SyllableParser {
    /// The syllable type this parser produces (e.g., Syllable, ZhuyinSyllable)
    type Syllable: SyllableType;

    /// Segment input into top-k best syllable sequences
    fn segment_top_k(&self, input: &str, k: usize, allow_fuzzy: bool) -> Vec<Vec<Self::Syllable>>;

    /// Like `segment_top_k`, with an optional double-pinyin scheme name.
    /// Default ignores the scheme (full phonetic input).
    fn segment_top_k_with_scheme(
        &self,
        input: &str,
        k: usize,
        allow_fuzzy: bool,
        _scheme: Option<&str>,
    ) -> Vec<Vec<Self::Syllable>> {
        self.segment_top_k(input, k, allow_fuzzy)
    }
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
    emoji_dict: Option<EmojiDict>,
    emoji_enabled: RefCell<bool>,
    english_dict: Option<EnglishDict>,
    english_enabled: RefCell<bool>,
    /// When true, prefer candidates whose matching key/syllable span is shorter.
    sort_by_pinyin_length: RefCell<bool>,
    /// When true, parser may apply fuzzy AMB / corrections / incomplete.
    allow_fuzzy: RefCell<bool>,
    /// Optional double-pinyin scheme name forwarded to parsers that support it.
    double_pinyin_scheme: RefCell<Option<String>>,
    /// Additional addon dictionaries (topic-specific)
    addon_lexicons: RefCell<Vec<(String, Arc<crate::Lexicon>, bool)>>,
}

impl<P: SyllableParser> Engine<P> {
    /// Create a new engine with the given model and parser.
    pub fn new(model: Model, parser: P) -> Self {
        // Default cache capacity
        let cache_capacity = 1000;

        Self {
            model,
            parser,
            limit: 50,
            cache: RefCell::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(cache_capacity)
                    .unwrap_or(std::num::NonZeroUsize::new(1000).unwrap()),
            )),
            cache_hits: RefCell::new(0),
            cache_misses: RefCell::new(0),
            emoji_dict: None,
            emoji_enabled: RefCell::new(false),
            english_dict: None,
            english_enabled: RefCell::new(false),
            sort_by_pinyin_length: RefCell::new(false),
            allow_fuzzy: RefCell::new(true),
            double_pinyin_scheme: RefCell::new(None),
            addon_lexicons: RefCell::new(Vec::new()),
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
    /// 5. Append emoji candidates if enabled
    /// 6. Cache the result
    pub fn input(&self, input: &str) -> Vec<Candidate> {
        // Check cache first (LRU automatically updates access time)
        if let Some(cached) = self.cache.borrow_mut().get(input) {
            *self.cache_hits.borrow_mut() += 1;
            return cached.clone();
        }

        *self.cache_misses.borrow_mut() += 1;

        // C uses one parse. `segment_top_k(..., 1)` routes to `segment_best`.
        let allow_fuzzy = *self.allow_fuzzy.borrow();
        let segs = {
            let scheme = self.double_pinyin_scheme.borrow();
            self.parser.segment_top_k_with_scheme(
                input,
                1,
                allow_fuzzy,
                scheme.as_deref(),
            )
        };

        // Single primary segmentation (k=1): no HashMap merge needed.
        let mut vec: Vec<Candidate> = match segs.len() {
            0 => Vec::new(),
            1 => self.generate_candidates_from_segmentation(&segs[0], true),
            _ => {
                let mut best: HashMap<String, Candidate> = HashMap::new();
                for seg in segs {
                    for cand in self.generate_candidates_from_segmentation(&seg, true) {
                        match best.get(&cand.text) {
                            Some(existing) if existing.score >= cand.score => {}
                            _ => {
                                best.insert(cand.text.clone(), cand);
                            }
                        }
                    }
                }
                best.into_values().collect()
            }
        };

        // Filter out masked phrases (session config + persisted userdict masks).
        let config = self.model.config.borrow();
        let has_config_masks = !config.masked_phrases.is_empty();
        if has_config_masks {
            vec.retain(|c| !config.is_masked(&c.text));
        }
        drop(config);
        if self.model.userdict.has_masks() {
            vec.retain(|c| !self.model.userdict.is_masked(&c.text));
        }

        // Rank by configured mode, then score/frequency.
        let input_len = input.len();
        let rank_mode = self.model.config.borrow().candidate_rank_mode;
        let matched_span = |c: &Candidate| c.input_consumed.unwrap_or(input_len);
        let phrase_chars = |c: &Candidate| c.text.chars().count();
        let rank = |a: &Candidate, b: &Candidate| {
            let primary = match rank_mode {
                crate::CandidateRankMode::MatchedSpanThenScore => {
                    matched_span(b).cmp(&matched_span(a))
                }
                crate::CandidateRankMode::PhraseLengthThenScore => {
                    phrase_chars(b).cmp(&phrase_chars(a))
                }
            };
            if primary != std::cmp::Ordering::Equal {
                return primary;
            }
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        };
        vec.sort_by(rank);

        if vec.len() > self.limit {
            vec.truncate(self.limit);
        }

        // Query enabled addon lexicons and merge results
        {
            let addons = self.addon_lexicons.borrow();
            for (_name, lexicon, enabled) in addons.iter() {
                if !*enabled {
                    continue;
                }
                // Query addon with the full input as key
                let addon_entries = lexicon.lookup_with_freq(input);
                for (text, freq) in addon_entries {
                    if !vec.iter().any(|c| c.text == text) {
                        // Insert addon results with slightly lower priority
                        let score = freq as f32 * 0.8;
                        vec.push(Candidate::new(text, score));
                    }
                }
            }
        }

        vec.sort_by(rank);

        // Optional: when sort_by_pinyin_length is on, prefer fewer chars among near-ties.
        if *self.sort_by_pinyin_length.borrow() {
            vec.sort_by(|a, b| {
                let score_cmp = b
                    .score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal);
                if score_cmp == std::cmp::Ordering::Equal
                    || (a.score - b.score).abs() < 0.5
                {
                    a.text.chars().count().cmp(&b.text.chars().count()).then(score_cmp)
                } else {
                    score_cmp
                }
            });
        }

        // Mixed-input extras: keep a short phonetic head, then surface english /
        // emoji on the first page instead of appending after ~50 hanzi hits.
        let mut extras: Vec<Candidate> = Vec::new();
        if *self.english_enabled.borrow() {
            if let Some(ref english_dict) = self.english_dict {
                for (word, freq) in english_dict.candidates_for_input(input) {
                    if vec.iter().any(|c| c.text.eq_ignore_ascii_case(word))
                        || extras.iter().any(|c| c.text.eq_ignore_ascii_case(word))
                    {
                        continue;
                    }
                    extras.push(Candidate::new(word.to_string(), -50.0 + freq.ln_1p()));
                }
            }
        }
        if *self.emoji_enabled.borrow() {
            if let Some(ref emoji_dict) = self.emoji_dict {
                // Exact keyword on the raw buffer only.
                if let Some(entry) = emoji_dict.lookup_best(input) {
                    if !vec.iter().any(|c| c.text == entry.emoji)
                        && !extras.iter().any(|c| c.text == entry.emoji)
                    {
                        extras.push(Candidate::new(entry.emoji.clone(), -80.0));
                    }
                }
            }
        }

        if !extras.is_empty() {
            const PHONETIC_HEAD: usize = 3;
            let keep_phonetic = self.limit.saturating_sub(extras.len()).max(1);
            if vec.len() > keep_phonetic {
                vec.truncate(keep_phonetic);
            }
            let head = vec.len().min(PHONETIC_HEAD);
            let mut merged = Vec::with_capacity(vec.len() + extras.len());
            merged.extend(vec.drain(..head));
            merged.append(&mut extras);
            merged.append(&mut vec);
            vec = merged;
        }

        if vec.len() > self.limit {
            vec.truncate(self.limit);
        }

        // Cache the result (LRU automatically handles eviction)
        self.cache.borrow_mut().put(input.to_string(), vec.clone());

        vec
    }

    /// Commit a phrase to user learning.
    ///
    /// Records user selection to boost future rankings when learning is enabled.
    /// Clears cache to reflect updated frequencies immediately.
    pub fn commit(&self, phrase: &str) {
        if self.model.config.borrow().learning_enabled {
            self.model.userdict.learn(phrase);
        }
        self.clear_cache();
    }

    /// Forget a candidate phrase (fcitx Ctrl+7 / ibus-style remove from history).
    ///
    /// Deletes any learned frequency, masks the phrase so it stays hidden, and
    /// clears the candidate cache.
    pub fn forget_phrase(&self, phrase: &str) {
        if phrase.is_empty() {
            return;
        }
        let _ = self.model.userdict.forget_phrase(phrase);
        self.model.config.borrow_mut().mask_phrase(phrase);
        self.clear_cache();
    }

    /// Set the emoji dictionary for this engine.
    pub fn set_emoji_dict(&mut self, dict: EmojiDict) {
        self.emoji_dict = Some(dict);
    }

    /// Enable or disable emoji candidates in results.
    pub fn set_emoji_enabled(&self, enabled: bool) {
        *self.emoji_enabled.borrow_mut() = enabled;
        self.clear_cache();
    }

    /// Returns whether emoji candidates are enabled.
    pub fn emoji_enabled(&self) -> bool {
        *self.emoji_enabled.borrow()
    }

    /// Set the English word dictionary for mixed-input candidates.
    pub fn set_english_dict(&mut self, dict: EnglishDict) {
        self.english_dict = Some(dict);
    }

    /// Enable or disable English word candidates.
    pub fn set_english_enabled(&self, enabled: bool) {
        *self.english_enabled.borrow_mut() = enabled;
        self.clear_cache();
    }

    /// Returns whether English candidates are enabled.
    pub fn english_enabled(&self) -> bool {
        *self.english_enabled.borrow()
    }

    /// Enable or disable parser fuzzy AMB / corrections / incomplete matching.
    pub fn set_allow_fuzzy(&self, enabled: bool) {
        *self.allow_fuzzy.borrow_mut() = enabled;
        self.clear_cache();
    }

    /// Returns whether fuzzy matching is enabled for segmentation.
    pub fn allow_fuzzy(&self) -> bool {
        *self.allow_fuzzy.borrow()
    }

    /// Prefer shorter candidates when scores are close.
    pub fn set_sort_by_pinyin_length(&self, enabled: bool) {
        *self.sort_by_pinyin_length.borrow_mut() = enabled;
        self.clear_cache();
    }

    pub fn sort_by_pinyin_length(&self) -> bool {
        *self.sort_by_pinyin_length.borrow()
    }

    /// Emit best multi-token DP sentence into candidates (default: true).
    pub fn set_emit_composed_sentences(&self, enabled: bool) {
        self.model.config.borrow_mut().emit_composed_sentences = enabled;
        self.clear_cache();
    }

    pub fn emit_composed_sentences(&self) -> bool {
        self.model.config.borrow().emit_composed_sentences
    }

    /// Candidate bar ordering (matched pinyin span vs Chinese phrase length).
    pub fn set_candidate_rank_mode(&self, mode: crate::CandidateRankMode) {
        self.model.config.borrow_mut().candidate_rank_mode = mode;
        self.clear_cache();
    }

    pub fn candidate_rank_mode(&self) -> crate::CandidateRankMode {
        self.model.config.borrow().candidate_rank_mode
    }

    /// Apply a high-level [`crate::ImeProfile`] (BestUx vs LibpinyinCompat).
    pub fn set_profile(&self, profile: crate::ImeProfile) {
        profile.apply_to_config(&mut self.model.config.borrow_mut());
        match profile {
            crate::ImeProfile::BestUx => {
                // Mixed-input on; fuzzy AMB stays user-controlled (Mac/Windows/ibus
                // default fuzzy off — corrections/incomplete are separate knobs).
                *self.english_enabled.borrow_mut() = true;
                *self.emoji_enabled.borrow_mut() = true;
            }
            crate::ImeProfile::LibpinyinCompat => {
                *self.english_enabled.borrow_mut() = false;
                *self.emoji_enabled.borrow_mut() = false;
            }
        }
        self.clear_cache();
    }

    pub fn profile(&self) -> crate::ImeProfile {
        self.model.config.borrow().ime_profile
    }

    /// Best full-coverage sentence for `input` (C `pinyin_get_sentence` role).
    ///
    /// Runs segment + sentence DP only — does not build the candidate bar
    /// (prefixes / full-key heads / rank / cache). Available even when
    /// [`Config::emit_composed_sentences`] is false.
    pub fn best_sentence(&self, input: &str) -> Option<String> {
        let allow_fuzzy = *self.allow_fuzzy.borrow();
        let segs = {
            let scheme = self.double_pinyin_scheme.borrow();
            self.parser.segment_top_k_with_scheme(
                input,
                1,
                allow_fuzzy,
                scheme.as_deref(),
            )
        };
        let seg = segs.first()?;
        self.compose_best_sentence(seg)
    }

    /// Optional double-pinyin scheme (`Microsoft`, `ZiRanMa`, …). `None` = full pinyin.
    pub fn set_double_pinyin_scheme(&self, scheme: Option<String>) {
        *self.double_pinyin_scheme.borrow_mut() = scheme;
        self.clear_cache();
    }

    pub fn double_pinyin_scheme(&self) -> Option<String> {
        self.double_pinyin_scheme.borrow().clone()
    }

    /// Swap the lexicon (e.g., for simplified/traditional toggle). Clears cache.
    pub fn swap_lexicon(&self, lexicon: crate::Lexicon) {
        self.model.swap_lexicon(lexicon);
        self.clear_cache();
    }

    /// Swap the word bigram model together with lexicon on charset change.
    pub fn swap_word_bigram(&self, word_bigram: crate::WordBigram) {
        self.model.swap_word_bigram(word_bigram);
        self.clear_cache();
    }

    /// Add an addon lexicon. Returns its index.
    pub fn add_addon(&self, name: String, lexicon: crate::Lexicon, enabled: bool) -> usize {
        let mut addons = self.addon_lexicons.borrow_mut();
        let idx = addons.len();
        addons.push((name, Arc::new(lexicon), enabled));
        if enabled {
            self.clear_cache();
        }
        idx
    }

    /// Enable or disable an addon lexicon by name.
    pub fn set_addon_enabled(&self, name: &str, enabled: bool) {
        let mut addons = self.addon_lexicons.borrow_mut();
        for item in addons.iter_mut() {
            if item.0 == name {
                item.2 = enabled;
            }
        }
        drop(addons);
        self.clear_cache();
    }

    /// Get list of addon names and their enabled state.
    pub fn addon_list(&self) -> Vec<(String, bool)> {
        self.addon_lexicons
            .borrow()
            .iter()
            .map(|(name, _, enabled)| (name.clone(), *enabled))
            .collect()
    }

    /// Generate candidates from a segmentation by trying all possible word combinations.
    ///
    /// Uses dynamic programming to find valid word sequences that cover the entire segmentation.
    /// Scoring follows upstream libpinyin `PhoneticLookup`:
    /// - unigram_poss = phrase_freq / total_freq (phrase index)
    /// - no context: log(unigram_poss * pinyin_poss * unigram_lambda)
    /// - with context: log((λ·bigram + (1-λ)·unigram) * pinyin_poss)
    /// - user learning folds into phrase_freq as count * unigram_factor
    /// - LONG_SENTENCE_PENALTY used only when comparing paths of char-length ±1
    fn generate_candidates_from_segmentation(
        &self,
        seg: &[P::Syllable],
        allow_composed_sentence: bool,
    ) -> Vec<Candidate> {
        let n = seg.len();
        if n == 0 {
            return Vec::new();
        }

        let mut results: Vec<Candidate> = Vec::with_capacity(48);
        let mut seen: HashSet<String> = HashSet::with_capacity(64);
        let seg_pinyin_poss = Self::pinyin_poss_for_span(seg);
        let allow_fuzzy = *self.allow_fuzzy.borrow();
        let (lambda, unigram_factor, sentence_length_penalty, emit_sentences, total_freq) = {
            let cfg = self.model.config.borrow();
            let lex = self.model.lexicon.borrow();
            (
                cfg.lambda,
                cfg.unigram_factor,
                cfg.sentence_length_penalty,
                allow_composed_sentence && cfg.emit_composed_sentences,
                lex.total_freq(),
            )
        };

        let push_unique = |results: &mut Vec<Candidate>,
                           seen: &mut HashSet<String>,
                           cand: Candidate| {
            if seen.insert(cand.text.clone()) {
                results.push(cand);
            }
        };

        // Unigram-only score; userdict freqs are hot-cached (no redb I/O).
        let word_bigram = self.model.word_bigram.borrow();
        let score_uni = |phrase: &str, freq: u32, poss: f32| -> f32 {
            let user_freq = self.model.userdict.frequency(phrase);
            let effective = freq as f64 + unigram_factor as f64 * user_freq as f64;
            let unigram = if total_freq > 0 && effective > 0.0 {
                (effective / total_freq as f64) as f32
            } else {
                word_bigram.get_unigram_probability(phrase)
            };
            (unigram * poss * (1.0 - lambda)).max(1e-10).ln()
        };

        const FULL_KEY_TOP: usize = 16;
        const DP_LOOKUP_TOP: usize = 8;
        const PREFIX_LOOKUP_TOP: usize = 6;
        const AMB_SPAN_SYLLABLES: usize = 2;
        const MAX_PREFIX_SYLLABLES: usize = 2;

        let full_syllables: Vec<&str> = seg.iter().map(|s| s.text()).collect();
        // AMB fanout only on short spans; long keys already come from fuzzy segment.
        let full_amb = allow_fuzzy && n <= AMB_SPAN_SYLLABLES;
        let lexicon = self.model.lexicon.borrow();
        for (full_key, key_poss) in Self::lookup_key_variants(&full_syllables, full_amb) {
            let poss = (seg_pinyin_poss * key_poss).max(0.01);
            for (phrase, freq) in lexicon.lookup_with_freq_top(&full_key, FULL_KEY_TOP) {
                let score = score_uni(&phrase, freq, poss);
                push_unique(&mut results, &mut seen, Candidate::new(phrase, score));
            }
        }

        // Prefix partials (no DP required).
        if n > 1 {
            let partial_penalty = 100.0;
            let max_k = (n - 1).min(MAX_PREFIX_SYLLABLES);
            for k in 1..=max_k {
                let bytes_consumed: usize = seg[..k].iter().map(|s| s.text().len()).sum();
                let span_poss = Self::pinyin_poss_for_span(&seg[..k]);
                let span_syllables: Vec<&str> = seg[..k].iter().map(|s| s.text()).collect();
                let amb = allow_fuzzy && k <= AMB_SPAN_SYLLABLES;
                for (lookup_key, key_poss) in Self::lookup_key_variants(&span_syllables, amb) {
                    let poss = (span_poss * key_poss).max(0.01);
                    for (phrase, freq) in lexicon.lookup_with_freq_top(&lookup_key, PREFIX_LOOKUP_TOP)
                    {
                        let score = score_uni(&phrase, freq, poss) - partial_penalty;
                        push_unique(
                            &mut results,
                            &mut seen,
                            Candidate::with_input_consumed(phrase, score, bytes_consumed),
                        );
                    }
                }
            }
        }

        // Composed sentence needs DP (n≥2). Phrase-only mode (LibpinyinCompat) skips it.
        if emit_sentences && n >= 2 {
            if let Some((text, score)) = self.compose_sentence_dp(
                seg,
                &lexicon,
                &word_bigram,
                lambda,
                unigram_factor,
                total_freq,
                sentence_length_penalty,
                allow_fuzzy,
                DP_LOOKUP_TOP,
                AMB_SPAN_SYLLABLES,
            ) {
                push_unique(&mut results, &mut seen, Candidate::new(text, score));
            }
        }

        results
    }

    /// Sentence DP only (C `guess_sentence` role). Returns `(text, score)`.
    fn compose_best_sentence(&self, seg: &[P::Syllable]) -> Option<String> {
        let n = seg.len();
        if n == 0 {
            return None;
        }
        let allow_fuzzy = *self.allow_fuzzy.borrow();
        let (lambda, unigram_factor, sentence_length_penalty, total_freq) = {
            let cfg = self.model.config.borrow();
            let lex = self.model.lexicon.borrow();
            (
                cfg.lambda,
                cfg.unigram_factor,
                cfg.sentence_length_penalty,
                lex.total_freq(),
            )
        };
        const DP_LOOKUP_TOP: usize = 8;
        const AMB_SPAN_SYLLABLES: usize = 2;
        let lexicon = self.model.lexicon.borrow();
        let word_bigram = self.model.word_bigram.borrow();
        self.compose_sentence_dp(
            seg,
            &lexicon,
            &word_bigram,
            lambda,
            unigram_factor,
            total_freq,
            sentence_length_penalty,
            allow_fuzzy,
            DP_LOOKUP_TOP,
            AMB_SPAN_SYLLABLES,
        )
        .map(|(text, _)| text)
    }

    /// Core phrase DP over a segmentation. Shared by candidate emit and `best_sentence`.
    fn compose_sentence_dp(
        &self,
        seg: &[P::Syllable],
        lexicon: &crate::Lexicon,
        word_bigram: &crate::WordBigram,
        lambda: f32,
        unigram_factor: f32,
        total_freq: u64,
        sentence_length_penalty: f32,
        allow_fuzzy: bool,
        dp_lookup_top: usize,
        amb_span_syllables: usize,
    ) -> Option<(String, f32)> {
        let n = seg.len();
        if n < 2 {
            // Single syllable: best lexicon head is the "sentence".
            if n == 1 {
                let key = seg[0].text();
                let poss = Self::pinyin_poss_for_span(seg);
                let amb = allow_fuzzy;
                let mut best: Option<(String, f32)> = None;
                for (lookup_key, key_poss) in Self::lookup_key_variants(&[key], amb) {
                    for (word_text, freq) in lexicon.lookup_with_freq_top(&lookup_key, dp_lookup_top)
                    {
                        let score = Self::score_step_fast(
                            &self.model.userdict,
                            word_bigram,
                            &word_text,
                            freq,
                            None,
                            (poss * key_poss).max(0.01),
                            lambda,
                            unigram_factor,
                            total_freq,
                        );
                        let better = match &best {
                            None => true,
                            Some((_, s)) => score > *s,
                        };
                        if better {
                            best = Some((word_text, score));
                        }
                    }
                }
                return best;
            }
            return None;
        }

        struct Step {
            word: String,
            step_score: f32,
            prev: u16,
            total_score: f32,
            total_chars: u16,
        }
        let mut steps: Vec<Step> = Vec::with_capacity(n * 4);
        let mut best_head: Vec<Option<u16>> = vec![None; n + 1];

        const MAX_SHORT_SYLLABLES: usize = 4;
        const MAX_LONG_LOOKUP_SYLLABLES: usize = 6;

        for i in 0..n {
            if i > 0 && best_head[i].is_none() {
                continue;
            }
            let prev_idx = if i == 0 { None } else { best_head[i] };
            let (prev_total, prev_chars) = match prev_idx {
                None => (0.0f32, 0u16),
                Some(idx) => {
                    let s = &steps[idx as usize];
                    (s.total_score, s.total_chars)
                }
            };
            let prev_word_owned: Option<String> =
                prev_idx.map(|idx| steps[idx as usize].word.clone());

            for len in 1..=std::cmp::min(MAX_SHORT_SYLLABLES, n - i) {
                let span_poss = Self::pinyin_poss_for_span(&seg[i..i + len]);
                let span_syllables: Vec<&str> = seg[i..i + len].iter().map(|s| s.text()).collect();
                let amb = allow_fuzzy && len <= amb_span_syllables;
                for (lookup_key, key_poss) in Self::lookup_key_variants(&span_syllables, amb) {
                    let poss = (span_poss * key_poss).max(0.01);
                    let hits = lexicon.lookup_with_freq_top(&lookup_key, dp_lookup_top);
                    for (word_text, freq) in hits {
                        let step_score = Self::score_step_fast(
                            &self.model.userdict,
                            word_bigram,
                            &word_text,
                            freq,
                            prev_word_owned.as_deref(),
                            poss,
                            lambda,
                            unigram_factor,
                            total_freq,
                        );
                        let total_score = prev_total + step_score;
                        let total_chars = prev_chars + word_text.chars().count() as u16;
                        let end = i + len;
                        let better = match best_head[end] {
                            None => true,
                            Some(idx) => {
                                let old = &steps[idx as usize];
                                Self::totals_better_than(
                                    total_score,
                                    total_chars as i32,
                                    old.total_score,
                                    old.total_chars as i32,
                                    sentence_length_penalty,
                                )
                            }
                        };
                        if better {
                            let idx = steps.len() as u16;
                            steps.push(Step {
                                word: word_text,
                                step_score,
                                prev: prev_idx.unwrap_or(u16::MAX),
                                total_score,
                                total_chars,
                            });
                            best_head[end] = Some(idx);
                        }
                    }
                }
            }

            for len in (MAX_SHORT_SYLLABLES + 1)..=std::cmp::min(MAX_LONG_LOOKUP_SYLLABLES, n - i)
            {
                let span_syllables: Vec<&str> = seg[i..i + len].iter().map(|s| s.text()).collect();
                let exact = span_syllables.join("'");
                if !lexicon.has_key(&exact) {
                    continue;
                }
                let span_poss = Self::pinyin_poss_for_span(&seg[i..i + len]);
                let hits = lexicon.lookup_with_freq_top(&exact, dp_lookup_top);
                for (word_text, freq) in hits {
                    let step_score = Self::score_step_fast(
                        &self.model.userdict,
                        word_bigram,
                        &word_text,
                        freq,
                        prev_word_owned.as_deref(),
                        span_poss,
                        lambda,
                        unigram_factor,
                        total_freq,
                    );
                    let total_score = prev_total + step_score;
                    let total_chars = prev_chars + word_text.chars().count() as u16;
                    let end = i + len;
                    let better = match best_head[end] {
                        None => true,
                        Some(idx) => {
                            let old = &steps[idx as usize];
                            Self::totals_better_than(
                                total_score,
                                total_chars as i32,
                                old.total_score,
                                old.total_chars as i32,
                                sentence_length_penalty,
                            )
                        }
                    };
                    if better {
                        let idx = steps.len() as u16;
                        steps.push(Step {
                            word: word_text,
                            step_score,
                            prev: prev_idx.unwrap_or(u16::MAX),
                            total_score,
                            total_chars,
                        });
                        best_head[end] = Some(idx);
                    }
                }
            }
        }

        let head = best_head[n]?;
        let mut words = Vec::new();
        let mut idx = head;
        loop {
            let s = &steps[idx as usize];
            words.push((s.word.as_str(), s.step_score));
            if s.prev == u16::MAX {
                break;
            }
            idx = s.prev;
        }
        words.reverse();
        let text: String = words.iter().map(|(w, _)| *w).collect();
        let score: f32 = words.iter().map(|(_, s)| s).sum();
        Some((text, score))
    }

    /// Compare two trellis totals (score + char length) like `path_better_than`.
    fn totals_better_than(
        lhs_score: f32,
        lhs_chars: i32,
        rhs_score: f32,
        rhs_chars: i32,
        penalty: f32,
    ) -> bool {
        if lhs_chars + 1 == rhs_chars && lhs_score + penalty < rhs_score {
            return false;
        }
        if lhs_chars == rhs_chars + 1 && lhs_score < rhs_score + penalty {
            return false;
        }
        if lhs_chars == rhs_chars {
            return lhs_score > rhs_score;
        }
        lhs_chars < rhs_chars
    }

    /// Pronunciation match probability for a syllable span.
    /// Exact syllables → 1.0; each fuzzy/correction syllable multiplies by 0.5.
    fn pinyin_poss_for_span(span: &[P::Syllable]) -> f32 {
        let fuzzy = span.iter().filter(|s| s.is_fuzzy()).count();
        if fuzzy == 0 {
            1.0
        } else {
            0.5_f32.powi(fuzzy as i32).max(0.01)
        }
    }

    /// Lexicon keys to probe for a syllable span, including AMB initial swaps
    /// when fuzzy is enabled (so `nve` also finds `lve` → 略).
    fn lookup_key_variants(syllables: &[&str], allow_fuzzy: bool) -> Vec<(String, f32)> {
        if syllables.is_empty() {
            return Vec::new();
        }
        let exact = syllables.join("'");
        let mut out = vec![(exact, 1.0)];
        if !allow_fuzzy {
            return out;
        }
        // Product of per-syllable shengmu variants (capped).
        let mut variants: Vec<(Vec<String>, f32)> = vec![(Vec::new(), 1.0)];
        for syl in syllables {
            let alts = Self::shengmu_amb_variants(syl);
            let mut next = Vec::new();
            for (prefix, poss) in variants {
                for (alt, ap) in &alts {
                    if next.len() >= 16 {
                        break;
                    }
                    let mut p = prefix.clone();
                    p.push(alt.clone());
                    next.push((p, poss * ap));
                }
            }
            variants = next;
        }
        for (parts, poss) in variants {
            let key = parts.join("'");
            if !out.iter().any(|(k, _)| k == &key) {
                out.push((key, poss));
            }
        }
        out
    }

    /// AMB shengmu variants for one syllable (`nve`→`lve`, `zong`→`zhong`, …).
    fn shengmu_amb_variants(syl: &str) -> Vec<(String, f32)> {
        let mut out = vec![(syl.to_string(), 1.0)];
        let push = |out: &mut Vec<(String, f32)>, s: String| {
            if !out.iter().any(|(k, _)| k == &s) {
                out.push((s, 0.5));
            }
        };
        // Digraphs first so `zh` is not treated as `z`+`h`.
        if let Some(rest) = syl.strip_prefix("zh") {
            push(&mut out, format!("z{rest}"));
        } else if let Some(rest) = syl.strip_prefix('z') {
            push(&mut out, format!("zh{rest}"));
        }
        if let Some(rest) = syl.strip_prefix("ch") {
            push(&mut out, format!("c{rest}"));
        } else if let Some(rest) = syl.strip_prefix('c') {
            push(&mut out, format!("ch{rest}"));
        }
        if let Some(rest) = syl.strip_prefix("sh") {
            push(&mut out, format!("s{rest}"));
        } else if let Some(rest) = syl.strip_prefix('s') {
            push(&mut out, format!("sh{rest}"));
        }
        if let Some(rest) = syl.strip_prefix('n') {
            // Keep `ng…` / `n` incomplete as-is; still allow n↔l for nüe/lüe etc.
            if !rest.is_empty() || syl == "n" {
                push(&mut out, format!("l{rest}"));
            }
        }
        if let Some(rest) = syl.strip_prefix('l') {
            push(&mut out, format!("n{rest}"));
            push(&mut out, format!("r{rest}"));
        }
        if let Some(rest) = syl.strip_prefix('r') {
            push(&mut out, format!("l{rest}"));
        }
        if let Some(rest) = syl.strip_prefix('f') {
            push(&mut out, format!("h{rest}"));
        }
        if let Some(rest) = syl.strip_prefix('h') {
            push(&mut out, format!("f{rest}"));
        }
        if let Some(rest) = syl.strip_prefix('g') {
            push(&mut out, format!("k{rest}"));
        }
        if let Some(rest) = syl.strip_prefix('k') {
            push(&mut out, format!("g{rest}"));
        }
        // Final AMB (an↔ang, …) — needed for fan→方 when typed without 'g'.
        let yunmu_pairs = [
            ("ang", "an"),
            ("an", "ang"),
            ("eng", "en"),
            ("en", "eng"),
            ("ing", "in"),
            ("in", "ing"),
        ];
        for (from, to) in yunmu_pairs {
            if let Some(head) = syl.strip_suffix(from) {
                // Avoid turning "wang" into "wan"+"g" mishaps: strip_suffix is exact.
                push(&mut out, format!("{head}{to}"));
            }
        }
        out
    }

    /// Upstream phrase-index unigram with pre-hoisted totals (hot path).
    #[inline]
    fn unigram_poss_fast(
        userdict: &crate::UserDict,
        word_bigram: &crate::WordBigram,
        phrase: &str,
        lexicon_freq: u32,
        unigram_factor: f32,
        total_freq: u64,
    ) -> f32 {
        let user_freq = userdict.frequency(phrase);
        let effective = lexicon_freq as f64 + unigram_factor as f64 * user_freq as f64;
        if total_freq > 0 && effective > 0.0 {
            return (effective / total_freq as f64) as f32;
        }
        word_bigram.get_unigram_probability(phrase)
    }

    /// One trellis step score with pre-hoisted config / model refs (hot path).
    #[inline]
    fn score_step_fast(
        userdict: &crate::UserDict,
        word_bigram: &crate::WordBigram,
        phrase: &str,
        lexicon_freq: u32,
        prev: Option<&str>,
        pinyin_poss: f32,
        lambda: f32,
        unigram_factor: f32,
        total_freq: u64,
    ) -> f32 {
        let unigram = Self::unigram_poss_fast(
            userdict,
            word_bigram,
            phrase,
            lexicon_freq,
            unigram_factor,
            total_freq,
        );
        let raw = if let Some(prev_word) = prev {
            let bigram = word_bigram.get_probability(prev_word, phrase);
            (lambda * bigram + (1.0 - lambda) * unigram) * pinyin_poss
        } else {
            unigram * pinyin_poss * (1.0 - lambda)
        };
        raw.max(1e-10).ln()
    }

    /// Get cache statistics for monitoring.
    ///
    /// Returns (hits, misses) tuple.
    pub fn cache_stats(&self) -> (usize, usize) {
        (*self.cache_hits.borrow(), *self.cache_misses.borrow())
    }

    /// Get current cache size (number of entries).
    pub fn cache_size(&self) -> usize {
        self.cache.borrow().len()
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

    /// Segment input and return preedit text with apostrophe separators between syllables,
    /// along with the mapped cursor position.
    ///
    /// For example, "wode" with cursor at 2 → ("wo'de", 2), cursor at 4 → ("wo'de", 5).
    /// The cursor is adjusted to account for inserted apostrophes.
    pub fn preedit_with_separators(&self, input: &str, input_cursor: usize) -> (String, usize) {
        if input.is_empty() {
            return (String::new(), 0);
        }
        let segs = {
            let scheme = self.double_pinyin_scheme.borrow();
            let allow_fuzzy = *self.allow_fuzzy.borrow();
            self.parser
                .segment_top_k_with_scheme(input, 1, allow_fuzzy, scheme.as_deref())
        };
        if let Some(seg) = segs.into_iter().next() {
            if seg.len() > 1 {
                let mut preedit = String::new();
                let mut mapped_cursor = input_cursor;
                let mut input_pos = 0;
                for (i, s) in seg.iter().enumerate() {
                    if i > 0 {
                        preedit.push('\'');
                        // If cursor is past this syllable boundary, add 1 for the apostrophe
                        if input_cursor > input_pos {
                            mapped_cursor += 1;
                        }
                    }
                    preedit.push_str(s.text());
                    input_pos += s.text().len();
                }
                return (preedit, mapped_cursor);
            }
        }
        (input.to_string(), input_cursor)
    }
}

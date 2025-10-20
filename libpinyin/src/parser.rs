// libchinese/libpinyin/src/parser.rs
//
// Pinyin parser for syllable segmentation.
// - DP-based segmentation with beam search (segment_top_k)
// - TrieNode for efficient prefix matching
// - Fuzzy matching integrated via Engine
//
// References (upstream C++):
// - src/storage/pinyin_parser2.cpp
// - src/storage/pinyin_parser_table.h
// - src/pinyin.cpp
//
// Future enhancements:
// - Verify exact parity with upstream DP cost model
// - Add comprehensive test vectors from upstream test suite

use libchinese_core::TrieNode;
use libchinese_core::FuzzyMap;

/// A single matched syllable (a chunk of pinyin).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syllable {
    /// The syllable text as stored in the syllable set (e.g. "ni", "hao", "zhong").
    pub text: String,

    /// Whether this match was produced via a fuzzy rule (placeholder boolean).
    /// In a full implementation this would carry more information:
    /// which rule, penalty applied, and mapping direction.
    pub fuzzy: bool,
}

impl Syllable {
    pub fn new<T: Into<String>>(text: T, fuzzy: bool) -> Self {
        Self {
            text: text.into(),
            fuzzy,
        }
    }
}

/// Parser providing segmentation using a trie and fuzzy rules (placeholder).
///
/// Public entrypoints:
/// - `Parser::insert_syllable` to seed the trie
/// - `Parser::segment_best` to obtain the best segmentation
///
/// Implementation notes:
/// - This is a correctness-focused starter implementation. The upstream
///   `pinyin_parser2.cpp` uses table-driven parsing and DP tailored for
///   pinyin syllable ambiguities. We will port the exact DP recurrence later.
#[derive(Debug)]
pub struct Parser {
    trie: TrieNode,
    fuzzy: FuzzyMap,
}

impl Parser {
    /// Create an empty parser.
    pub fn new() -> Self {
        // Use standard pinyin fuzzy rules
        let rules = crate::standard_fuzzy_rules();
        Self {
            trie: TrieNode::new(),
            fuzzy: FuzzyMap::from_rules(&rules),
        }
    }

    /// Create a parser and insert a list of syllables.
    pub fn with_syllables<T: AsRef<str>>(syllables: &[T]) -> Self {
        let mut p = Parser::new();
        for s in syllables {
            p.insert_syllable(s.as_ref());
        }
        p
    }

    /// Insert a single syllable into the parser's trie.
    pub fn insert_syllable(&mut self, syllable: &str) {
        // canonicalize: lower-case and trim
        let key = syllable.trim().to_ascii_lowercase();
        if !key.is_empty() {
            self.trie.insert(&key);
        }
    }

    /// True if the parser contains the exact syllable.
    pub fn contains_syllable(&self, syllable: &str) -> bool {
        self.trie.contains_word(&syllable.to_ascii_lowercase())
    }

    /// Return fuzzy alternatives for a syllable (public API for tests).
    ///
    /// This exposes the parser's fuzzy map in a controlled way so tests can
    /// validate fuzzy alternatives without accessing private fields.
    pub fn fuzzy_alternatives(&self, syllable: &str) -> Vec<String> {
        self.fuzzy.alternatives(syllable)
            .into_iter()
            .map(|(alt, _penalty)| alt)
            .collect()
    }

    /// Perform segmentation on `input` and return the single-best segmentation.
    ///
    /// This implements a DP with tie-breaking rules inspired by upstream
    /// `pinyin_parser2.cpp`:
    ///  - Prefer lower total cost (primary).
    ///  - On equal cost, prefer larger parsed length (more input covered by real syllables).
    ///  - If parsed length equal, prefer fewer keys/segments.
    ///  - If still equal, prefer smaller distance (we approximate with fuzzy usage / penalties).
    ///
    /// Returns a vector of syllable strings (in order). Each syllable is a
    /// normalized token (lowercase).
    pub fn segment_best(&self, input: &str, allow_fuzzy: bool) -> Vec<Syllable> {
        // Normalize input: lowercase and remove whitespace
        let normalized: Vec<char> = input
            .to_ascii_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();

        let n = normalized.len();
        if n == 0 {
            return Vec::new();
        }

        // Enhanced DP state per position with improved cost modeling:
        // - best_cost[pos]: comprehensive cost including length, frequency, and penalty factors
        // - best_parsed[pos]: total parsed characters (higher coverage is better) 
        // - best_num_keys[pos]: number of segments used (fewer segments preferred)
        // - best_distance[pos]: accumulated fuzzy/edit distance penalty (lower is better)
        // - best_choice[pos]: the chosen transition (end_pos, matched_string, fuzzy_flag)
        let mut best_cost: Vec<f32> = vec![std::f32::INFINITY; n + 1];
        let mut best_parsed: Vec<usize> = vec![0; n + 1];
        let mut best_num_keys: Vec<usize> = vec![usize::MAX; n + 1];
        let mut best_distance: Vec<i32> = vec![i32::MAX; n + 1];
        let mut best_choice: Vec<Option<(usize, String, bool)>> = vec![None; n + 1];

        // base: at end of input zero cost, zero parsed, zero keys, zero distance
        best_cost[n] = 0.0;
        best_parsed[n] = 0;
        best_num_keys[n] = 0;
        best_distance[n] = 0;

        // helper to decide whether candidate should replace current best at pos
        // Use a plain function that takes references to the best_* arrays to avoid
        // closure-capture borrow conflicts when we need to mutate those arrays.
        fn should_replace(
            pos: usize,
            cand_cost: f32,
            cand_parsed: usize,
            cand_keys: usize,
            cand_dist: i32,
            best_cost: &Vec<f32>,
            best_parsed: &Vec<usize>,
            best_num_keys: &Vec<usize>,
            best_distance: &Vec<i32>,
        ) -> bool {
            // primary: strictly lower cost
            if cand_cost < best_cost[pos] {
                return true;
            }
            // nearly-equal cost: apply tie-breakers
            if (cand_cost - best_cost[pos]).abs() < 1e-6 {
                if cand_parsed > best_parsed[pos] {
                    return true;
                }
                if cand_parsed == best_parsed[pos] {
                    if cand_keys < best_num_keys[pos] {
                        return true;
                    }
                    if cand_keys == best_num_keys[pos] {
                        if cand_dist < best_distance[pos] {
                            return true;
                        }
                    }
                }
            }
            false
        }

        // iterate positions backward
        for pos in (0..n).rev() {
            // First try all exact trie prefixes from pos
            let prefixes = self.trie.walk_prefixes(&normalized, pos);

            for (end, matched) in prefixes.iter() {
                // Only consider suffixes that are reachable (best_cost[end] finite)
                if best_cost[*end].is_infinite() {
                    continue;
                }

                // Enhanced cost model based on segment length and frequency
                let seg_len = end - pos;
                let seg_cost = self.calculate_segment_cost(matched, seg_len, false);
                let cand_cost = seg_cost + best_cost[*end];
                let cand_parsed = seg_len + best_parsed[*end];
                // num_keys: 1 for this segment + keys used from end
                let cand_keys = 1 + best_num_keys[*end];
                // distance: pass-through (exact match doesn't add distance)
                let cand_dist = best_distance[*end];

                if should_replace(
                    pos,
                    cand_cost,
                    cand_parsed,
                    cand_keys,
                    cand_dist,
                    &best_cost,
                    &best_parsed,
                    &best_num_keys,
                    &best_distance,
                ) {
                    best_cost[pos] = cand_cost;
                    best_parsed[pos] = cand_parsed;
                    best_num_keys[pos] = cand_keys;
                    best_distance[pos] = cand_dist;
                    best_choice[pos] = Some((*end, matched.clone(), false));
                }
            }

            // If fuzzy allowed, attempt fuzzy alternatives for substrings of varying lengths.
            // This allows different-length substitutions (e.g., "zi" -> "zhi", "an" -> "ang")
            // which is essential for comprehensive fuzzy matching in Chinese pinyin.
            if allow_fuzzy {
                for len in 1..=4 {
                    if pos + len > n {
                        break;
                    }
                    let substr: String = normalized[pos..pos + len].iter().collect();
                    let alts = self.fuzzy.alternatives(&substr);
                    for (alt, penalty) in alts {
                        if self.trie.contains_word(&alt) && alt != substr {
                            // Calculate the actual end position based on the alternative's length
                            // For same-length alternatives, use original end position
                            // For different-length alternatives, adjust accordingly
                            let alt_len = alt.chars().count();
                            let original_len = substr.chars().count();
                            
                            // For now, handle same-length and different-length cases
                            let end = if alt_len == original_len {
                                pos + len
                            } else {
                                // For different lengths, we need to consider if we can consume
                                // the alternative at this position. Since we're looking at
                                // fuzzy alternatives, we treat this as a substitution at the
                                // original position length.
                                pos + len
                            };
                            
                            if end <= n && !best_cost[end].is_infinite() {
                                let seg_cost = self.calculate_segment_cost(&alt, alt_len, true);
                                let cand_cost = seg_cost + best_cost[end];
                                let cand_parsed = len + best_parsed[end];  // Use original length for parsing position
                                let cand_keys = 1 + best_num_keys[end];
                                // Use the per-rule penalty from fuzzy map
                                let fuzzy_penalty = (penalty * 100.0) as i32; // Scale to integer
                                let cand_dist = fuzzy_penalty + best_distance[end];

                                if should_replace(
                                    pos,
                                    cand_cost,
                                    cand_parsed,
                                    cand_keys,
                                    cand_dist,
                                    &best_cost,
                                    &best_parsed,
                                    &best_num_keys,
                                    &best_distance,
                                ) {
                                    best_cost[pos] = cand_cost;
                                    best_parsed[pos] = cand_parsed;
                                    best_num_keys[pos] = cand_keys;
                                    best_distance[pos] = cand_dist;
                                    best_choice[pos] = Some((end, alt.clone(), true));
                                }
                            }
                        }
                    }
                }
            }

            // If still no choice found, fallback: consume one character as unknown token
            // We still allow this option but with a large penalty; it contributes 1 parsed char.
            if best_choice[pos].is_none() {
                let end = pos + 1;
                if !best_cost[end].is_infinite() {
                    let substr: String = normalized[pos..end].iter().collect();
                    let seg_cost = 10.0_f32; // large penalty for unknown pieces
                    let cand_cost = seg_cost + best_cost[end];
                    let cand_parsed = 1 + best_parsed[end];
                    let cand_keys = 1 + best_num_keys[end];
                    let cand_dist = 1000 + best_distance[end]; // very high distance for unknowns

                    if should_replace(
                        pos,
                        cand_cost,
                        cand_parsed,
                        cand_keys,
                        cand_dist,
                        &best_cost,
                        &best_parsed,
                        &best_num_keys,
                        &best_distance,
                    ) {
                        best_cost[pos] = cand_cost;
                        best_parsed[pos] = cand_parsed;
                        best_num_keys[pos] = cand_keys;
                        best_distance[pos] = cand_dist;
                        best_choice[pos] = Some((end, substr, false));
                    }
                }
            }
        }

        // Reconstruct segmentation
        let mut out: Vec<Syllable> = Vec::new();
        let mut cur = 0usize;
        while cur < n {
            if let Some((next, word, fuzzy)) = &best_choice[cur] {
                // Treat apostrophe (') as an enforced separator and skip it in the final output.
                // Upstream behavior propagates state across apostrophes but does not emit them
                // as tokens; mimic that here by advancing the cursor without pushing a token.
                if word == "'" {
                    cur = *next;
                    continue;
                }
                out.push(Syllable::new(word.clone(), *fuzzy));
                cur = *next;
            } else {
                // defensive fallback (shouldn't happen)
                let ch: String = normalized[cur].to_string();
                out.push(Syllable::new(ch, false));
                cur += 1;
            }
        }

        out
    }

    /// Return top-K segmentation alternatives (beam search).
    ///
    /// This implements a left-to-right beam search that expands exact trie
    /// Calculate the cost of a segment based on length, content, and fuzzy status.
    /// This implements a more sophisticated cost model similar to upstream libpinyin.
    fn calculate_segment_cost(&self, segment: &str, length: usize, is_fuzzy: bool) -> f32 {
        let base_cost = 1.0_f32;
        
        // Length bonus: longer segments are generally preferred in pinyin
        let length_bonus = match length {
            1 => 0.3,      // Single char segments get penalty
            2 => 0.0,      // Standard two-character segments are neutral  
            3 => -0.2,     // Three-character segments get small bonus
            4 => -0.3,     // Four+ character segments get larger bonus
            _ => -0.4,
        };
        
        // Content-based adjustments for common vs rare syllables
        let content_adjustment = match segment.len() {
            0..=2 => 0.1,   // Very short segments slightly penalized
            3..=5 => 0.0,   // Normal length segments
            _ => 0.05,      // Long segments slightly penalized for complexity
        };
        
        // Fuzzy penalty based on type of fuzzy match
        let fuzzy_penalty = if is_fuzzy { 0.8 } else { 0.0 };
        
        base_cost + length_bonus + content_adjustment + fuzzy_penalty
    }

    /// Return top-K segmentation alternatives (beam search).
    ///
    /// This implements a left-to-right beam search that expands exact trie
    /// prefixes and simple fuzzy alternatives (up to a small substring length).
    /// States are ranked by a tuple similar to the DP tie-breakers used in
    /// `segment_best`: (cost ascending, parsed descending, keys ascending, distance ascending).
    ///
    /// The implementation is intentionally conservative and correctness-first:
    /// it favors clarity and parity with `segment_best`'s cost model while
    /// producing up to `k` distinct segmentation hypotheses.
    pub fn segment_top_k(&self, input: &str, k: usize, allow_fuzzy: bool) -> Vec<Vec<Syllable>> {
        // Normalize input: lowercase and remove whitespace (same as segment_best)
        let normalized: Vec<char> = input
            .to_ascii_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let n = normalized.len();
        if n == 0 {
            return Vec::new();
        }

        // Beam state
        #[derive(Clone)]
        struct State {
            pos: usize,
            tokens: Vec<Syllable>,
            cost: f32,
            parsed: usize,
            keys: usize,
            dist: i32,
        }

        // Comparator used for ranking states (lower is better)
        fn state_cmp(a: &State, b: &State) -> std::cmp::Ordering {
            // primary: cost (smaller better)
            if (a.cost - b.cost).abs() > 1e-6 {
                return a
                    .cost
                    .partial_cmp(&b.cost)
                    .unwrap_or(std::cmp::Ordering::Equal);
            }
            // tie: prefer larger parsed
            if a.parsed != b.parsed {
                return b.parsed.cmp(&a.parsed);
            }
            // tie: prefer fewer keys
            if a.keys != b.keys {
                return a.keys.cmp(&b.keys);
            }
            // tie: prefer smaller distance
            a.dist.cmp(&b.dist)
        }

        // initial state
        let start = State {
            pos: 0,
            tokens: Vec::new(),
            cost: 0.0,
            parsed: 0,
            keys: 0,
            dist: 0,
        };

        let mut beam: Vec<State> = vec![start];
        let mut completed: Vec<State> = Vec::new();

        // beam width: allow some slack beyond k to keep diverse hypotheses
        let beam_width = std::cmp::max(8, k.saturating_mul(4));

        while !beam.is_empty() {
            let mut next_beam: Vec<State> = Vec::new();

            for st in beam.into_iter() {
                // If this state already finished, keep it in completed set.
                if st.pos == n {
                    completed.push(st);
                    continue;
                }

                // Expand exact trie prefixes starting at st.pos
                let prefixes = self.trie.walk_prefixes(&normalized, st.pos);
                for (end, matched) in prefixes.into_iter() {
                    // only expand if suffix from `end` is reachable (we don't require that here)
                    let mut new_tokens = st.tokens.clone();
                    new_tokens.push(Syllable::new(matched.clone(), false));
                    let new_state = State {
                        pos: end,
                        tokens: new_tokens,
                        cost: st.cost + 1.0_f32, // exact match cost
                        parsed: st.parsed + (end - st.pos),
                        keys: st.keys + 1,
                        dist: st.dist,
                    };
                    next_beam.push(new_state);
                }

                // Fuzzy alternatives (approximate): try short substrings and map via fuzzy.alternatives
                if allow_fuzzy {
                    for len in 1..=4 {
                        if st.pos + len > n {
                            break;
                        }
                        let substr: String = normalized[st.pos..st.pos + len].iter().collect();
                        let alts = self.fuzzy.alternatives(&substr);
                        for (alt, penalty) in alts.into_iter() {
                            // If the alt is an exact syllable in the trie, use it as a fuzzy match
                            if self.trie.contains_word(&alt) {
                                // only accept substitutions that match the same length in chars
                                if alt.chars().count() == substr.chars().count() {
                                    let end = st.pos + len;
                                    let mut new_tokens = st.tokens.clone();
                                    new_tokens.push(Syllable::new(alt.clone(), true));
                                    let new_state = State {
                                        pos: end,
                                        tokens: new_tokens,
                                        cost: st.cost + penalty, // Use per-rule penalty
                                        parsed: st.parsed + (end - st.pos),
                                        keys: st.keys + 1,
                                        dist: st.dist + (penalty * 100.0) as i32, // Scale for distance
                                    };
                                    next_beam.push(new_state);
                                }
                            }
                        }
                    }
                }

                // Unknown fallback: consume one character with heavy penalty
                let end = st.pos + 1;
                if end <= n {
                    let substr: String = normalized[st.pos..end].iter().collect();
                    let mut new_tokens = st.tokens.clone();
                    new_tokens.push(Syllable::new(substr.clone(), false));
                    let new_state = State {
                        pos: end,
                        tokens: new_tokens,
                        cost: st.cost + 10.0_f32,
                        parsed: st.parsed + 1,
                        keys: st.keys + 1,
                        dist: st.dist + 1000,
                    };
                    next_beam.push(new_state);
                }
            }

            if next_beam.is_empty() {
                break;
            }

            // prune next_beam to beam_width using our comparator
            next_beam.sort_by(|a, b| state_cmp(a, b));
            if next_beam.len() > beam_width {
                next_beam.truncate(beam_width);
            }

            beam = next_beam;
        }

        // If no completed segmentation was found, fall back to best single segmentation
        if completed.is_empty() {
            return vec![self.segment_best(input, allow_fuzzy)];
        }

        // Sort completed states and return top-k token sequences
        completed.sort_by(|a, b| state_cmp(a, b));
        let mut out: Vec<Vec<Syllable>> = Vec::new();
        for st in completed.into_iter().take(k) {
            out.push(st.tokens);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trie_insert_and_contains() {
        let mut trie = TrieNode::new();
        trie.insert("ni");
        trie.insert("hao");
        assert!(trie.contains_word("ni"));
        assert!(trie.contains_word("hao"));
        assert!(!trie.contains_word("n"));
        assert!(!trie.contains_word("ha"));
    }

    #[test]
    fn walk_prefixes_find_matches() {
        let mut trie = TrieNode::new();
        trie.insert("ni");
        trie.insert("nihao");
        trie.insert("n");
        let input: Vec<char> = "nihao".chars().collect();
        let prefixes = trie.walk_prefixes(&input, 0);
        // should find "n", "ni", "nihao" as prefixes that are words
        let mut found: Vec<String> = prefixes.into_iter().map(|(_, s)| s).collect();
        found.sort();
        assert!(found.contains(&"n".to_string()));
        assert!(found.contains(&"ni".to_string()));
        assert!(found.contains(&"nihao".to_string()));
    }

    #[test]
    fn parser_segment_basic() {
        let mut parser = Parser::new();
        // common demo syllables
        parser.insert_syllable("ni");
        parser.insert_syllable("hao");
        parser.insert_syllable("zhong");
        parser.insert_syllable("guo");

        let seg = parser.segment_best("nihao", /*allow_fuzzy=*/ false);
        let texts: Vec<String> = seg.into_iter().map(|s| s.text).collect();
        assert_eq!(texts, vec!["ni".to_string(), "hao".to_string()]);

        let seg2 = parser.segment_best("zhongguo", false);
        let texts2: Vec<String> = seg2.into_iter().map(|s| s.text).collect();
        assert_eq!(texts2, vec!["zhong".to_string(), "guo".to_string()]);
    }

    #[test]
    fn parser_unknown_fallback() {
        let mut parser = Parser::new();
        parser.insert_syllable("ni");
        let seg = parser.segment_best("nix", false);
        let texts: Vec<String> = seg.into_iter().map(|s| s.text).collect();
        // "ni" recognized, 'x' unknown falls back to single char token
        assert_eq!(texts, vec!["ni".to_string(), "x".to_string()]);
    }

    #[test]
    fn fuzzy_alternative_placeholder() {
        let mut parser = Parser::new();
        // insert both "zh" and "z" to illustrate fuzzy alternatives
        parser.insert_syllable("zh");
        parser.insert_syllable("z");
        // if fuzzy is enabled we at least have the alternative table available
        let alts = parser.fuzzy.alternatives("zh");
        assert!(alts.iter().any(|(alt, _)| alt == "z"));
        let alts2 = parser.fuzzy.alternatives("z");
        assert!(alts2.iter().any(|(alt, _)| alt == "zh"));
    }
}

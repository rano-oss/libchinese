/*!
SingleGram — compact token -> frequency container and merge helper.

This file provides an in-memory, correctness-first implementation of the
SingleGram abstraction used in the upstream libpinyin code. It mirrors the
runtime semantics required by the lookup and training code:

- store total frequency (u32) and a sorted list of (token, freq) pairs
- support lookup, insert, set, remove and range search
- produce normalized frequencies (freq / total) used by lookup scoring
- merge system + user grams into a merged SingleGram with summed counts

The implementation favors clarity and parity with upstream behavior rather
than maximal performance or a disk-backed store.
*/

use std::cmp::Ordering;

/// Token type used in phrase tables (upstream uses integer token ids).
pub type PhraseToken = u32;

/// SingleGram stores a total frequency and a sorted vector of (token, freq).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SingleGram {
    total_freq: u32,
    /// Sorted by token ascending and unique tokens.
    items: Vec<(PhraseToken, u32)>,
}

impl SingleGram {
    /// Create an empty SingleGram.
    pub fn new() -> Self {
        Self {
            total_freq: 0,
            items: Vec::new(),
        }
    }

    /// Get total frequency.
    pub fn get_total_freq(&self) -> u32 {
        self.total_freq
    }

    /// Set total frequency.
    pub fn set_total_freq(&mut self, total: u32) {
        self.total_freq = total;
    }

    /// Return number of stored items (distinct tokens).
    pub fn get_length(&self) -> usize {
        self.items.len()
    }

    /// Insert a token with given frequency.
    ///
    /// Returns true if inserted; if token already exists, does not modify and
    /// returns false (matches upstream `insert_freq` semantics).
    pub fn insert_freq(&mut self, token: PhraseToken, freq: u32) -> bool {
        match self.items.binary_search_by(|(t, _)| t.cmp(&token)) {
            Ok(_) => false,
            Err(idx) => {
                self.items.insert(idx, (token, freq));
                true
            }
        }
    }

    /// Set an existing token's frequency.
    ///
    /// Returns true if the token existed and was updated, false if not found.
    pub fn set_freq(&mut self, token: PhraseToken, freq: u32) -> bool {
        match self.items.binary_search_by(|(t, _)| t.cmp(&token)) {
            Ok(idx) => {
                self.items[idx].1 = freq;
                true
            }
            Err(_) => false,
        }
    }

    /// Remove a token. Returns Some(freq) if removed, or None if not found.
    pub fn remove_freq(&mut self, token: PhraseToken) -> Option<u32> {
        match self.items.binary_search_by(|(t, _)| t.cmp(&token)) {
            Ok(idx) => {
                let (_, freq) = self.items.remove(idx);
                Some(freq)
            }
            Err(_) => None,
        }
    }

    /// Retrieve raw frequency for a token, if present.
    pub fn get_freq(&self, token: PhraseToken) -> Option<u32> {
        match self.items.binary_search_by(|(t, _)| t.cmp(&token)) {
            Ok(idx) => Some(self.items[idx].1),
            Err(_) => None,
        }
    }

    /// Search for tokens inside [range_begin, range_end) and return a vector of
    /// (token, normalized_freq) where normalized_freq = freq / total_freq as f32.
    /// If total_freq == 0 the normalized_freq will be 0.0.
    pub fn search_range(
        &self,
        range_begin: PhraseToken,
        range_end: PhraseToken,
    ) -> Vec<(PhraseToken, f32)> {
        let mut out = Vec::new();
        if range_begin >= range_end || self.items.is_empty() {
            return out;
        }

        // Find first index where token >= range_begin using binary search.
        let start = match self.items.binary_search_by(|(t, _)| {
            if *t < range_begin {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        }) {
            Ok(idx) => idx,
            Err(idx) => idx,
        };

        let total = self.total_freq as f32;
        for i in start..self.items.len() {
            let (tok, freq) = self.items[i];
            if tok >= range_end {
                break;
            }
            let norm = if total > 0.0 {
                (freq as f32) / total
            } else {
                0.0
            };
            out.push((tok, norm));
        }
        out
    }

    /// Retrieve all items as (token, count, normalized_freq).
    pub fn retrieve_all(&self) -> Vec<(PhraseToken, u32, f32)> {
        let total = self.total_freq as f32;
        self.items
            .iter()
            .map(|(t, c)| {
                (
                    *t,
                    *c,
                    if total > 0.0 {
                        (*c as f32) / total
                    } else {
                        0.0
                    },
                )
            })
            .collect()
    }

    /// Mask out items whose `(token & mask) != value`. Returns number of removed items.
    ///
    /// Upstream `mask_out` removes items that don't satisfy the mask equality.
    pub fn mask_out(&mut self, mask: PhraseToken, value: PhraseToken) -> u32 {
        let mut removed = 0u32;
        let mut i = 0usize;
        while i < self.items.len() {
            let (tok, freq) = self.items[i];
            if (tok & mask) != value {
                self.items.remove(i);
                self.total_freq = self.total_freq.saturating_sub(freq);
                removed += 1;
            } else {
                i += 1;
            }
        }
        removed
    }
}

/// Merge two SingleGram instances into `merged` following upstream semantics.
///
/// - If both `system` and `user` are None, return false.
/// - If one is None, copy the other into merged.
/// - Otherwise, set merged.total_freq = system.total + user.total and merge
///   the two sorted item lists by token; for equal tokens sum frequencies.
///
/// Returns true on success.
pub fn merge_single_gram(
    merged: &mut SingleGram,
    system: Option<&SingleGram>,
    user: Option<&SingleGram>,
) -> bool {
    match (system, user) {
        (None, None) => false,
        (Some(s), None) => {
            merged.total_freq = s.total_freq;
            merged.items = s.items.clone();
            true
        }
        (None, Some(u)) => {
            merged.total_freq = u.total_freq;
            merged.items = u.items.clone();
            true
        }
        (Some(s), Some(u)) => {
            let sys_total = s.total_freq;
            let user_total = u.total_freq;
            merged.total_freq = sys_total.saturating_add(user_total);

            merged.items.clear();
            merged.items.reserve(s.items.len() + u.items.len());

            let mut i = 0usize;
            let mut j = 0usize;
            while i < s.items.len() && j < u.items.len() {
                let (st, sf) = s.items[i];
                let (ut, uf) = u.items[j];
                if st < ut {
                    merged.items.push((st, sf));
                    i += 1;
                } else if st > ut {
                    merged.items.push((ut, uf));
                    j += 1;
                } else {
                    merged.items.push((st, sf.saturating_add(uf)));
                    i += 1;
                    j += 1;
                }
            }
            while i < s.items.len() {
                merged.items.push(s.items[i]);
                i += 1;
            }
            while j < u.items.len() {
                merged.items.push(u.items[j]);
                j += 1;
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_insert_and_get() {
        let mut g = SingleGram::new();
        assert_eq!(g.get_total_freq(), 0);
        assert_eq!(g.get_length(), 0);

        // insert a few tokens
        assert!(g.insert_freq(2, 10));
        assert!(g.insert_freq(5, 20));
        assert!(g.insert_freq(1, 5));
        // insertion of existing token should return false
        assert!(!g.insert_freq(5, 3));

        // items should be sorted by token
        assert_eq!(g.get_length(), 3);
        assert_eq!(g.get_freq(1), Some(5));
        assert_eq!(g.get_freq(2), Some(10));
        assert_eq!(g.get_freq(5), Some(20));
    }

    #[test]
    fn set_and_remove_freq() {
        let mut g = SingleGram::new();
        g.insert_freq(3, 7);
        assert_eq!(g.get_freq(3), Some(7));
        assert!(g.set_freq(3, 42));
        assert_eq!(g.get_freq(3), Some(42));
        assert_eq!(g.remove_freq(3), Some(42));
        assert_eq!(g.get_freq(3), None);
    }

    #[test]
    fn retrieve_all_normalized() {
        let mut g = SingleGram::new();
        g.insert_freq(2, 10);
        g.insert_freq(4, 30);
        g.set_total_freq(40);
        let all = g.retrieve_all();
        assert_eq!(all.len(), 2);
        // tokens in ascending order
        assert_eq!(all[0].0, 2);
        assert_eq!(all[1].0, 4);
        // normalized frequencies
        assert!((all[0].2 - 0.25).abs() < 1e-6);
        assert!((all[1].2 - 0.75).abs() < 1e-6);
    }

    #[test]
    fn search_range_behaviour() {
        let mut g = SingleGram::new();
        g.insert_freq(1, 1);
        g.insert_freq(3, 3);
        g.insert_freq(5, 5);
        g.set_total_freq(9);
        let res = g.search_range(2, 6);
        // should include tokens 3 and 5
        assert_eq!(res.len(), 2);
        assert_eq!(res[0].0, 3);
        assert_eq!(res[1].0, 5);
        assert!((res[0].1 - (3.0 / 9.0)).abs() < 1e-6);
    }

    #[test]
    fn mask_out_removes_items_and_adjusts_total() {
        let mut g = SingleGram::new();
        g.insert_freq(1, 2);
        g.insert_freq(2, 3);
        g.insert_freq(3, 5);
        g.set_total_freq(10);
        let removed = g.mask_out(1, 0);
        assert!(removed > 0);
        assert!(g.get_total_freq() <= 10);
    }

    #[test]
    fn merge_single_gram_combines_sorted_lists() {
        let mut s = SingleGram::new();
        s.insert_freq(1, 10);
        s.insert_freq(3, 30);
        s.set_total_freq(40);

        let mut u = SingleGram::new();
        u.insert_freq(2, 5);
        u.insert_freq(3, 7);
        u.set_total_freq(12);

        let mut merged = SingleGram::new();
        let ok = merge_single_gram(&mut merged, Some(&s), Some(&u));
        assert!(ok);
        // merged total = 40 + 12 = 52
        assert_eq!(merged.get_total_freq(), 52);
        // tokens: 1,2,3 with freq 10,5,37
        assert_eq!(merged.get_freq(1), Some(10));
        assert_eq!(merged.get_freq(2), Some(5));
        assert_eq!(merged.get_freq(3), Some(37));
    }
}

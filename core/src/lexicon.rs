//! Lexicon abstraction for libchinese-core
//!
//! This module provides a simple, serializable in-memory lexicon mapping a
//! canonical pinyin (or zhuyin) key to a list of phrases. It is intended as a
//! correctness-first implementation that can be replaced later with a compact
//! `fst`-backed index and a separate phrase-store.
//!
//! Reference upstream implementation: `libpinyin/src/storage/phrase_index.*`
//!
//! Public API:
//! - `PhraseEntry` — lightweight phrase metadata (currently only `text`)
//! - `Lexicon` — primary lookup/insert API, bincode (de)serialization helpers
//!
//! Notes:
//! - The key convention (how syllables are joined) is language/crate-specific.
//!   Downstream crates (libpinyin / libzhuyin) should pick a canonical joiner
//!   (eg. `""` or `" "`) and consistently use that when loading / querying the
//!   lexicon.
//! - This is intentionally simple to make unit-testing and early integration
//!   easier. Performance/space optimizations (fst, memory-mapping, compressed
//!   phrase stores) come in later phases.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

/// A single lexicon phrase entry.
///
/// For now this is minimal; we store the canonical phrase text and a simple
/// frequency count. Downstream code can extend this with attributes (POS,
/// source flags, offsets, etc).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhraseEntry {
    pub text: String,
    /// Simple frequency / weight. Higher means more frequent.
    pub freq: u64,
}

impl PhraseEntry {
    pub fn new<T: Into<String>>(text: T, freq: u64) -> Self {
        Self {
            text: text.into(),
            freq,
        }
    }
}

/// In-memory lexicon mapping a joined phonetic key -> Vec<PhraseEntry>.
///
/// This struct is serializable with `serde` and (de)serializable with `bincode`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Lexicon {
    map: HashMap<String, Vec<PhraseEntry>>,
}

impl Lexicon {
    /// Create an empty lexicon.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Insert a phrase for a given key.
    ///
    /// If the same phrase already exists for the key, its frequency will be
    /// incremented by `freq`, otherwise it will be pushed as a new entry.
    pub fn insert<K: Into<String>, T: Into<String>>(&mut self, key: K, phrase: T, freq: u64) {
        let key = key.into();
        let phrase = phrase.into();
        let bucket = self.map.entry(key).or_default();
        if let Some(e) = bucket.iter_mut().find(|e| e.text == phrase) {
            e.freq = e.freq.saturating_add(freq);
        } else {
            bucket.push(PhraseEntry::new(phrase, freq));
        }
    }

    /// Directly push a PhraseEntry for a key (useful for bulk-loading).
    pub fn push_entry<K: Into<String>>(&mut self, key: K, entry: PhraseEntry) {
        let key = key.into();
        self.map.entry(key).or_default().push(entry);
    }

    /// Lookup phrases for a given key.
    ///
    /// Returns a Vec of phrase texts. The returned vector is a clone of stored
    /// phrase texts to simplify ownership for callers. Downstream code can use
    /// `lookup_entries` to get full metadata.
    pub fn lookup(&self, key: &str) -> Vec<String> {
        self.map
            .get(key)
            .map(|v| v.iter().map(|e| e.text.clone()).collect())
            .unwrap_or_default()
    }

    /// Lookup full phrase entries (with frequency metadata).
    pub fn lookup_entries(&self, key: &str) -> Vec<PhraseEntry> {
        self.map.get(key).cloned().unwrap_or_default()
    }

    /// Remove a phrase for a given key. Returns true if removed.
    pub fn remove_phrase<K: AsRef<str>, T: AsRef<str>>(&mut self, key: K, phrase: T) -> bool {
        if let Some(bucket) = self.map.get_mut(key.as_ref()) {
            let before = bucket.len();
            bucket.retain(|e| e.text != phrase.as_ref());
            let after = bucket.len();
            return after != before;
        }
        false
    }

    /// Simple demo loader with a few phrases for smoke-testing.
    pub fn load_demo() -> Self {
        let mut lx = Self::new();
        lx.insert("nihao", "你好", 10);
        lx.insert("nihao", "你号", 1);
        lx.insert("zhongguo", "中国", 20);
        lx.insert("zhongguo", "中华", 4);
        lx
    }

    /// Save the lexicon to a file using bincode serialization.
    pub fn save_bincode<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        bincode::serialize_into(writer, self)?;
        Ok(())
    }

    /// Load the lexicon from a bincode file produced by `save_bincode`.
    pub fn load_bincode<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let model: Self = bincode::deserialize_from(reader)?;
        Ok(model)
    }

    /// Return the number of keys in the lexicon.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Return true if the lexicon is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_lookup() {
        let mut lx = Lexicon::new();
        lx.insert("nihao", "你好", 1);
        lx.insert("nihao", "你号", 2);
        let res = lx.lookup("nihao");
        assert_eq!(res.len(), 2);
        assert!(res.contains(&"你好".to_string()));
        assert!(res.contains(&"你号".to_string()));
    }

    #[test]
    fn duplicate_insert_increments_freq() {
        let mut lx = Lexicon::new();
        lx.insert("k", "x", 1);
        lx.insert("k", "x", 3);
        let entries = lx.lookup_entries("k");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].freq, 4);
    }

    #[test]
    fn save_and_load_bincode_roundtrip() {
        let tmp = std::env::temp_dir().join("libchinese_lexicon_test.bin");
        let mut lx = Lexicon::new();
        lx.insert("a", "甲", 5);
        lx.insert("b", "乙", 2);
        lx.save_bincode(&tmp).unwrap();
        let loaded = Lexicon::load_bincode(&tmp).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.lookup("a"), vec!["甲".to_string()]);
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn remove_phrase_works() {
        let mut lx = Lexicon::new();
        lx.insert("k", "x", 1);
        assert!(lx.remove_phrase("k", "x"));
        assert!(lx.lookup("k").is_empty());
    }
}

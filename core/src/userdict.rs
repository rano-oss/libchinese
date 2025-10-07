//! User dictionary and configuration skeletons for libchinese-core.
//!
//! Responsibilities implemented here:
//! - `Config`: serde-deserializable config for fuzzy rules and n-gram weights.
//! - `InMemoryUserDict`: thread-safe in-memory user dictionary (learn/merge/frequency).
//! - `UserDict` enum: a simple backend switch (in-memory by default; `redb` backend
//!    can be added behind a feature flag in the future).
//!
//! This file contains a correctness-first implementation designed to be replaced
//! later with a persistent `redb`-backed implementation and/or more advanced
//! merging strategies. It is intentionally small and well-documented so it can
//! be used in early phases and tests.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Configuration for fuzzy matching and n-gram interpolation.
///
/// Designed to be deserialized from TOML (via `serde`). Fields are intentionally
/// conservative and can be extended as needed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Fuzzy equivalence rules as textual pairs (e.g. "zh=z", "ch=c").
    /// Parsers or language-specific crates should parse these into efficient maps.
    pub fuzzy: Vec<String>,

    /// Interpolation weights for n-gram scoring. These should typically be
    /// non-negative and sum to ~1.0 but normalization is performed at scoring
    /// time if necessary.
    pub unigram_weight: f32,
    pub bigram_weight: f32,
    pub trigram_weight: f32,

    /// A penalty applied (in ln-space or scaled units) when a fuzzy substitution
    /// is used. Larger values penalize fuzzy matches more heavily.
    pub fuzzy_penalty: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            fuzzy: vec![
                "zh=z".to_string(),
                "ch=c".to_string(),
                "sh=s".to_string(),
                "l=n".to_string(),
            ],
            unigram_weight: 0.6,
            bigram_weight: 0.3,
            trigram_weight: 0.1,
            fuzzy_penalty: 1.0, // default penalty (tunable)
        }
    }
}

/// A thread-safe in-memory user dictionary.
///
/// Stores learned phrases and their counts. This implementation is useful for
/// unit tests and early-stage runtime behavior. It is intentionally simple:
/// the counts are u64 and merge semantics simply add frequencies.
#[derive(Clone, Debug)]
pub struct InMemoryUserDict {
    inner: Arc<RwLock<HashMap<String, u64>>>,
}

impl InMemoryUserDict {
    /// Create a new empty in-memory user dictionary.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Learn a phrase: increment its count by 1.
    ///
    /// This method is safe to call concurrently from multiple threads.
    pub fn learn(&self, phrase: &str) {
        if let Ok(mut map) = self.inner.write() {
            let entry = map.entry(phrase.to_string()).or_insert(0);
            *entry = entry.saturating_add(1);
        }
    }

    /// Learn a phrase with a custom increment (useful for import or batch updates).
    pub fn learn_with_count(&self, phrase: &str, delta: u64) {
        if delta == 0 {
            return;
        }
        if let Ok(mut map) = self.inner.write() {
            let entry = map.entry(phrase.to_string()).or_insert(0);
            *entry = entry.saturating_add(delta);
        }
    }

    /// Get the learned frequency for a phrase.
    pub fn frequency(&self, phrase: &str) -> u64 {
        if let Ok(map) = self.inner.read() {
            map.get(phrase).copied().unwrap_or(0)
        } else {
            0
        }
    }

    /// Merge another in-memory user dictionary into this one (summing frequencies).
    pub fn merge_from(&self, other: &InMemoryUserDict) {
        if let (Ok(mut dst), Ok(src)) = (self.inner.write(), other.inner.read()) {
            for (k, v) in src.iter() {
                let entry = dst.entry(k.clone()).or_insert(0);
                *entry = entry.saturating_add(*v);
            }
        }
    }

    /// Export a snapshot of the current data (useful for serialization or tests).
    pub fn snapshot(&self) -> HashMap<String, u64> {
        if let Ok(map) = self.inner.read() {
            map.clone()
        } else {
            HashMap::new()
        }
    }

    /// Replace the entire contents of this dict with the provided map.
    /// Useful for loading persisted state atomically.
    pub fn replace_with(&self, data: HashMap<String, u64>) {
        if let Ok(mut map) = self.inner.write() {
            *map = data;
        }
    }
}

/// A lightweight, ergonomic enum used by higher-level code to select a userdict backend.
///
/// Implements two backends:
/// - `InMemory` (default): fast, thread-safe in-memory map used for tests and
///   early runtime.
/// - `Redb`: persistent, ACID-backed storage using `redb`. This is optional but
///   provided here as a basic implementation for persistence.
#[derive(Clone, Debug)]
pub enum UserDict {
    InMemory(InMemoryUserDict),
    Redb(RedbUserDict),
}

impl UserDict {
    /// Construct a default `InMemory` user dictionary.
    pub fn new_in_memory() -> Self {
        UserDict::InMemory(InMemoryUserDict::new())
    }

    /// Construct a new `Redb`-backed user dictionary at the provided path.
    ///
    /// Returns an error if the database cannot be created/opened.
    pub fn new_redb<P: AsRef<std::path::Path>>(path: P) -> Result<Self, redb::Error> {
        Ok(UserDict::Redb(RedbUserDict::new(path)?))
    }

    /// Learn a phrase (increment by 1) using the selected backend.
    pub fn learn(&self, phrase: &str) {
        match self {
            UserDict::InMemory(m) => m.learn(phrase),
            UserDict::Redb(r) => {
                let _ = r.learn(phrase);
            }
        }
    }

    /// Learn with a custom delta.
    pub fn learn_with_count(&self, phrase: &str, delta: u64) {
        match self {
            UserDict::InMemory(m) => m.learn_with_count(phrase, delta),
            UserDict::Redb(r) => {
                let _ = r.learn_with_count(phrase, delta);
            }
        }
    }

    /// Get frequency for phrase.
    pub fn frequency(&self, phrase: &str) -> u64 {
        match self {
            UserDict::InMemory(m) => m.frequency(phrase),
            UserDict::Redb(r) => r.frequency(phrase).unwrap_or(0),
        }
    }

    /// Merge another UserDict into this one. Cross-backend merges are supported by
    /// exporting the source snapshot and applying to the destination backend.
    pub fn merge_from(&self, other: &UserDict) {
        match (self, other) {
            (UserDict::InMemory(dst), UserDict::InMemory(src)) => dst.merge_from(src),
            (UserDict::Redb(dst), UserDict::Redb(src)) => {
                // attempt an efficient merge by iterating src and incrementing dst
                if let Ok(snapshot) = src.iter_snapshot() {
                    for (k, v) in snapshot.into_iter() {
                        let _ = dst.learn_with_count(&k, v);
                    }
                }
            }
            // Cross-backend merges: snapshot source and apply to destination
            (UserDict::InMemory(dst), src_other) => {
                let snap = src_other.snapshot();
                for (k, v) in snap.into_iter() {
                    dst.learn_with_count(&k, v);
                }
            }
            (UserDict::Redb(dst), src_other) => {
                let snap = src_other.snapshot();
                for (k, v) in snap.into_iter() {
                    let _ = dst.learn_with_count(&k, v);
                }
            }
        }
    }

    /// Snapshot the contents as a HashMap (cloned).
    pub fn snapshot(&self) -> HashMap<String, u64> {
        match self {
            UserDict::InMemory(m) => m.snapshot(),
            UserDict::Redb(r) => r.snapshot().unwrap_or_default(),
        }
    }

    /// Convenience: iterate over all entries (in-memory or redb) as a vector of pairs.
    pub fn iter_all(&self) -> Vec<(String, u64)> {
        match self {
            UserDict::InMemory(m) => m.snapshot().into_iter().collect(),
            UserDict::Redb(r) => r.iter_all().unwrap_or_default(),
        }
    }
}

/// Redb-backed user dictionary implementation.
///
/// This provides basic persistent semantics:
/// - atomic increments via a write transaction
/// - read access via read transactions
///
/// Note: this implementation keeps things simple for the first pass. It can be
/// extended with batching, async flush, and compaction later.
pub struct RedbUserDict {
    db: redb::DatabaseOpened,
    // we store the path for informational/debugging uses
    #[allow(dead_code)]
    path: std::path::PathBuf,
}

impl RedbUserDict {
    /// Table definition for user phrase counts. We store keys as &str and values as u64.
    const TABLE_DEF: redb::TableDefinition<&'static str, u64> =
        redb::TableDefinition::new("user_dict");

    /// Create or open a redb database at `path`.
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Result<Self, redb::Error> {
        // Ensure parent exists
        if let Some(parent) = path.as_ref().parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let db = redb::Database::create(path.as_ref())?;
        Ok(RedbUserDict {
            db,
            path: path.as_ref().to_path_buf(),
        })
    }

    /// Increment phrase count by 1.
    pub fn learn(&self, phrase: &str) -> Result<(), redb::Error> {
        self.learn_with_count(phrase, 1)
    }

    /// Increment phrase count by `delta`.
    pub fn learn_with_count(&self, phrase: &str, delta: u64) -> Result<(), redb::Error> {
        let write_txn = self.db.begin_write()?;
        let mut table = write_txn.open_table(Self::TABLE_DEF)?;
        // get current value if any
        if let Some(existing) = table.get(phrase)? {
            let cur = existing.value();
            let new = cur.saturating_add(delta);
            table.insert(phrase, &new)?;
        } else {
            table.insert(phrase, &delta)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get frequency for phrase.
    pub fn frequency(&self, phrase: &str) -> Result<u64, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(Self::TABLE_DEF)?;
        if let Some(val) = table.get(phrase)? {
            Ok(val.value())
        } else {
            Ok(0)
        }
    }

    /// Snapshot full contents into a HashMap.
    pub fn snapshot(&self) -> Result<HashMap<String, u64>, redb::Error> {
        let mut out = HashMap::new();
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(Self::TABLE_DEF)?;
        for item in table.iter()? {
            let (k, v) = item?;
            out.insert(k.to_string(), v.value());
        }
        Ok(out)
    }

    /// Iterate returning a Vec<(String, u64)>. Useful for merges or exports.
    pub fn iter_all(&self) -> Result<Vec<(String, u64)>, redb::Error> {
        let mut out = Vec::new();
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(Self::TABLE_DEF)?;
        for item in table.iter()? {
            let (k, v) = item?;
            out.push((k.to_string(), v.value()));
        }
        Ok(out)
    }

    /// Convenience: return snapshot as Vec for quick merging operations.
    pub fn iter_snapshot(&self) -> Result<Vec<(String, u64)>, redb::Error> {
        self.iter_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_learn_and_frequency() {
        let d = InMemoryUserDict::new();
        assert_eq!(d.frequency("你好"), 0);
        d.learn("你好");
        assert_eq!(d.frequency("你好"), 1);
        d.learn_with_count("你好", 4);
        assert_eq!(d.frequency("你好"), 5);
    }

    #[test]
    fn in_memory_merge() {
        let a = InMemoryUserDict::new();
        let b = InMemoryUserDict::new();
        a.learn_with_count("a", 2);
        b.learn_with_count("a", 3);
        b.learn_with_count("b", 1);

        a.merge_from(&b);
        assert_eq!(a.frequency("a"), 5);
        assert_eq!(a.frequency("b"), 1);
    }

    #[test]
    fn userdict_enum_roundtrip_snapshot() {
        let u = UserDict::new_in_memory();
        u.learn("x");
        u.learn_with_count("y", 2);
        let snap = u.snapshot();
        assert_eq!(snap.get("x").copied().unwrap_or(0), 1);
        assert_eq!(snap.get("y").copied().unwrap_or(0), 2);
    }

    #[test]
    fn config_defaults_present() {
        let cfg = Config::default();
        assert!(cfg.unigram_weight > 0.0);
        assert!(cfg.fuzzy.len() >= 1);
    }
}

//! Redb-first UserDict implementation for core.
//!
//! This file exports `UserDict` whose public API is small and test-friendly.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use redb::{Database, ReadableTable, TableDefinition};

/// UserDict backed by `redb`.
#[derive(Clone, Debug)]
pub struct UserDict {
    db: Arc<Database>,
    #[allow(dead_code)]
    path: PathBuf,
}

impl UserDict {
    /// Create/open a redb-backed user dict at the given path.
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Result<Self, redb::Error> {
        if let Some(dir) = path.as_ref().parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let db = Database::create(path.as_ref())?;
        Ok(UserDict {
            db: Arc::new(db),
            path: path.as_ref().to_path_buf(),
        })
    }

    fn table_def() -> TableDefinition<'static, &'static str, u64> {
        TableDefinition::new("user_dict")
    }

    fn bigram_table_def() -> TableDefinition<'static, &'static str, u64> {
        TableDefinition::new("user_bigram")
    }

    /// Encode bigram key as "w1\0w2" for redb storage.
    fn encode_bigram_key(w1: &str, w2: &str) -> String {
        format!("{}\0{}", w1, w2)
    }

    /// Decode bigram key from "w1\0w2" format.
    fn decode_bigram_key(key: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = key.split('\0').collect();
        if parts.len() == 2 {
            Some((parts[0].to_string(), parts[1].to_string()))
        } else {
            None
        }
    }

    /// Learn a phrase (increment by 1).
    pub fn learn(&self, phrase: &str) {
        let _ = self.learn_with_count(phrase, 1);
    }

    /// Learn with a custom delta.
    pub fn learn_with_count(&self, phrase: &str, delta: u64) -> Result<(), redb::Error> {
        // Read current value in a read transaction to avoid borrow conflicts
        let cur = {
            let r = self.db.begin_read()?;
            match r.open_table(Self::table_def()) {
                Ok(table) => {
                    if let Some(existing) = table.get(&phrase)? {
                        existing.value()
                    } else {
                        0u64
                    }
                }
                Err(e) => {
                    // open_table returns a redb::TableError; treat missing table as empty
                    if matches!(e, redb::TableError::TableDoesNotExist(_)) {
                        0u64
                    } else {
                        return Err(e.into());
                    }
                }
            }
        };

        let w = self.db.begin_write()?;
        {
            let mut table = w.open_table(Self::table_def())?;
            let new = cur.saturating_add(delta);
            table.insert(&phrase, &new)?;
        }
        w.commit()?;
        Ok(())
    }

    /// Get frequency for phrase.
    pub fn frequency(&self, phrase: &str) -> u64 {
        self.frequency_result(phrase).unwrap_or(0)
    }

    fn frequency_result(&self, phrase: &str) -> Result<u64, redb::Error> {
        let r = self.db.begin_read()?;
        match r.open_table(Self::table_def()) {
            Ok(table) => {
                if let Some(v) = table.get(&phrase)? {
                    Ok(v.value())
                } else {
                    Ok(0)
                }
            }
            Err(e) => {
                if matches!(e, redb::TableError::TableDoesNotExist(_)) {
                    Ok(0)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    /// Snapshot full contents as a HashMap.
    pub fn snapshot(&self) -> HashMap<String, u64> {
        self.snapshot_result().unwrap_or_default()
    }

    fn snapshot_result(&self) -> Result<HashMap<String, u64>, redb::Error> {
        let mut out = HashMap::new();
        let r = self.db.begin_read()?;
        match r.open_table(Self::table_def()) {
            Ok(table) => {
                for item in table.iter()? {
                    let (k, v) = item?;
                    out.insert(k.value().to_string(), v.value());
                }
            }
            Err(e) => {
                if matches!(e, redb::TableError::TableDoesNotExist(_)) {
                    // treat as empty
                } else {
                    return Err(e.into());
                }
            }
        }
        Ok(out)
    }

    /// Iterate all entries as Vec<(String,u64)>.
    pub fn iter_all(&self) -> Vec<(String, u64)> {
        self.iter_all_result().unwrap_or_default()
    }

    fn iter_all_result(&self) -> Result<Vec<(String, u64)>, redb::Error> {
        let mut out = Vec::new();
        let r = self.db.begin_read()?;
        match r.open_table(Self::table_def()) {
            Ok(table) => {
                for item in table.iter()? {
                    let (k, v) = item?;
                    out.push((k.value().to_string(), v.value()));
                }
            }
            Err(e) => {
                if matches!(e, redb::TableError::TableDoesNotExist(_)) {
                    // empty
                } else {
                    return Err(e.into());
                }
            }
        }
        Ok(out)
    }

    // ========== User Bigram Learning API ==========

    /// Learn a bigram (w1 → w2) by incrementing its count.
    ///
    /// This is used for prediction learning - when user selects a prediction,
    /// we record the bigram relationship to improve future predictions.
    ///
    /// # Example
    /// ```ignore
    /// userdict.learn_bigram("你", "好");  // User selected "好" after "你"
    /// ```
    pub fn learn_bigram(&self, w1: &str, w2: &str) {
        let _ = self.learn_bigram_with_count(w1, w2, 1);
    }

    /// Learn a bigram with a custom count delta.
    pub fn learn_bigram_with_count(
        &self,
        w1: &str,
        w2: &str,
        delta: u64,
    ) -> Result<(), redb::Error> {
        let key = Self::encode_bigram_key(w1, w2);

        // Read current value
        let cur = {
            let r = self.db.begin_read()?;
            match r.open_table(Self::bigram_table_def()) {
                Ok(table) => {
                    if let Some(existing) = table.get(key.as_str())? {
                        existing.value()
                    } else {
                        0u64
                    }
                }
                Err(e) => {
                    if matches!(e, redb::TableError::TableDoesNotExist(_)) {
                        0u64
                    } else {
                        return Err(e.into());
                    }
                }
            }
        };

        // Write new value
        let w = self.db.begin_write()?;
        {
            let mut table = w.open_table(Self::bigram_table_def())?;
            let new = cur.saturating_add(delta);
            table.insert(key.as_str(), &new)?;
        }
        w.commit()?;
        Ok(())
    }

    /// Get the frequency count for a specific bigram (w1 → w2).
    pub fn bigram_frequency(&self, w1: &str, w2: &str) -> u64 {
        self.bigram_frequency_result(w1, w2).unwrap_or(0)
    }

    fn bigram_frequency_result(&self, w1: &str, w2: &str) -> Result<u64, redb::Error> {
        let key = Self::encode_bigram_key(w1, w2);
        let r = self.db.begin_read()?;
        match r.open_table(Self::bigram_table_def()) {
            Ok(table) => {
                if let Some(v) = table.get(key.as_str())? {
                    Ok(v.value())
                } else {
                    Ok(0)
                }
            }
            Err(e) => {
                if matches!(e, redb::TableError::TableDoesNotExist(_)) {
                    Ok(0)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    /// Get all bigrams that start with w1, returning (w2 → count) mapping.
    ///
    /// This is used during prediction to merge user-learned bigrams with
    /// the static n-gram model.
    ///
    /// # Example
    /// ```ignore
    /// let bigrams = userdict.get_bigrams_after("好");
    /// // Returns: {"的" => 5, "吗" => 3, "啊" => 1}
    /// ```
    pub fn get_bigrams_after(&self, w1: &str) -> HashMap<String, u64> {
        self.get_bigrams_after_result(w1).unwrap_or_default()
    }

    fn get_bigrams_after_result(&self, w1: &str) -> Result<HashMap<String, u64>, redb::Error> {
        let mut out = HashMap::new();
        let r = self.db.begin_read()?;
        let prefix = format!("{}\0", w1);

        match r.open_table(Self::bigram_table_def()) {
            Ok(table) => {
                // Iterate through all bigrams
                for item in table.iter()? {
                    let (key, count) = item?;
                    let key_str = key.value();

                    // Check if key starts with our prefix
                    if key_str.starts_with(&prefix) {
                        if let Some((_, w2)) = Self::decode_bigram_key(key_str) {
                            out.insert(w2, count.value());
                        }
                    }
                }
            }
            Err(e) => {
                if matches!(e, redb::TableError::TableDoesNotExist(_)) {
                    // Empty table, return empty map
                } else {
                    return Err(e.into());
                }
            }
        }
        Ok(out)
    }

    /// Get all user bigrams as a snapshot.
    ///
    /// Returns HashMap of ((w1, w2) → count). Useful for debugging and testing.
    pub fn snapshot_bigrams(&self) -> HashMap<(String, String), u64> {
        self.snapshot_bigrams_result().unwrap_or_default()
    }

    fn snapshot_bigrams_result(&self) -> Result<HashMap<(String, String), u64>, redb::Error> {
        let mut out = HashMap::new();
        let r = self.db.begin_read()?;
        match r.open_table(Self::bigram_table_def()) {
            Ok(table) => {
                for item in table.iter()? {
                    let (key, count) = item?;
                    if let Some((w1, w2)) = Self::decode_bigram_key(key.value()) {
                        out.insert((w1, w2), count.value());
                    }
                }
            }
            Err(e) => {
                if matches!(e, redb::TableError::TableDoesNotExist(_)) {
                    // Empty
                } else {
                    return Err(e.into());
                }
            }
        }
        Ok(out)
    }

    // ========== User Phrase Management API for GUI ==========

    /// List all phrases in user dictionary (alias for iter_all for clarity).
    ///
    /// Returns a vector of (phrase, frequency) tuples.
    /// This method is intended for GUI display of all learned phrases.
    pub fn list_all(&self) -> Vec<(String, u64)> {
        self.iter_all()
    }

    /// Add a phrase manually with specified frequency.
    ///
    /// This overwrites any existing entry for the phrase.
    /// Used by GUI to manually add custom phrases.
    ///
    /// # Arguments
    /// * `phrase` - The phrase text to add
    /// * `frequency` - Initial frequency (higher = more likely to appear)
    pub fn add_phrase(&self, phrase: &str, frequency: u64) -> Result<(), redb::Error> {
        let w = self.db.begin_write()?;
        {
            let mut table = w.open_table(Self::table_def())?;
            table.insert(&phrase, &frequency)?;
        }
        w.commit()?;
        Ok(())
    }

    /// Delete a phrase from the user dictionary.
    ///
    /// If the phrase doesn't exist, this is a no-op.
    /// Used by GUI to remove unwanted learned phrases.
    pub fn delete_phrase(&self, phrase: &str) -> Result<(), redb::Error> {
        let w = self.db.begin_write()?;
        {
            let mut table = w.open_table(Self::table_def())?;
            table.remove(&phrase)?;
        }
        w.commit()?;
        Ok(())
    }

    /// Update the frequency of an existing phrase.
    ///
    /// This is an alias for `add_phrase` since redb overwrites existing entries.
    /// Used by GUI to manually adjust phrase ranking.
    pub fn update_frequency(&self, phrase: &str, new_freq: u64) -> Result<(), redb::Error> {
        self.add_phrase(phrase, new_freq)
    }

    /// Search phrases by prefix (for GUI filtering).
    ///
    /// Returns all phrases starting with the given prefix.
    pub fn search_by_prefix(&self, prefix: &str) -> Result<Vec<(String, u64)>, redb::Error> {
        let mut results = Vec::new();
        let r = self.db.begin_read()?;
        match r.open_table(Self::table_def()) {
            Ok(table) => {
                for item in table.iter()? {
                    let (k, v) = item?;
                    let phrase = k.value().to_string();
                    if phrase.starts_with(prefix) {
                        results.push((phrase, v.value()));
                    }
                }
            }
            Err(e) => {
                if matches!(e, redb::TableError::TableDoesNotExist(_)) {
                    // empty - return empty vec
                } else {
                    return Err(e.into());
                }
            }
        }
        Ok(results)
    }
}

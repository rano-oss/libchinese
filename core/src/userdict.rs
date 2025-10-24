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
    ///
    /// # Example
    /// ```no_run
    /// # use libchinese_core::UserDict;
    /// let dict = UserDict::new("userdict.redb").unwrap();
    /// let all_phrases = dict.list_all();
    /// for (phrase, freq) in all_phrases {
    ///     println!("{}: {}", phrase, freq);
    /// }
    /// ```
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
    ///
    /// # Example
    /// ```no_run
    /// # use libchinese_core::UserDict;
    /// let dict = UserDict::new("userdict.redb").unwrap();
    /// dict.add_phrase("你好世界", 100).unwrap();
    /// ```
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
    ///
    /// # Example
    /// ```no_run
    /// # use libchinese_core::UserDict;
    /// let dict = UserDict::new("userdict.redb").unwrap();
    /// dict.delete_phrase("错误短语").unwrap();
    /// ```
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
    ///
    /// # Example
    /// ```no_run
    /// # use libchinese_core::UserDict;
    /// let dict = UserDict::new("userdict.redb").unwrap();
    /// dict.update_frequency("你好", 200).unwrap();
    /// ```
    pub fn update_frequency(&self, phrase: &str, new_freq: u64) -> Result<(), redb::Error> {
        self.add_phrase(phrase, new_freq)
    }

    /// Search phrases by prefix (for GUI filtering).
    ///
    /// Returns all phrases starting with the given prefix.
    ///
    /// # Example
    /// ```no_run
    /// # use libchinese_core::UserDict;
    /// let dict = UserDict::new("userdict.redb").unwrap();
    /// let matches = dict.search_by_prefix("你").unwrap();
    /// // Returns: [("你好", 100), ("你好吗", 50), ...]
    /// ```
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn userdict_roundtrip_redb() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "libchinese_test_userdict_{}.redb",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let u = UserDict::new(&tmp).expect("new");
        u.learn("a");
        u.learn_with_count("b", 3).expect("learn_with_count");
        let snap = u.snapshot();
        assert_eq!(snap.get("a").copied().unwrap_or(0), 1);
        assert_eq!(snap.get("b").copied().unwrap_or(0), 3);
    }

    #[test]
    fn test_add_and_list_phrases() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "test_add_list_{}.redb",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let dict = UserDict::new(&tmp).unwrap();

        dict.add_phrase("测试", 100).unwrap();
        dict.add_phrase("示例", 50).unwrap();
        dict.add_phrase("你好", 200).unwrap();

        let all = dict.list_all();
        assert_eq!(all.len(), 3);

        // Check all phrases exist with correct frequencies
        assert!(all.contains(&("测试".to_string(), 100)));
        assert!(all.contains(&("示例".to_string(), 50)));
        assert!(all.contains(&("你好".to_string(), 200)));
    }

    #[test]
    fn test_delete_phrase() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "test_delete_{}.redb",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let dict = UserDict::new(&tmp).unwrap();

        dict.add_phrase("删除我", 10).unwrap();
        assert_eq!(dict.frequency("删除我"), 10);

        dict.delete_phrase("删除我").unwrap();
        assert_eq!(dict.frequency("删除我"), 0);

        let all = dict.list_all();
        assert_eq!(all.len(), 0);
    }

    #[test]
    fn test_update_frequency() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "test_update_{}.redb",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let dict = UserDict::new(&tmp).unwrap();

        dict.add_phrase("更新", 100).unwrap();
        assert_eq!(dict.frequency("更新"), 100);

        dict.update_frequency("更新", 500).unwrap();
        assert_eq!(dict.frequency("更新"), 500);
    }

    #[test]
    fn test_search_by_prefix() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "test_search_{}.redb",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let dict = UserDict::new(&tmp).unwrap();

        dict.add_phrase("你好", 100).unwrap();
        dict.add_phrase("你好吗", 50).unwrap();
        dict.add_phrase("我好", 30).unwrap();
        dict.add_phrase("你是谁", 40).unwrap();

        let results = dict.search_by_prefix("你").unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.contains(&("你好".to_string(), 100)));
        assert!(results.contains(&("你好吗".to_string(), 50)));
        assert!(results.contains(&("你是谁".to_string(), 40)));

        let results2 = dict.search_by_prefix("我").unwrap();
        assert_eq!(results2.len(), 1);
        assert!(results2.contains(&("我好".to_string(), 30)));
    }

    #[test]
    fn test_add_phrase_overwrites_existing() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "test_overwrite_{}.redb",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let dict = UserDict::new(&tmp).unwrap();

        dict.add_phrase("重复", 100).unwrap();
        assert_eq!(dict.frequency("重复"), 100);

        // Add again with different frequency - should overwrite
        dict.add_phrase("重复", 200).unwrap();
        assert_eq!(dict.frequency("重复"), 200);

        // Should only have one entry
        let all = dict.list_all();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_delete_nonexistent_phrase() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "test_delete_nonexist_{}.redb",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let dict = UserDict::new(&tmp).unwrap();

        // Should not panic or error
        dict.delete_phrase("不存在").unwrap();

        assert_eq!(dict.frequency("不存在"), 0);
        assert_eq!(dict.list_all().len(), 0);
    }

    #[test]
    fn test_learn_bigram_basic() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "test_bigram_basic_{}.redb",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let dict = UserDict::new(&tmp).unwrap();

        // Learn some bigrams
        dict.learn_bigram("你", "好");
        dict.learn_bigram("你", "好"); // Learn again
        dict.learn_bigram("好", "的");

        // Check frequencies
        assert_eq!(dict.bigram_frequency("你", "好"), 2);
        assert_eq!(dict.bigram_frequency("好", "的"), 1);
        assert_eq!(dict.bigram_frequency("你", "是"), 0); // Not learned
    }

    #[test]
    fn test_get_bigrams_after() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "test_bigrams_after_{}.redb",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let dict = UserDict::new(&tmp).unwrap();

        // Learn multiple bigrams starting with "好"
        dict.learn_bigram("好", "的");
        dict.learn_bigram("好", "的");
        dict.learn_bigram("好", "吗");
        dict.learn_bigram("好", "啊");
        dict.learn_bigram("好", "啊");
        dict.learn_bigram("好", "啊");

        let bigrams = dict.get_bigrams_after("好");

        assert_eq!(bigrams.len(), 3);
        assert_eq!(bigrams.get("的"), Some(&2));
        assert_eq!(bigrams.get("吗"), Some(&1));
        assert_eq!(bigrams.get("啊"), Some(&3));

        // Non-existent prefix returns empty
        let empty = dict.get_bigrams_after("不存在");
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_snapshot_bigrams() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "test_snapshot_bigrams_{}.redb",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let dict = UserDict::new(&tmp).unwrap();

        dict.learn_bigram("你", "好");
        dict.learn_bigram("好", "的");
        dict.learn_bigram("我", "是");

        let snapshot = dict.snapshot_bigrams();

        assert_eq!(snapshot.len(), 3);
        assert_eq!(
            snapshot.get(&("你".to_string(), "好".to_string())),
            Some(&1)
        );
        assert_eq!(
            snapshot.get(&("好".to_string(), "的".to_string())),
            Some(&1)
        );
        assert_eq!(
            snapshot.get(&("我".to_string(), "是".to_string())),
            Some(&1)
        );
    }

    #[test]
    fn test_bigram_with_custom_count() {
        let mut tmp = std::env::temp_dir();
        tmp.push(format!(
            "test_bigram_custom_count_{}.redb",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let dict = UserDict::new(&tmp).unwrap();

        // Learn with custom delta (like upstream's initial_seed boost)
        dict.learn_bigram_with_count("你", "好", 10).unwrap();

        assert_eq!(dict.bigram_frequency("你", "好"), 10);

        // Increment again
        dict.learn_bigram("你", "好");
        assert_eq!(dict.bigram_frequency("你", "好"), 11);
    }
}

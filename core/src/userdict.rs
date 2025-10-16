//! Redb-first UserDict implementation for core.
//!
//! This file exports `UserDict` whose public API is small and test-friendly.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use serde::{Serialize, Deserialize};

use redb::{Database, TableDefinition, ReadableTable};

/// Metadata for user dictionary storage format versioning and compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDictMetadata {
    pub version: String,
    pub created_at: String,
    pub last_modified: String,
    pub entry_count: usize,
    pub total_frequency: u64,
}

impl Default for UserDictMetadata {
    fn default() -> Self {
        let now = format!("{:?}", std::time::SystemTime::now());
        Self {
            version: "1.0".to_string(),
            created_at: now.clone(),
            last_modified: now,
            entry_count: 0,
            total_frequency: 0,
        }
    }
}

/// UserDict backed by `redb`. `UserDict::new()` creates a temp redb file so
/// existing tests that call `UserDict::new()` continue to work.
#[derive(Clone, Debug)]
pub struct UserDict {
    inner: Arc<RedbUserDict>,
}

impl UserDict {
    /// Create a new redb-backed user dict in a temporary file.
    pub fn new() -> Self {
        let mut p = std::env::temp_dir();
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let pid = std::process::id();
        p.push(format!(
            "libchinese_userdict_{}_{}.redb",
            pid, now_nanos
        ));
        Self::new_redb(p).expect("create temp redb for userdict")
    }

    /// Create/open a redb-backed user dict at the given path.
    pub fn new_redb<P: AsRef<std::path::Path>>(path: P) -> Result<Self, redb::Error> {
        if let Some(dir) = path.as_ref().parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let db = Database::create(path.as_ref())?;
        Ok(UserDict {
            inner: Arc::new(RedbUserDict {
                db,
                path: path.as_ref().to_path_buf(),
            }),
        })
    }

    /// Learn a phrase (increment by 1).
    pub fn learn(&self, phrase: &str) {
        let _ = self.learn_with_count(phrase, 1);
    }

    /// Learn with a custom delta.
    pub fn learn_with_count(&self, phrase: &str, delta: u64) -> Result<(), redb::Error> {
        self.inner.learn_with_count(phrase, delta)
    }

    /// Get frequency for phrase.
    pub fn frequency(&self, phrase: &str) -> u64 {
        self.inner.frequency(phrase).unwrap_or(0)
    }

    /// Snapshot full contents as a HashMap.
    pub fn snapshot(&self) -> HashMap<String, u64> {
        self.inner.snapshot().unwrap_or_default()
    }

    /// Iterate all entries as Vec<(String,u64)>.
    pub fn iter_all(&self) -> Vec<(String, u64)> {
        self.inner.iter_all().unwrap_or_default()
    }

    /// Get metadata about the user dictionary.
    pub fn get_metadata(&self) -> UserDictMetadata {
        let snapshot = self.snapshot();
        let now = format!("{:?}", std::time::SystemTime::now());
        UserDictMetadata {
            version: "1.0".to_string(),
            created_at: now.clone(), // Could be stored in DB metadata table
            last_modified: now,
            entry_count: snapshot.len(),
            total_frequency: snapshot.values().sum(),
        }
    }

    /// Export metadata to JSON file.
    pub fn export_metadata_json<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let metadata = self.get_metadata();
        let json = serde_json::to_string_pretty(&metadata)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct RedbUserDict {
    db: Database,
    path: PathBuf,
}

impl RedbUserDict {
    fn table_def() -> TableDefinition<'static, &'static str, u64> {
        TableDefinition::new("user_dict")
    }

    fn learn_with_count(&self, phrase: &str, delta: u64) -> Result<(), redb::Error> {
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

    fn frequency(&self, phrase: &str) -> Result<u64, redb::Error> {
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

    fn snapshot(&self) -> Result<HashMap<String, u64>, redb::Error> {
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

    fn iter_all(&self) -> Result<Vec<(String, u64)>, redb::Error> {
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
        let u = UserDict::new_redb(&tmp).expect("new_redb");
        u.learn("a");
        u.learn_with_count("b", 3).expect("learn_with_count");
        let snap = u.snapshot();
        assert_eq!(snap.get("a").copied().unwrap_or(0), 1);
        assert_eq!(snap.get("b").copied().unwrap_or(0), 3);
    }
}

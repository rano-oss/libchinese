//! Redb-first UserDict implementation for core.
//!
//! This file exports `UserDict` whose public API is small and test-friendly.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use redb::{Database, TableDefinition, ReadableTable};

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
}

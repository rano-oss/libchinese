use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
pub struct BigramDB {
    pub version: u32,
    pub total_bigram_counts: u128,
    pub bigram_right_freqs: HashMap<u64, u128>,
}

impl BigramDB {
    pub fn new() -> Self {
        BigramDB { version: 1, total_bigram_counts: 0, bigram_right_freqs: HashMap::new() }
    }

    /// Parse a .table file. Supports simple line formats:
    /// - "left right count" (three tokens, all numeric)
    /// - "right count" (two tokens, numeric right and count)
    /// Lines that do not match are skipped with a warning.
    pub fn ingest_table_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let p = path.as_ref();
        let f = File::open(p).with_context(|| format!("opening table file {}", p.display()))?;
        let rdr = BufReader::new(f);
        for (_lineno, line_res) in rdr.lines().enumerate() {
            let line = line_res?;
            let s = line.trim();
            if s.is_empty() { continue; }

            // split by whitespace (handles tabs)
            let parts: Vec<&str> = s.split_whitespace().collect();
            if parts.len() >= 2 {
                // Common format in repository files: <something> <something> <id> <count>
                // Try to parse the last two columns as (id, count).
                let last = parts.len() - 1;
                let penult = parts.len() - 2;
                if let (Ok(id), Ok(cnt)) = (parts[penult].parse::<u64>(), parts[last].parse::<u128>()) {
                    let e = self.bigram_right_freqs.entry(id).or_insert(0);
                    *e += cnt;
                    self.total_bigram_counts += cnt;
                    continue;
                }

                // Fallback: if there are exactly 3 numeric tokens treat as left right count
                if parts.len() == 3 {
                    if let (Ok(_left), Ok(right), Ok(cnt)) = (parts[0].parse::<u64>(), parts[1].parse::<u64>(), parts[2].parse::<u128>()) {
                        let e = self.bigram_right_freqs.entry(right).or_insert(0);
                        *e += cnt;
                        self.total_bigram_counts += cnt;
                        continue;
                    }
                }
            }
            // otherwise skip line silently
        }
        Ok(())
    }

    pub fn save_bincode<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let p = path.as_ref();
        let f = File::create(p).with_context(|| format!("creating db file {}", p.display()))?;
        bincode::serialize_into(f, self).with_context(|| "serializing bigram db")?;
        Ok(())
    }

    pub fn load_bincode<P: AsRef<Path>>(path: P) -> Result<Self> {
        let p = path.as_ref();
        let f = File::open(p).with_context(|| format!("opening db file {}", p.display()))?;
        let db: BigramDB = bincode::deserialize_from(f).with_context(|| "deserializing bigram db")?;
        Ok(db)
    }
}

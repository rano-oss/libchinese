use anyhow::Result;
use fst::Map;
use redb::{Database, TableDefinition};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lambdas(pub [f32; 3]);

/// Interpolator holds an fst map (key -> index) and a redb database table
/// storing bincode-serialized `Lambdas` values keyed by index.
#[derive(Debug, Clone)]
pub struct Interpolator {
    map: Option<Map<Vec<u8>>>,
    db_path: Option<String>,
    in_memory: Option<HashMap<String, Lambdas>>,
}

impl Interpolator {
    /// Load from fst + redb pair. If fst_path doesn't exist, returns an empty interpolator.
    pub fn load<P: AsRef<Path>>(fst_path: P, redb_path: P) -> Result<Self> {
        let fst_path = fst_path.as_ref();
        let redb_path = redb_path.as_ref();

        let map = if fst_path.exists() {
            let mut f = File::open(fst_path)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            // Map::new takes the underlying data container
            Some(Map::new(buf)?)
        } else {
            None
        };

        let db_path_str = if redb_path.exists() {
            Some(redb_path.to_string_lossy().to_string())
        } else {
            None
        };

        Ok(Self {
            map,
            db_path: db_path_str,
            in_memory: None,
        })
    }

    /// Create an interpolator from in-memory map (useful for tests).
    pub fn from_map(map: HashMap<String, Lambdas>) -> Self {
        Self {
            map: None,
            db_path: None,
            in_memory: Some(map),
        }
    }

    /// Lookup lambdas for a key. Returns None if not found.
    pub fn lookup(&self, key: &str) -> Option<Lambdas> {
        // check in-memory first
        if let Some(map) = &self.in_memory {
            if let Some(l) = map.get(key) {
                return Some(l.clone());
            }
        }

        let map = match &self.map {
            Some(m) => m,
            None => return None,
        };

        let idx = map.get(key)? as u64;

        // NOTE: reading payloads from redb requires a small adapter layer to
        // convert the access guard into a byte slice. To keep this initial
        // implementation focused and testable we skip attempting to read the
        // redb payload here. If a redb path is provided, future work should
        // implement reading the table and deserializing the Lambdas bytes.

        None
    }
}

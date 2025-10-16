use anyhow::Result;
use fst::Map;
use redb::TableDefinition;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use std::path::Path;
use bincode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lambdas(pub [f32; 3]);

/// Interpolator holds an fst map (key -> index) and a redb database table
/// storing bincode-serialized `Lambdas` values keyed by index.
#[derive(Debug, Clone)]
pub struct Interpolator {
    map: Map<Vec<u8>>,
    db_path: Option<String>,
    db: Option<Arc<redb::Database>>,
}

impl Interpolator {
    /// Load from fst + redb pair. If fst_path doesn't exist, returns an error.
    pub fn load<P: AsRef<Path>>(fst_path: P, redb_path: P) -> Result<Self> {
        let fst_path = fst_path.as_ref();
        let redb_path = redb_path.as_ref();

        let map = {
            let mut f = File::open(fst_path)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            Map::new(buf)?
        };

        let (db_path_str, db_opened) = if redb_path.exists() {
            let pstr = redb_path.to_string_lossy().to_string();
            match redb::Database::open(redb_path) {
                Ok(db) => (Some(pstr), Some(Arc::new(db))),
                Err(_) => (Some(pstr), None),
            }
        } else {
            (None, None)
        };

        Ok(Self { map, db_path: db_path_str, db: db_opened })
    }

    /// Load from fst data and an already-opened redb Database (avoid reopening on each lookup)
    pub fn load_with_db<P: AsRef<Path>>(fst_path: P, db: redb::Database) -> Result<Self> {
        let fst_path = fst_path.as_ref();
        let map = {
            let mut f = File::open(fst_path)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            Map::new(buf)?
        };

        Ok(Self { map, db_path: None, db: Some(Arc::new(db)) })
    }

    /// Lookup lambdas for a key. Returns None if not found.
    pub fn lookup(&self, key: &str) -> Option<Lambdas> {
        // consult fst + redb
        let idx = self.map.get(key)? as u64;

        // If we have a redb path, open it and read the "lambdas" table at the
        // numeric index. Any error along the way results in None (lookup
        // failure) to keep the public API ergonomic.
        // Use cached opened DB if available
        // db_ref is an Arc<Database> if we have a cached DB; otherwise try opening temporarily.
        let db_arc_opt: Option<Arc<redb::Database>> = if let Some(db_arc) = &self.db {
            Some(db_arc.clone())
        } else if let Some(path) = &self.db_path {
            match redb::Database::open(path) {
                Ok(d) => Some(Arc::new(d)),
                Err(e) => { eprintln!("Interpolator: failed to open redb '{}': {}", path, e); None }
            }
        } else {
            None
        };

        let db_arc = match db_arc_opt {
            Some(a) => a,
            None => return None,
        };

        let read_txn = match db_arc.begin_read() {
            Ok(t) => t,
            Err(e) => { eprintln!("Interpolator: begin_read failed: {}", e); return None; }
        };

        let k_table: TableDefinition<u64, Vec<u8>> = TableDefinition::new("lambdas");
        let table = match read_txn.open_table(k_table) {
            Ok(t) => t,
            Err(e) => { eprintln!("Interpolator: open_table failed: {}", e); return None; }
        };

        match table.get(&idx) {
            Ok(Some(val)) => {
                let bytes = val.value();
                match bincode::deserialize::<Lambdas>(&bytes) {
                    Ok(l) => Some(l),
                    Err(e) => { eprintln!("Interpolator: bincode deserialize error: {}", e); None },
                }
            }
            Ok(None) => { eprintln!("Interpolator: table.get returned None for idx={}", idx); None },
            Err(e) => { eprintln!("Interpolator: table.get error: {}", e); None },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use fst::MapBuilder;
    use redb::Database as RedbDatabase;

    #[test]
    fn disk_backed_lookup_returns_lambdas() {
        // create temp file paths
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
    let mut fst_path = std::env::temp_dir();
    fst_path.push(format!("libchinese_test_interp_{}.fst", stamp));
    let mut redb_path = std::env::temp_dir();
    redb_path.push(format!("libchinese_test_interp_{}.redb", stamp));

        // build fst mapping "k" -> 0
        let mut builder = MapBuilder::new(Vec::new()).expect("fst builder");
        builder.insert("k", 0u64).expect("insert");
        let fst_bytes = builder.into_inner().expect("into_inner");
        let mut f = File::create(&fst_path).expect("create fst");
        std::io::Write::write_all(&mut f, &fst_bytes).expect("write fst");

        // build redb with lambdas at index 0
        let db = RedbDatabase::create(&redb_path).expect("create redb");
        let k_table: TableDefinition<u64, Vec<u8>> = TableDefinition::new("lambdas");
        let w = db.begin_write().expect("begin write");
        {
            let mut table = w.open_table(k_table).expect("open table");
            let l = Lambdas([0.1, 0.9, 0.0]);
            let ser = bincode::serialize(&l).expect("serialize");
            table.insert(&0u64, &ser).expect("insert");
        }
        w.commit().expect("commit");
    // drop the write database handle so we can re-open the DB for a direct read
    drop(db);

    // verify by opening and reading back directly
    let db2 = RedbDatabase::open(&redb_path).expect("open redb");
    let rt = db2.begin_read().expect("begin read");
    let table2 = rt.open_table(k_table).expect("open table2");
    let got_direct = table2.get(&0u64).expect("get direct");
    assert!(got_direct.is_some(), "direct redb read missing");
    // drop direct read handles so Interpolator can open the DB path itself
    drop(table2);
    drop(rt);

        // drop db so Interpolator::load can open it
        drop(db2);
        let interp = Interpolator::load(&fst_path, &redb_path).expect("load");

        let got = interp.lookup("k");
        assert!(got.is_some());
        let l = got.unwrap();
        assert!((l.0[0] - 0.1).abs() < 1e-6);
        assert!((l.0[1] - 0.9).abs() < 1e-6);
    }
}

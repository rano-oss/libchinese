use anyhow::Result;
use fst::Map;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use bincode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lambdas(pub [f32; 3]);

/// Interpolator holds an fst map (key -> index) and a bincode vector
/// storing `Lambdas` values keyed by index.
#[derive(Debug, Clone)]
pub struct Interpolator {
    map: Map<Vec<u8>>,
    // in-memory bincode-backed lambdas vector (index -> Lambdas)
    lambdas: Vec<Lambdas>,
}

impl Interpolator {
    /// Create an empty interpolator with default lambdas
    pub fn new() -> Self {
        let map = Map::default();
        Self { map, lambdas: vec![Lambdas([0.33, 0.33, 0.34])] }
    }

    /// Load from fst + bincode pair.
    /// 
    /// - fst_path: lambdas.fst file mapping keys to indices
    /// - bincode_path: lambdas.bincode file containing Vec<Lambdas>
    pub fn load<P: AsRef<Path>>(fst_path: P, bincode_path: P) -> Result<Self> {
        let fst_path = fst_path.as_ref();
        let bincode_path = bincode_path.as_ref();

        let map = {
            let mut f = File::open(fst_path)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            Map::new(buf)?
        };
        
        let lambdas = {
            let mut f = File::open(bincode_path)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            bincode::deserialize(&buf)?
        };
        
        Ok(Self { map, lambdas })
    }

    /// Lookup lambdas for a key. Returns None if not found.
    pub fn lookup(&self, key: &str) -> Option<Lambdas> {
        let idx = self.map.get(key)? as usize;
        self.lambdas.get(idx).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use fst::MapBuilder;

    #[test]
    fn bincode_backed_lookup_returns_lambdas() {
        // create temp file paths
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
        let mut fst_path = std::env::temp_dir();
        fst_path.push(format!("libchinese_test_interp_{}.fst", stamp));
        let mut bincode_path = std::env::temp_dir();
        bincode_path.push(format!("libchinese_test_interp_{}.bincode", stamp));

        // build fst mapping "k" -> 0
        let mut builder = MapBuilder::new(Vec::new()).expect("fst builder");
        builder.insert("k", 0u64).expect("insert");
        let fst_bytes = builder.into_inner().expect("into_inner");
        let mut f = File::create(&fst_path).expect("create fst");
        f.write_all(&fst_bytes).expect("write fst");

        // build bincode with lambdas at index 0
        let lambdas_vec = vec![Lambdas([0.1, 0.9, 0.0])];
        let ser = bincode::serialize(&lambdas_vec).expect("serialize");
        let mut f = File::create(&bincode_path).expect("create bincode");
        f.write_all(&ser).expect("write bincode");

        // load and verify
        let interp = Interpolator::load(&fst_path, &bincode_path).expect("load");

        let got = interp.lookup("k");
        assert!(got.is_some());
        let l = got.unwrap();
        assert!((l.0[0] - 0.1).abs() < 1e-6);
        assert!((l.0[1] - 0.9).abs() < 1e-6);
        
        // cleanup
        let _ = std::fs::remove_file(&fst_path);
        let _ = std::fs::remove_file(&bincode_path);
    }
}

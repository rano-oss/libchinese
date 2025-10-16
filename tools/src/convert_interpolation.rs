use anyhow::Result;
use redb::{Database, TableDefinition};
use redb::ReadableTable;
use serde::{Serialize, Deserialize};
use std::fs::File;
use std::io::{BufReader, Read, BufRead, Write};
use std::path::PathBuf;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct Lambdas(pub [f32; 3]);

/// Parse interpolation-style text and write fst+redb mapping.
/// This supports three input shapes:
/// 1) upstream "tagged" interpolation model dumps (contain "\\data" and
///    sections like "\\1-gram" / "\\2-gram"). These are full model
///    dumps (tokens/phrases/counts) and do not contain per-key lambdas; we
///    treat them as model dumps and (for now) emit a raw-text redb fallback.
/// 2) simple per-line "key <a> <b> <c>" where each line ends with three
///    floats (the previous heuristic).
/// 3) estimator output lines like "token:<id> lambda:<float>". When used
///    together with `--id-map` (a phrase-index dump containing "\\item <id> <phrase> ...")
///    we translate ids to phrase keys and synthesize Lambdas([1-l, l, 0.0]).
pub fn run(input: &PathBuf, id_map: &Option<PathBuf>, phrase_fst: &Option<PathBuf>, phrase_redb: &Option<PathBuf>, out_fst: &PathBuf, out_redb: &PathBuf) -> Result<()> {
    let file = File::open(input)?;
    let mut reader = BufReader::new(file);
    let mut content = String::new();
    reader.read_to_string(&mut content)?;

    let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    // Detect upstream tag-based model dump: presence of "\\data" and
    // "interpolation" (anywhere nearby) indicates the import_interpolation.cpp
    // style file which contains \1-gram / \2-gram sections.
    let mut is_tagged_model = false;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        if t.starts_with("\\data") {
            // search a few following lines for the word "interpolation"
            for j in i..std::cmp::min(lines.len(), i + 8) {
                if lines[j].to_lowercase().contains("interpolation") {
                    is_tagged_model = true;
                    break;
                }
            }
            if is_tagged_model { break; }
        }
    }

    // If it's a tagged model dump (like the upstream parser expects), we
    // consider this a model dump and fall back to storing raw content in redb.
    if is_tagged_model {
        let db = Database::create(out_redb)?;
        let k_table: TableDefinition<u64, Vec<u8>> = TableDefinition::new("rows");
        let w = db.begin_write()?;
        {
            let mut table = w.open_table(k_table)?;
            let ser = bincode::serialize(&content)?;
            table.insert(&0u64, &ser)?;
        }
        w.commit()?;
        return Ok(());
    }

    // Try to detect simple per-line lambdas: lines ending with three floats.
    let mut perkey_map: HashMap<String, Lambdas> = Default::default();
    let mut perkey_detected = true;
    for raw in lines.iter() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 { perkey_detected = false; break; }
        let a = parts[parts.len()-3].parse::<f32>();
        let b = parts[parts.len()-2].parse::<f32>();
        let c = parts[parts.len()-1].parse::<f32>();
        if a.is_err() || b.is_err() || c.is_err() { perkey_detected = false; break; }
        let key = parts[0..parts.len()-3].join(" ");
        perkey_map.insert(key, Lambdas([a.unwrap(), b.unwrap(), c.unwrap()]));
    }

    // Detect estimator output: token:<id> lambda:<f>
    let mut id_map_lambda: HashMap<u64, f32> = Default::default();
    for raw in lines.iter() {
        let line = raw.trim();
        if line.is_empty() { continue; }
        if let Some(tok_idx) = line.find("token:") {
            if let Some(lidx) = line.find("lambda:") {
                let id_str = &line[tok_idx + "token:".len()..lidx].trim();
                let lambda_str = &line[lidx + "lambda:".len()..].trim();
                if let (Ok(idv), Ok(lv)) = (id_str.parse::<u64>(), lambda_str.parse::<f32>()) {
                    id_map_lambda.insert(idv, lv);
                }
            }
        }
    }

    // If we have estimator output, try to load an id->phrase dump provided
    // via --id-map. We expect lines like "\\item <id> <phrase> ..." similar
    // to upstream phrase-index dumps.
    let mut id_to_phrase: HashMap<u64, String> = Default::default();
    if !id_map_lambda.is_empty() {
        // prefer explicit id_map file if provided
        if let Some(id_map_path) = id_map {
            let f = File::open(id_map_path)?;
            let rdr = BufReader::new(f);
            for line in rdr.lines() {
                let line = line?;
                let s = line.trim();
                if s.is_empty() { continue; }
                if s.starts_with("\\item") {
                    let parts: Vec<&str> = s.split_whitespace().collect();
                    // common dump formats put the id after \item, then the phrase
                    if parts.len() >= 3 {
                        if let Ok(idv) = parts[1].parse::<u64>() {
                            let phrase = parts[2].to_string();
                            id_to_phrase.insert(idv, phrase);
                        }
                    }
                }
            }
        } else if let Some(redb_path) = phrase_redb {
            // try to open fst+redb produced by convert_table and build id->phrase map
            if redb_path.exists() {
                let db = Database::open(redb_path)?;
                let read_txn = db.begin_read()?;
                let k_table: TableDefinition<u64, Vec<u8>> = TableDefinition::new("phrases");
                if let Ok(table) = read_txn.open_table(k_table) {
                    for item in table.iter()? {
                        let (k, v) = item?;
                        let idx = k.value();
                        // local struct matching PhraseEntry in convert_table
                        #[derive(Deserialize)]
                        struct PhraseEntry { text: String, freq: u64 }
                        if let Ok(list) = bincode::deserialize::<Vec<PhraseEntry>>(&v.value()) {
                            if !list.is_empty() {
                                id_to_phrase.insert(idx, list[0].text.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    // Synthesize per-key lambdas from estimator output + id->phrase map
    if !id_map_lambda.is_empty() && !id_to_phrase.is_empty() {
        for (idv, lam) in id_map_lambda.iter() {
            if let Some(k) = id_to_phrase.get(idv) {
                perkey_map.insert(k.clone(), Lambdas([1.0 - *lam, *lam, 0.0]));
            }
        }
        perkey_detected = true;
    }

    // If we found per-key lambdas, emit fst+redb
    if perkey_detected && !perkey_map.is_empty() {
        let mut keys: Vec<String> = perkey_map.keys().cloned().collect();
        keys.sort();
        let mut builder = fst::MapBuilder::new(Vec::new())?;
        for (i, k) in keys.iter().enumerate() {
            builder.insert(k, i as u64)?;
        }
        let fst_bytes = builder.into_inner()?;
        let mut out = File::create(out_fst)?;
        out.write_all(&fst_bytes)?;

        let db = Database::create(out_redb)?;
        let k_table: TableDefinition<u64, Vec<u8>> = TableDefinition::new("lambdas");
        let w = db.begin_write()?;
        {
            let mut table = w.open_table(k_table)?;
            for (i, k) in keys.iter().enumerate() {
                let l = &perkey_map[k];
                let ser = bincode::serialize(l)?;
                table.insert(&(i as u64), &ser)?;
            }
        }
        w.commit()?;
        return Ok(());
    }

    // fallback: write raw text to redb
    let db = Database::create(out_redb)?;
    let k_table: TableDefinition<u64, Vec<u8>> = TableDefinition::new("rows");
    let w = db.begin_write()?;
    {
        let mut table = w.open_table(k_table)?;
        let ser = bincode::serialize(&content)?;
        table.insert(&0u64, &ser)?;
    }
    w.commit()?;
    Ok(())
}

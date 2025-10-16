use anyhow::Result;
use redb::{Database, TableDefinition};
use serde::Deserialize;
use std::fs::File;
use std::io::{BufReader, BufRead, Write};
use std::path::PathBuf;
use std::collections::HashMap;

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct PhraseEntry {
    pub text: String,
    pub freq: u64,
}

pub fn run(inputs: &[PathBuf], out_fst: &PathBuf, out_redb: &PathBuf) -> Result<()> {
    // We'll namespace keys per-file to avoid collisions: composite_key = "<table_name>\t<key>".
    let mut global: HashMap<String, Vec<PhraseEntry>> = Default::default();

    for input in inputs.iter() {
        let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let file = File::open(input)?;
        let mut reader = BufReader::new(file);

        // read first non-empty line for detection
        // let mut first_line = String::new();
        // reader.read_line(&mut first_line)?;
        // let sep = if first_line.contains('\t') { '\t' } else { ' ' };
        // let first_tok = first_line.split(sep).next().unwrap_or("").to_string();
        // reader.seek(std::io::SeekFrom::Start(0))?;

        let mut map: HashMap<String, Vec<PhraseEntry>> = Default::default();
        for line_res in reader.lines() {
            let line = line_res?;
            if line.trim().is_empty() { continue; }
            let parts: Vec<&str> = if line.contains('\t') { line.split('\t').collect() } else { line.split_whitespace().collect() };
            if parts.len() < 2 { continue; }
            let key = parts[0].to_string();
            let phrase = parts[1].to_string();
            let freq = parts.last().and_then(|s| s.parse::<u64>().ok()).unwrap_or(1u64);
            map.entry(key).or_default().push(PhraseEntry { text: phrase, freq });
        }

        // namespace and insert into global map
        for (k, v) in map.into_iter() {
            let composite = format!("{}\t{}", stem, k);
            global.insert(composite, v);
        }
    }

    // Now build fst from sorted composite keys and write a single redb containing all payloads
    let mut keys: Vec<String> = global.keys().cloned().collect();
    keys.sort();
    let mut builder = fst::MapBuilder::new(Vec::new())?;
    for (i, k) in keys.iter().enumerate() {
        builder.insert(k, i as u64)?;
    }
    let fst_bytes = builder.into_inner()?;
    let mut out = File::create(out_fst)?;
    out.write_all(&fst_bytes)?;

    let db = Database::create(out_redb)?;
    let k_table: TableDefinition<u64, Vec<u8>> = TableDefinition::new("phrases");
    let w = db.begin_write()?;
    {
        let mut table = w.open_table(k_table)?;
        for (i, k) in keys.iter().enumerate() {
            let list = &global[k];
            let ser = bincode::serialize(list)?;
            table.insert(&(i as u64), &ser)?;
        }
    }
    w.commit()?;

    Ok(())
}

// Useful for punct table
// for line in reader.lines() {
// let line = line?;
// if line.trim().is_empty() { continue; }
// let parts: Vec<&str> = if line.contains('\t') { line.split('\t').collect() } else { line.split_whitespace().collect() };
// if parts.len() < 2 { continue; }
// if let Ok(id) = parts[0].parse::<u64>() {
//     let rest: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
//     // store as a single PhraseEntry with the joined rest as text
//     let joined = rest.join(" ");
//     let pe = PhraseEntry { text: joined, freq: 1 };
//     let composite = format!("{}\t#{}", stem, id);
//     global.entry(composite).or_default().push(pe);
// }
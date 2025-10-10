use anyhow::Result;
use clap::Parser;
// ...existing imports...
use redb::{Database, TableDefinition};
use serde::Deserialize;
use std::fs::File;
use std::io::{BufReader, Write, BufRead};
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    /// Input JSON file mapping key -> array of { text, freq }
    #[arg(long)]
    input: PathBuf,

    /// Output fst map file path (keys -> index)
    #[arg(long, default_value = "lexicon.fst")]
    out_fst: PathBuf,

    /// Output redb file path (phrase lists)
    #[arg(long, default_value = "lexicon.redb")]
    out_redb: PathBuf,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct PhraseEntry {
    text: String,
    freq: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Auto-detect input format by extension and sample content.
    let input_ext = args
        .input
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if input_ext == "json" {
        let file = File::open(&args.input)?;
        let reader = BufReader::new(file);
        let raw: std::collections::HashMap<String, Vec<PhraseEntry>> = serde_json::from_reader(reader)?;

        // Build fst map from key -> sequential index
        let mut keys: Vec<String> = raw.keys().cloned().collect();
        keys.sort();
        // Build into an in-memory Vec<u8>
        let mut builder = fst::MapBuilder::new(Vec::new())?;
        for (i, k) in keys.iter().enumerate() {
            builder.insert(k, i as u64)?;
        }
        let fst_bytes = builder.into_inner()?;
        let mut out = File::create(&args.out_fst)?;
        out.write_all(&fst_bytes)?;

        // Build redb with table "phrases": index -> serialized Vec<PhraseEntry>
        let db = Database::create(&args.out_redb)?;
        let k_table: TableDefinition<u64, Vec<u8>> = TableDefinition::new("phrases");
        let mut w = db.begin_write()?;
        {
            let mut table = w.open_table(k_table)?;
            for (i, k) in keys.iter().enumerate() {
                let list = &raw[k];
                let ser = bincode::serialize(list)?;
                table.insert(&(i as u64), &ser)?;
            }
            // table dropped here
        }
        w.commit()?;

    } else if input_ext == "table" {
        // Parse .table files. Common layouts observed in repo:
        // - key-first: key\tphrase\tid\tfreq  (pinyin/zhuyin -> phrase)
        // - id-first: id\t... (numeric first column)

        let file = File::open(&args.input)?;
        let mut reader = BufReader::new(file);

        // read first non-empty line for detection
        let mut first_line = String::new();
        loop {
            first_line.clear();
            let bytes = reader.read_line(&mut first_line)?;
            if bytes == 0 { break; }
            if !first_line.trim().is_empty() { break; }
        }

        if first_line.trim().is_empty() {
            anyhow::bail!("empty table file");
        }

        let sep = if first_line.contains('\t') { '\t' } else { ' ' };
        let first_tok = first_line.split(sep).next().unwrap_or("").to_string();

        // Re-open reader to iterate from start
        let file = File::open(&args.input)?;
        reader = BufReader::new(file);

        if first_tok.chars().all(|c| c.is_ascii_alphabetic() || c == '\'' || c == '\u02ca' || c == '\u02cb' || c == '-') {
            // key-first: build key -> Vec<PhraseEntry>
            let mut map: std::collections::HashMap<String, Vec<PhraseEntry>> = Default::default();
            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() { continue; }
                let parts: Vec<&str> = if line.contains('\t') { line.split('\t').collect() } else { line.split_whitespace().collect() };
                if parts.len() < 2 { continue; }
                let key = parts[0].to_string();
                let phrase = parts[1].to_string();
                let freq = parts.last().and_then(|s| s.parse::<u64>().ok()).unwrap_or(1u64);
                map.entry(key).or_default().push(PhraseEntry { text: phrase, freq });
            }

            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();
            let mut builder = fst::MapBuilder::new(Vec::new())?;
            for (i, k) in keys.iter().enumerate() {
                builder.insert(k, i as u64)?;
            }
            let fst_bytes = builder.into_inner()?;
            let mut out = File::create(&args.out_fst)?;
            out.write_all(&fst_bytes)?;

            let db = Database::create(&args.out_redb)?;
            let k_table: TableDefinition<u64, Vec<u8>> = TableDefinition::new("phrases");
            let mut w = db.begin_write()?;
            {
                let mut table = w.open_table(k_table)?;
                for (i, k) in keys.iter().enumerate() {
                    let list = &map[k];
                    let ser = bincode::serialize(list)?;
                    table.insert(&(i as u64), &ser)?;
                }
            }
            w.commit()?;

        } else {
            // id-first: store rows keyed by numeric id into redb
            let db = Database::create(&args.out_redb)?;
            let k_table: TableDefinition<u64, Vec<u8>> = TableDefinition::new("rows");
            let mut w = db.begin_write()?;
            {
                let mut table = w.open_table(k_table)?;
                for line in reader.lines() {
                    let line = line?;
                    if line.trim().is_empty() { continue; }
                    let parts: Vec<&str> = if line.contains('\t') { line.split('\t').collect() } else { line.split_whitespace().collect() };
                    if parts.len() < 2 { continue; }
                    if let Ok(id) = parts[0].parse::<u64>() {
                        let rest: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
                        let ser = bincode::serialize(&rest)?;
                        table.insert(&id, &ser)?;
                    }
                }
            }
            w.commit()?;
        }

    } else {
        anyhow::bail!("unsupported input format: {}", input_ext);
    }

    println!("Wrote fst to {} and redb to {}", args.out_fst.display(), args.out_redb.display());
    Ok(())
}

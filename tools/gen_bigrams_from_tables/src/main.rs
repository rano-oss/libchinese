use anyhow::Result;
use clap::Parser;
use fst::Map;
use fst::Streamer;
use redb::{Database, TableDefinition};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write, Read};
use std::path::PathBuf;

#[derive(Parser)]
struct Opts {
    /// Path to fst file (phrases fst)
    fst: PathBuf,

    /// Path to redb file (phrases redb)
    redb: PathBuf,

    /// Output bigram text file (w1 w2\tcount)
    #[clap(short, long, default_value = "deleted_bigrams.txt")]
    out: PathBuf,
}

#[derive(serde::Deserialize)]
struct PhraseEntry {
    text: String,
    freq: u64,
}

fn main() -> Result<()> {
    let opts = Opts::parse();

    // load fst
    let mut f = File::open(&opts.fst)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let map = Map::new(buf)?;

    // open redb
    let db = Database::open(&opts.redb)?;

    let mut counts: HashMap<(String, String), u64> = HashMap::new();

    let rt = db.begin_read()?;
    let td: TableDefinition<u64, Vec<u8>> = TableDefinition::new("phrases");
    let table = rt.open_table(td)?;

    // iterate fst keys
    let mut stream = map.stream();
    while let Some((k, v)) = stream.next() {
        if let Ok(_s) = std::str::from_utf8(k) {
            // keys are likely like "<id>\t<key>" depending on producer; we only need the value id `v`
            let id = v as u64;
            if let Ok(Some(val)) = table.get(&id) {
                let bytes = val.value();
                if let Ok(list) = bincode::deserialize::<Vec<PhraseEntry>>(&bytes) {
                    for pe in list.into_iter() {
                        // naive tokenization: characters as tokens
                        let chars: Vec<String> = pe.text.chars().map(|c| c.to_string()).collect();
                        for i in 0..chars.len().saturating_sub(1) {
                            let left = chars[i].clone();
                            let right = chars[i + 1].clone();
                            *counts.entry((left, right)).or_default() += 1;
                        }
                    }
                }
            }
        }
    }

    // write counts
    let of = File::create(&opts.out)?;
    let mut w = BufWriter::new(of);
    for ((w1, w2), cnt) in counts.iter() {
        writeln!(w, "{} {}\t{}", w1, w2, cnt)?;
    }

    println!("wrote {} bigrams to {}", counts.len(), opts.out.display());
    Ok(())
}

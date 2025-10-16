use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use convert_tables::bigram_db::BigramDB;

#[derive(Parser)]
struct Args {
    /// Input table files (glob or list). If empty, default locations will be used.
    #[arg(long)]
    files: Option<Vec<PathBuf>>,

    /// Output db file (bincode)
    #[arg(long, default_value = "data/bigram_db.bin")]
    out: PathBuf,

    /// Also write JSON for inspection
    #[arg(long, default_value_t = true)]
    json: bool,
}

fn default_table_paths() -> Vec<PathBuf> {
    let mut v = Vec::new();
    v.push(PathBuf::from("data/merged.table"));
    v.push(PathBuf::from("data/opengram.table"));
    v.push(PathBuf::from("data/gb_char.table"));
    v.push(PathBuf::from("data/gbk_char.table"));
    v.push(PathBuf::from("data/tsi.table"));
    v.push(PathBuf::from("data/zhuyin/tsi.table"));
    v.push(PathBuf::from("data/addon/addon.table"));
    v
}

fn main() -> Result<()> {
    let args = Args::parse();

    let files = if let Some(f) = args.files { f } else { default_table_paths() };

    let mut db = BigramDB::new();
    for p in files.iter() {
        if p.exists() {
            println!("Ingesting {}", p.display());
            db.ingest_table_file(p)?;
        } else {
            println!("Skipping missing {}", p.display());
        }
    }

    db.save_bincode(&args.out)?;
    println!("Wrote {} (total_bigram_counts={})", args.out.display(), db.total_bigram_counts);
    if args.json {
        let j = serde_json::to_string_pretty(&db)?;
        let mut outj = args.out.clone();
        outj.set_extension("json");
        std::fs::write(&outj, j)?;
        println!("Wrote {}", outj.display());
    }

    Ok(())
}

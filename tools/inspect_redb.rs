use redb::Database;
use std::path::Path;

fn main() -> redb::Result<()> {
    let path = Path::new("data/pinyin.redb");
    let db = Database::open(path)?;
    println!("Opened redb: {:?}", path);
    let mut rt = db.begin_read()?;

    // We don't know table names up front; redb's public API doesn't list tables
    // directly, but we can try common names used by the project.
    let candidate_names = ["phrases", "lexicon", "entries", "words", "lambdas"];
    for tn in &candidate_names {
        let td = redb::TableDefinition::<u64, Vec<u8>>::new(tn);
        match rt.open_table(td) {
            Ok(table) => {
                println!("Found table '{}' (u64->Vec<u8>)", tn);
                let mut iter = table.iter()?;
                for (i, kv) in iter.take(5).enumerate() {
                    if let Some(kv) = kv? {
                        println!("  [{}] key={:?} val_len={}", i, kv.key(), kv.value().len());
                    }
                }
            }
            Err(e) => {
                println!("Table '{}' not present or different type: {}", tn, e);
            }
        }
    }

    // also try string-keyed tables
    for tn in &candidate_names {
        let td = redb::TableDefinition::<&str, Vec<u8>>::new(tn);
        match rt.open_table(td) {
            Ok(table) => {
                println!("Found table '{}' (&str->Vec<u8>)", tn);
                let mut iter = table.iter()?;
                for (i, kv) in iter.take(5).enumerate() {
                    if let Some(kv) = kv? {
                        println!("  [{}] key={:?} val_len={}", i, kv.key(), kv.value().len());
                    }
                }
            }
            Err(e) => {
                println!("Table '{}' not present or different type (str): {}", tn, e);
            }
        }
    }

    Ok(())
}

use redb::{Database, ReadableTable};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new("data/pinyin.redb");
    let db = Database::open(path)?;
    println!("Opened redb: {:?}", path);
    let rt = db.begin_read()?;

    // Try a few likely table definitions and also attempt to detect tables by name
    let candidates = ["phrases", "lexicon", "entries", "words", "lambdas", "pinyin"];
    for tn in &candidates {
        // try u64 -> Vec<u8>
        let td1 = redb::TableDefinition::<u64, Vec<u8>>::new(tn);
        match rt.open_table(td1) {
            Ok(table) => {
                println!("Found table '{}' (u64->Vec<u8>)", tn);
                let mut iter = table.iter()?;
                for (i, item_res) in iter.take(5).enumerate() {
                    let (k, v) = item_res?;
                    println!("  [{}] key={:?} val_len={}", i, k.value(), v.value().len());
                }
            }
            Err(e) => println!("Table '{}' not present as u64->Vec<u8>: {}", tn, e),
        }

        // try &str -> Vec<u8>
        let td2 = redb::TableDefinition::<&str, Vec<u8>>::new(tn);
        match rt.open_table(td2) {
            Ok(table) => {
                println!("Found table '{}' (&str->Vec<u8>)", tn);
                let mut iter = table.iter()?;
                for (i, item_res) in iter.take(5).enumerate() {
                    let (k, v) = item_res?;
                    println!("  [{}] key={:?} val_len={}", i, k.value(), v.value().len());
                }
            }
            Err(e) => println!("Table '{}' not present as &str->Vec<u8>: {}", tn, e),
        }
    }

    Ok(())
}

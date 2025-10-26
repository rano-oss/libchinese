use fst::Map;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LexEntry {
    utf8: String,
    token: u32,
    freq: u32,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: {} <fst_path> <bincode_path> <key>", args[0]);
        std::process::exit(1);
    }

    let fst_path = &args[1];
    let bincode_path = &args[2];
    let search_key = &args[3];

    // Read FST
    let fst_data = std::fs::read(fst_path).expect("Failed to read FST");
    let fst_map = Map::new(fst_data).expect("Failed to parse FST");

    // Read bincode
    let mut file = File::open(bincode_path).expect("Failed to open bincode");
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).expect("Failed to read bincode");
    let entries: Vec<Vec<LexEntry>> =
        bincode::deserialize(&buffer).expect("Failed to deserialize bincode");

    // Lookup key
    if let Some(idx) = fst_map.get(search_key) {
        println!("Key '{}' found at index {}", search_key, idx);
        let idx = idx as usize;
        if idx < entries.len() {
            let group = &entries[idx];
            println!("Found {} entries:", group.len());
            for (i, entry) in group.iter().enumerate() {
                println!("  {}. {} (token={}, freq={})", i+1, entry.utf8, entry.token, entry.freq);
            }
        } else {
            println!("Error: Index {} out of range (total groups: {})", idx, entries.len());
        }
    } else {
        println!("Key '{}' not found in FST", search_key);
    }
}

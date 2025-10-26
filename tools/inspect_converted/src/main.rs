use fst::{Map, Streamer};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LexEntry {
    utf8: String,
    token: u32,
    freq: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Lambdas(pub [f32; 3]);

fn inspect_dataset(dataset_path: &str, search_key: Option<&str>) {
    println!("\n=== Inspecting {} ===", dataset_path);

    // Check lexicon FST
    let lexicon_fst_path = format!("{}/lexicon.fst", dataset_path);
    if Path::new(&lexicon_fst_path).exists() {
        let fst_data = std::fs::read(&lexicon_fst_path).expect("Failed to read lexicon FST");
        let fst_map = Map::new(fst_data).expect("Failed to parse lexicon FST");

        println!("Lexicon FST:");
        println!("  - Keys count: {}", fst_map.len());
        
        // If search_key is provided, look for it specifically
        if let Some(key) = search_key {
            println!("  - Searching for key: '{}'", key);
            if let Some(val) = fst_map.get(key) {
                println!("    ✓ FOUND: {} -> {}", key, val);
            } else {
                println!("    ✗ NOT FOUND: {}", key);
                // Search for similar keys
                println!("  - Similar keys:");
                let mut stream = fst_map.stream();
                let mut count = 0;
                while let Some((k, v)) = stream.next() {
                    let k_str = String::from_utf8_lossy(k);
                    if k_str.contains(key) || key.contains(&k_str.as_ref()) {
                        println!("    {} -> {}", k_str, v);
                        count += 1;
                        if count >= 10 {
                            break;
                        }
                    }
                }
            }
        } else {
            println!("  - Sample keys (first 10):");
            let mut stream = fst_map.stream();
            let mut count = 0;
            while let Some((key, val)) = stream.next() {
                println!("    {} -> {}", String::from_utf8_lossy(key), val);
                count += 1;
                if count >= 10 {
                    break;
                }
            }
        }
    } else {
        println!("❌ lexicon.fst not found");
    }

    // Check lexicon bincode
    let lexicon_bincode_path = format!("{}/lexicon.bincode", dataset_path);
    if Path::new(&lexicon_bincode_path).exists() {
        let mut file = File::open(&lexicon_bincode_path).expect("Failed to open lexicon bincode");
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .expect("Failed to read lexicon bincode");

        let entries: Vec<Vec<LexEntry>> =
            bincode::deserialize(&buffer).expect("Failed to deserialize lexicon bincode");

        println!("\nLexicon Bincode:");
        println!("  - Entry groups: {}", entries.len());
        println!(
            "  - Total entries: {}",
            entries.iter().map(|v| v.len()).sum::<usize>()
        );

        // Sample first few entries
        println!("  - Sample entries (first 5 groups):");
        for (i, group) in entries.iter().enumerate() {
            if i >= 5 {
                break;
            }
            println!("    Group {}: {} entries", i, group.len());
            for (j, entry) in group.iter().enumerate() {
                if j >= 2 {
                    break;
                }
                println!(
                    "      - {}: token={}, freq={}",
                    entry.utf8, entry.token, entry.freq
                );
            }
        }
    } else {
        println!("❌ lexicon.bincode not found");
    }

    // Check lambdas FST
    let lambdas_fst_path = format!("{}/lambdas.fst", dataset_path);
    if Path::new(&lambdas_fst_path).exists() {
        let fst_data = std::fs::read(&lambdas_fst_path).expect("Failed to read lambdas FST");
        let fst_map = Map::new(fst_data).expect("Failed to parse lambdas FST");

        println!("\nLambdas FST:");
        println!("  - Keys count: {}", fst_map.len());
        println!("  - Sample keys (first 5):");
        let mut stream = fst_map.stream();
        let mut count = 0;
        while let Some((key, val)) = stream.next() {
            println!("    {} -> {}", String::from_utf8_lossy(key), val);
            count += 1;
            if count >= 5 {
                break;
            }
        }
    } else {
        println!("❌ lambdas.fst not found");
    }

    // Check lambdas bincode
    let lambdas_bincode_path = format!("{}/lambdas.bincode", dataset_path);
    if Path::new(&lambdas_bincode_path).exists() {
        let mut file = File::open(&lambdas_bincode_path).expect("Failed to open lambdas bincode");
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .expect("Failed to read lambdas bincode");

        let lambdas: Vec<Lambdas> =
            bincode::deserialize(&buffer).expect("Failed to deserialize lambdas");

        println!("\nLambdas Bincode:");
        println!("  - Lambda count: {}", lambdas.len());
        println!("  - Sample lambdas (first 5):");
        for (i, lambda) in lambdas.iter().enumerate() {
            if i >= 5 {
                break;
            }
            println!(
                "    {}: [{:.4}, {:.4}, {:.4}]",
                i, lambda.0[0], lambda.0[1], lambda.0[2]
            );
        }

        // Validate lambda values are reasonable (sum to ~1, non-negative)
        let mut warnings = 0;
        for (i, lambda) in lambdas.iter().enumerate() {
            let sum = lambda.0[0] + lambda.0[1] + lambda.0[2];
            if (sum - 1.0).abs() > 0.01 || lambda.0.iter().any(|&v| !(0.0..=1.0).contains(&v)) {
                if warnings < 3 {
                    println!(
                        "    ⚠️  Lambda {} has unusual values: [{:.4}, {:.4}, {:.4}] (sum={:.4})",
                        i, lambda.0[0], lambda.0[1], lambda.0[2], sum
                    );
                }
                warnings += 1;
            }
        }
        if warnings > 0 {
            println!(
                "  ⚠️  {} lambdas have unusual values (sum != 1.0 or out of range)",
                warnings
            );
        }
    } else {
        println!("❌ lambdas.bincode not found");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    let (datasets, search_key) = if args.len() >= 2 {
        // If path provided, inspect just that one
        if args[1].starts_with("data/") {
            let search = if args.len() >= 3 { Some(args[2].as_str()) } else { None };
            (vec![args[1].clone()], search)
        } else {
            // First arg is search key
            let datasets = vec![
                "data/converted/simplified".to_string(),
                "data/converted/traditional".to_string(),
                "data/converted/zhuyin_traditional".to_string(),
            ];
            (datasets, Some(args[1].as_str()))
        }
    } else {
        // Default: inspect all
        (vec![
            "data/converted/simplified".to_string(),
            "data/converted/traditional".to_string(),
            "data/converted/zhuyin_traditional".to_string(),
        ], None)
    };

    for dataset in datasets {
        inspect_dataset(&dataset, search_key);
    }

    println!("\n=== Summary ===");
    println!("✓ All datasets inspected. Check for ❌ and ⚠️  markers above for issues.");
}

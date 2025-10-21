//! Import custom phrases into user dictionary
//!
//! Supports multiple input formats:
//! - JSON: [["phrase", frequency], ...]
//! - CSV: phrase,frequency header with data rows
//! - TXT: one phrase per line (frequency defaults to 1)
//!
//! Usage:
//!   cargo run -p import_phrases -- --db data/userdict.redb --input phrases.json
//!   cargo run -p import_phrases -- --db data/userdict.redb --input phrases.csv --format csv
//!   cargo run -p import_phrases -- --db data/userdict.redb --input phrases.txt --format txt

use clap::Parser;
use libchinese_core::UserDict;
use std::path::PathBuf;

#[derive(clap::Parser, Debug)]
#[command(name = "import_phrases")]
#[command(about = "Import custom phrases into user dictionary")]
struct Args {
    /// Path to the user dictionary database
    #[arg(short, long)]
    db: PathBuf,

    /// Input file containing phrases
    #[arg(short, long)]
    input: PathBuf,

    /// Input format: json, csv, or txt
    #[arg(short, long, default_value = "json")]
    format: String,

    /// Merge mode: add (default) or replace
    #[arg(short, long, default_value = "add")]
    mode: String,

    /// Dry run (show what would be imported without actually importing)
    #[arg(long)]
    dry_run: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Read input file
    let content = std::fs::read_to_string(&args.input)
        .map_err(|e| anyhow::anyhow!("Failed to read input file: {}", e))?;

    // Parse based on format
    let entries = match args.format.as_str() {
        "json" => parse_json(&content)?,
        "csv" => parse_csv(&content)?,
        "txt" => parse_txt(&content)?,
        _ => anyhow::bail!("Unsupported format: {}. Use 'json', 'csv', or 'txt'", args.format),
    };

    println!("Parsed {} phrases from {}", entries.len(), args.input.display());

    if args.dry_run {
        println!("\n[DRY RUN] Would import:");
        for (phrase, freq) in entries.iter().take(10) {
            println!("  {} (frequency: {})", phrase, freq);
        }
        if entries.len() > 10 {
            println!("  ... and {} more", entries.len() - 10);
        }
        return Ok(());
    }

    // Load user dictionary
    let userdict = UserDict::new(&args.db)
        .map_err(|e| anyhow::anyhow!("Failed to open user dict: {}", e))?;

    // Import entries
    match args.mode.as_str() {
        "add" => {
            println!("Importing in ADD mode (increments existing frequencies)...");
            for (phrase, freq) in entries {
                userdict.learn_with_count(&phrase, freq)
                    .map_err(|e| anyhow::anyhow!("Failed to import '{}': {}", phrase, e))?;
            }
        }
        "replace" => {
            println!("Importing in REPLACE mode (sets exact frequencies)...");
            println!("WARNING: This is not yet implemented. Use 'add' mode instead.");
            anyhow::bail!("Replace mode not implemented yet");
        }
        _ => anyhow::bail!("Unsupported mode: {}. Use 'add' or 'replace'", args.mode),
    }

    println!("✓ Import complete!");
    Ok(())
}

fn parse_json(content: &str) -> anyhow::Result<Vec<(String, u64)>> {
    let entries: Vec<(String, u64)> = serde_json::from_str(content)
        .map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e))?;
    Ok(entries)
}

fn parse_csv(content: &str) -> anyhow::Result<Vec<(String, u64)>> {
    let mut entries = Vec::new();
    let mut lines = content.lines();

    // Skip header if it looks like a header
    if let Some(first_line) = lines.next() {
        if !first_line.to_lowercase().contains("phrase") {
            // Not a header, parse it
            if let Some((phrase, freq)) = parse_csv_line(first_line) {
                entries.push((phrase, freq));
            }
        }
    }

    // Parse remaining lines
    for line in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((phrase, freq)) = parse_csv_line(line) {
            entries.push((phrase, freq));
        }
    }

    Ok(entries)
}

fn parse_csv_line(line: &str) -> Option<(String, u64)> {
    // Simple CSV parser (handles quoted fields)
    let parts: Vec<String> = if line.contains('"') {
        // Handle quoted fields
        let mut parts = Vec::new();
        let mut in_quotes = false;
        let mut current = String::new();

        for ch in line.chars() {
            match ch {
                '"' => in_quotes = !in_quotes,
                ',' if !in_quotes => {
                    parts.push(current.trim().to_string());
                    current.clear();
                }
                _ => current.push(ch),
            }
        }
        parts.push(current.trim().to_string());
        parts
    } else {
        line.split(',').map(|s| s.to_string()).collect()
    };

    if parts.len() >= 2 {
        let phrase = parts[0].trim().trim_matches('"').to_string();
        let freq = parts[1].trim().parse::<u64>().ok()?;
        Some((phrase, freq))
    } else {
        None
    }
}

fn parse_txt(content: &str) -> anyhow::Result<Vec<(String, u64)>> {
    let entries: Vec<(String, u64)> = content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                None
            } else {
                Some((line.to_string(), 1)) // Default frequency of 1
            }
        })
        .collect();
    Ok(entries)
}

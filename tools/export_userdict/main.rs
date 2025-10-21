//! Export user dictionary to JSON or CSV format
//!
//! Usage:
//!   cargo run -p export_userdict -- --db data/userdict.redb --format json
//!   cargo run -p export_userdict -- --db data/userdict.redb --format csv --output phrases.csv

use clap::Parser;
use libchinese_core::UserDict;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "export_userdict")]
#[command(about = "Export user dictionary to JSON or CSV format")]
struct Args {
    /// Path to the user dictionary database
    #[arg(short, long)]
    db: PathBuf,

    /// Output format: json or csv
    #[arg(short, long, default_value = "json")]
    format: String,

    /// Output file (defaults to stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Sort by frequency (descending)
    #[arg(long)]
    sort_by_freq: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Load user dictionary
    let userdict = UserDict::new(&args.db)
        .map_err(|e| anyhow::anyhow!("Failed to open user dict: {}", e))?;

    // Get all entries
    let mut entries = userdict.iter_all();

    // Sort if requested
    if args.sort_by_freq {
        entries.sort_by(|a, b| b.1.cmp(&a.1)); // Descending by frequency
    }

    // Export based on format
    let output = match args.format.as_str() {
        "json" => export_json(&entries)?,
        "csv" => export_csv(&entries)?,
        _ => anyhow::bail!("Unsupported format: {}. Use 'json' or 'csv'", args.format),
    };

    // Write to file or stdout
    if let Some(path) = args.output {
        std::fs::write(path, output)?;
    } else {
        print!("{}", output);
    }

    Ok(())
}

fn export_json(entries: &[(String, u64)]) -> anyhow::Result<String> {
    let json = serde_json::to_string_pretty(entries)?;
    Ok(json)
}

fn export_csv(entries: &[(String, u64)]) -> anyhow::Result<String> {
    let mut output = String::from("phrase,frequency\n");
    for (phrase, freq) in entries {
        // Escape quotes in phrases
        let escaped = phrase.replace('"', "\"\"");
        output.push_str(&format!("\"{}\",{}\n", escaped, freq));
    }
    Ok(output)
}

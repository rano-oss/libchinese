use anyhow::Result;
use clap::Parser;
use fst::MapBuilder;
use redb::{Database, TableDefinition};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use libchinese_core::interpolation::Lambdas;

/// Simple utility: read deleted bigram text dump and compute per-left-token
/// interpolation lambda using the fixed-point iteration from upstream.
#[derive(Parser)]
struct Opts {
    /// Path to deleted bigram text file (format: "w1 w2\t<count>" per line)
    input: PathBuf,

    /// Output directory for fst+redb (default: data)
    #[clap(short, long, default_value = "data")]
    out_dir: PathBuf,
}

fn parse_line(line: &str) -> Option<((String, String), u64)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    if let Some(pos) = line.rfind('\t') {
        let part = &line[..pos];
        let cnt_s = &line[pos + 1..];
        if let Ok(cnt) = cnt_s.trim().parse::<u64>() {
            let mut parts: Vec<&str> = part.split_whitespace().collect();
            if parts.len() == 2 {
                return Some(((parts[0].to_string(), parts[1].to_string()), cnt));
            }
        }
    }
    // fallback to last whitespace-separated token as count
    let mut parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 3 {
        if let Ok(cnt) = parts.pop().unwrap().parse::<u64>() {
            let w2 = parts.pop().unwrap().to_string();
            let w1 = parts.join(" ");
            return Some(((w1, w2), cnt));
        }
    }
    None
}

fn compute_lambda_for_prefix(
    deleted_pairs: &HashMap<(String, String), u64>,
    prefix: &str,
    unigram_counts: &HashMap<String, u64>,
    bigram_counts: &HashMap<(String, String), u64>,
) -> f32 {
    // collect deleted counts for this prefix
    let mut total_deleted: f32 = 0.0;
    for ((w1, _w2), &c) in deleted_pairs.iter() {
        if w1 == prefix {
            total_deleted += c as f32;
        }
    }
    if total_deleted <= 0.0 {
        return 0.0;
    }

    let total_unigram: f32 = unigram_counts.values().map(|&v| v as f32).sum();
    let total_bigram: f32 = bigram_counts.values().map(|&v| v as f32).sum();

    let mut lambda: f32 = 0.6;
    let mut next_lambda: f32 = lambda;
    let epsilon: f32 = 0.001; // upstream uses 0.001

    for _ in 0..1000 {
        lambda = next_lambda;
        let mut accum: f32 = 0.0;

        for ((w1, w2), &deleted_count) in deleted_pairs.iter() {
            if w1 != prefix {
                continue;
            }
            let bigram_count = *bigram_counts.get(&(w1.clone(), w2.clone())).unwrap_or(&0) as f32;
            let elem_bigram = if total_bigram > 0.0 { bigram_count / total_bigram } else { 0.0 };
            let unigram_count = *unigram_counts.get(w2).unwrap_or(&0) as f32;
            let elem_unigram = if total_unigram > 0.0 { unigram_count / total_unigram } else { 0.0 };

            let numerator = lambda * elem_bigram;
            let denom_part = (1.0 - lambda) * elem_unigram;
            let denom = numerator + denom_part;
            if denom <= 0.0 {
                continue;
            }
            accum += (deleted_count as f32) * (numerator / denom);
        }

        if total_deleted > 0.0 {
            next_lambda = accum / total_deleted;
        }
        if (next_lambda - lambda).abs() < epsilon {
            break;
        }
    }

    if next_lambda.is_nan() { 0.0 } else { next_lambda }
}

fn main() -> Result<()> {
    let opts = Opts::parse();

    // read input
    let f = File::open(&opts.input)?;
    let reader = BufReader::new(f);

    let mut deleted_pairs: HashMap<(String, String), u64> = HashMap::new();
    let mut unigram_counts: HashMap<String, u64> = HashMap::new();
    let mut bigram_counts: HashMap<(String, String), u64> = HashMap::new();

    for line in reader.lines() {
        let l = line?;
        if let Some(((w1, w2), cnt)) = parse_line(&l) {
            *deleted_pairs.entry((w1.clone(), w2.clone())).or_default() += cnt;
            *bigram_counts.entry((w1.clone(), w2.clone())).or_default() += cnt;
            *unigram_counts.entry(w2.clone()).or_default() += cnt;
        }
    }

    // compute unique prefixes list (left tokens)
    let mut prefixes: Vec<String> = {
        let mut s = HashSet::new();
        for ((w1, _w2), _c) in deleted_pairs.iter() {
            s.insert(w1.clone());
        }
        let mut v: Vec<_> = s.into_iter().collect();
        v.sort();
        v
    };

    // prepare outputs
    std::fs::create_dir_all(&opts.out_dir)?;
    let fst_path = opts.out_dir.join("pinyin.lambdas.fst");
    let redb_path = opts.out_dir.join("pinyin.lambdas.redb");

    // build fst
    let mut map_builder = MapBuilder::new(Vec::new())?;
    for (i, key) in prefixes.iter().enumerate() {
        map_builder.insert(key, i as u64)?;
    }
    let fst_bytes = map_builder.into_inner()?;
    std::fs::write(&fst_path, &fst_bytes)?;

    // build redb and write lambdas table
    let db = Database::create(&redb_path)?;
    let table_def: TableDefinition<u64, Vec<u8>> = TableDefinition::new("lambdas");
    let wtxn = db.begin_write()?;
    {
        let mut table = wtxn.open_table(table_def)?;
        for (i, key) in prefixes.iter().enumerate() {
            let lam = compute_lambda_for_prefix(&deleted_pairs, key, &unigram_counts, &bigram_counts);
            // store as Lambdas([1-l, l, 0.0]) using f32 array (existing Interpolator expects f32)
            let l = Lambdas([1.0_f32 - lam, lam, 0.0_f32]);
            let ser = bincode::serialize(&l)?;
            table.insert(&(i as u64), &ser)?;
        }
    }
    wtxn.commit()?;

    println!("wrote {} + {}", fst_path.display(), redb_path.display());
    Ok(())
}

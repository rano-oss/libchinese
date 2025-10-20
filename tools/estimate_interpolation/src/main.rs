use anyhow::Result;
use fst::MapBuilder;
use redb::{Database, TableDefinition};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use libchinese_core::interpolation::Lambdas;

#[derive(serde::Deserialize)]
struct LexEntry {
    utf8: String,
    token: u32,
    freq: u32,
}

// Helper to compute lambdas for a dataset given in-memory payloads
fn compute_for_dataset<P: AsRef<Path>>(dataset_dir: P, payloads: Vec<Vec<(String, u32, u32)>>) -> Result<()> {
    let dataset_dir = dataset_dir.as_ref();

    // Build unigram and bigram counts from payloads
    let mut unigram_counts: HashMap<String, u64> = HashMap::new();
    let mut bigram_counts: HashMap<(String, String), u64> = HashMap::new();

    for list in payloads.iter() {
        for pe in list.iter() {
            let text = &pe.0;
            let chars: Vec<String> = text.chars().map(|c| c.to_string()).collect();
            for i in 0..chars.len().saturating_sub(1) {
                let left = chars[i].clone();
                let right = chars[i + 1].clone();
                *bigram_counts.entry((left.clone(), right.clone())).or_default() += 1;
                *unigram_counts.entry(right).or_default() += 1;
            }
        }
    }

    // compute deleted pairs: for this simple approach, reuse bigram_counts as deleted_pairs
    let deleted_pairs = bigram_counts.clone();

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

    // prepare outputs: write fst and a bincode lambdas vector (no redb)
    std::fs::create_dir_all(&dataset_dir)?;
    let fst_path = dataset_dir.join("lambdas.fst");
    let bincode_path = dataset_dir.join("lambdas.bincode");

    // build fst
    let mut map_builder = MapBuilder::new(Vec::new())?;
    for (i, key) in prefixes.iter().enumerate() {
        map_builder.insert(key, i as u64)?;
    }
    let fst_bytes = map_builder.into_inner()?;
    std::fs::write(&fst_path, &fst_bytes)?;

    // Prepare a bincode vector for all lambdas so consumers can load directly.
    let mut b_vec: Vec<Lambdas> = Vec::with_capacity(prefixes.len());
    for key in prefixes.iter() {
        let lam = compute_lambda_for_prefix(&deleted_pairs, key, &unigram_counts, &bigram_counts);
        let l = Lambdas([1.0_f32 - lam, lam, 0.0_f32]);
        b_vec.push(l);
    }
    let bbytes = bincode::serialize(&b_vec)?;
    std::fs::write(&bincode_path, &bbytes)?;

    println!("wrote {} + {}", fst_path.display(), bincode_path.display());
    Ok(())
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
    // Hardcoded converted dataset directories
    let converted = Path::new("data/converted");
    let cases = ["simplified", "traditional", "zhuyin_traditional"];

    for c in cases.iter() {
        let dir = converted.join(c);
        // read lexicon.bincode (Vec<Vec<LexEntry-like>>) format produced by convert_table
        let bin_path = dir.join("lexicon.bincode");
        if !bin_path.exists() {
            eprintln!("warning: {} missing, skipping", bin_path.display());
            continue;
        }
        let mut bf = File::open(&bin_path)?;
        let mut buf = Vec::new();
        bf.read_to_end(&mut buf)?;
        // payloads: Vec<Vec<LexEntry>> where LexEntry has utf8, token, freq
        let raw: Vec<Vec<LexEntry>> = bincode::deserialize(&buf)?;
        // Normalize into Vec<Vec<(text, token, freq)>> for compute_for_dataset
        let mut payloads: Vec<Vec<(String, u32, u32)>> = Vec::with_capacity(raw.len());
        for list in raw.into_iter() {
            let mut v: Vec<(String, u32, u32)> = Vec::with_capacity(list.len());
            for e in list.into_iter() {
                v.push((e.utf8, e.token, e.freq));
            }
            payloads.push(v);
        }

        compute_for_dataset(&dir, payloads)?;
    }

    Ok(())
}

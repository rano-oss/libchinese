use anyhow::{Context, Result};
use libchinese_core::NGramModel;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::collections::HashMap;

/// Simple parser for ngram text files.
/// Accepts lines like:
/// <token1> <token2> <token3>\t<count>
/// or
/// <token1> <token2> <token3> <count>
fn parse_line(line: &str) -> Option<(Vec<String>, u64)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    // try split by tab first
    if let Some(pos) = line.rfind('\t') {
        let part = &line[..pos];
        let cnt_s = &line[pos + 1..];
        if let Ok(cnt) = cnt_s.trim().parse::<u64>() {
            let tokens: Vec<String> = part
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            return Some((tokens, cnt));
        }
    }
    // fallback: last whitespace separated token is count
    let mut parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    if let Ok(cnt) = parts.pop().unwrap().parse::<u64>() {
        let tokens = parts.into_iter().map(|s| s.to_string()).collect();
        return Some((tokens, cnt));
    }
    None
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: serialize_ngram <ngram-text-file> [more files...]\nOutputs to data/ngram.bincode");
        std::process::exit(1);
    }

    // Accumulate raw counts first
    let mut unigram_counts: HashMap<String, u64> = HashMap::new();
    let mut bigram_counts: HashMap<(String, String), u64> = HashMap::new();
    let mut trigram_counts: HashMap<(String, String, String), u64> = HashMap::new();

    for path in &args[1..] {
        let f = File::open(path).with_context(|| format!("open {}", path))?;
        let reader = BufReader::new(f);
        for line in reader.lines() {
            let l = line?;
            if let Some((tokens, cnt)) = parse_line(&l) {
                match tokens.len() {
                    1 => {
                        let w = tokens[0].clone();
                        *unigram_counts.entry(w).or_default() += cnt;
                    }
                    2 => {
                        let w1 = tokens[0].clone();
                        let w2 = tokens[1].clone();
                        *bigram_counts.entry((w1, w2)).or_default() += cnt;
                    }
                    3 => {
                        let w1 = tokens[0].clone();
                        let w2 = tokens[1].clone();
                        let w3 = tokens[2].clone();
                        *trigram_counts.entry((w1, w2, w3)).or_default() += cnt;
                    }
                    _ => {
                        // ignore higher-order ngrams
                    }
                }
            }
        }
    }

    // Build model using Modified Kneser-Ney smoothing with multiple discounts
    // Estimate D1, D2, D3+ using Chen & Goodman (1999) formula using counts-of-counts

    // Precompute continuation counts:
    // For unigram continuation: number of unique left contexts for each word w (i.e., N1+(•, w))
    use std::collections::HashSet;
    let mut uniq_left_for_w: HashMap<String, usize> = HashMap::new();
    let mut uniq_right_for_w1: HashMap<String, usize> = HashMap::new();
    let mut uniq_right_for_bigram: HashMap<(String, String), usize> = HashMap::new();

    // For bigrams: count unique left contexts per w2
    for ((w1, w2), _) in &bigram_counts {
        uniq_left_for_w.entry(w2.clone()).or_insert(0);
    }
    // Build sets to compute unique counts
    let mut left_sets: HashMap<String, HashSet<String>> = HashMap::new();
    let mut right_sets_by_left: HashMap<String, HashSet<String>> = HashMap::new();
    let mut right_sets_by_bigram: HashMap<(String, String), HashSet<String>> = HashMap::new();

    for ((w1, w2), _) in &bigram_counts {
        left_sets.entry(w2.clone()).or_default().insert(w1.clone());
        right_sets_by_left.entry(w1.clone()).or_default().insert(w2.clone());
    }
    for ((w1, w2, w3), _) in &trigram_counts {
        right_sets_by_bigram
            .entry((w1.clone(), w2.clone()))
            .or_default()
            .insert(w3.clone());
        // also ensure bigram right set is aware of this trigram
        right_sets_by_left.entry(w2.clone()).or_default().insert(w3.clone());
        left_sets.entry(w3.clone()).or_default().insert(w2.clone());
    }

    for (w, s) in left_sets.into_iter() {
        uniq_left_for_w.insert(w, s.len());
    }
    for (w1, s) in right_sets_by_left.into_iter() {
        uniq_right_for_w1.insert(w1, s.len());
    }
    for (bg, s) in right_sets_by_bigram.into_iter() {
        uniq_right_for_bigram.insert(bg, s.len());
    }

    let total_bigram_types = bigram_counts.len() as f64;

    // counts-of-counts for bigrams and trigrams
    let mut bc_of_c: HashMap<u64, usize> = HashMap::new();
    for &cnt in bigram_counts.values() {
        *bc_of_c.entry(cnt).or_default() += 1;
    }
    let mut tc_of_c: HashMap<u64, usize> = HashMap::new();
    for &cnt in trigram_counts.values() {
        *tc_of_c.entry(cnt).or_default() += 1;
    }

    let get_discount = |cc: &HashMap<u64, usize>| -> (f64, f64, f64) {
        let n1 = *cc.get(&1).unwrap_or(&0) as f64;
        let n2 = *cc.get(&2).unwrap_or(&0) as f64;
        let n3 = *cc.get(&3).unwrap_or(&0) as f64;
        let n4 = *cc.get(&4).unwrap_or(&0) as f64;
        if n1 == 0.0 || n2 == 0.0 {
            // fallback to defaults
            return (0.75f64, 0.75f64, 0.75f64);
        }
        let y = n1 / (n1 + 2.0 * n2);
        let d1 = (1.0 - 2.0 * y * (n2 / n1)).max(0.0);
        let d2 = if n2 > 0.0 { (2.0 - 3.0 * y * (n3 / n2)).max(0.0) } else { d1 };
        let d3 = if n3 > 0.0 { (3.0 - 4.0 * y * (n4 / n3)).max(0.0) } else { d2 };
        (d1, d2, d3)
    };

    let (bd1, bd2, bd3) = get_discount(&bc_of_c);
    let (td1, td2, td3) = get_discount(&tc_of_c);

    let mut model = NGramModel::new();

    // Unigram probabilities under Kneser-Ney are based on continuation counts
    // P_cont(w) = N1+(•, w) / sum_w' N1+(•, w')
    let total_continuation: usize = uniq_left_for_w.values().copied().sum();
    for (w, &uniq_left) in &uniq_left_for_w {
        if total_continuation > 0 && uniq_left > 0 {
            let p = (uniq_left as f64) / (total_continuation as f64);
            model.insert_unigram(w, p.ln());
        }
    }

    // Helper: bigram KN backoff probability p_kn_backoff(w2 | w1) used as backing-off for trigrams
    let p_kn_bigram = |w1: &str, w2: &str| -> Option<f32> {
        let c_w1 = unigram_counts.get(w1).copied().unwrap_or(0) as f64;
        let c_w1w2 = bigram_counts.get(&(w1.to_string(), w2.to_string())).copied().unwrap_or(0) as f64;
        if c_w1 <= 0.0 {
            return None;
        }
        let left_cont = uniq_right_for_w1.get(w1).copied().unwrap_or(0) as f64; // number of unique continuations for w1
        let cont_w2 = uniq_left_for_w.get(w2).copied().unwrap_or(0) as f64; // N1+(•, w2)
        let p_cont_w2 = if total_bigram_types > 0.0 { cont_w2 / total_bigram_types } else { 0.0 };
        // pick discount based on integer count
        let cint = bigram_counts.get(&(w1.to_string(), w2.to_string())).copied().unwrap_or(0);
        let d = match cint {
            0 => bd1, // treat zero same as 1 for safety (shouldn't occur for stored bigrams)
            1 => bd1,
            2 => bd2,
            _ => bd3,
        };
        let first = ((c_w1w2 - d).max(0.0)) / c_w1;
        let lambda = if c_w1 > 0.0 { (d * left_cont) / c_w1 } else { 0.0 };
        Some(((first + lambda * p_cont_w2) as f32))
    };

    // Bigram probabilities (KN)
    for ((w1, w2), &cnt) in &bigram_counts {
        if let Some(p) = p_kn_bigram(w1, w2) {
            if p > 0.0 {
                model.insert_bigram(w1, w2, (p as f64).ln());
            }
        }
    }

    // Trigram probabilities (KN with backoff to KN bigrams)
    for ((w1, w2, w3), &cnt) in &trigram_counts {
        let c_w1w2 = bigram_counts.get(&(w1.clone(), w2.clone())).copied().unwrap_or(0) as f64;
    let c_w1w2w3 = cnt as f64;
        if c_w1w2 <= 0.0 {
            continue;
        }
        let left_cont = uniq_right_for_bigram.get(&(w1.clone(), w2.clone())).copied().unwrap_or(0) as f64;
        let cint = cnt;
        let d = match cint {
            0 => td1,
            1 => td1,
            2 => td2,
            _ => td3,
        };
        let first = ((c_w1w2w3 - d).max(0.0)) / c_w1w2;
        let lambda = if c_w1w2 > 0.0 { (d * left_cont) / c_w1w2 } else { 0.0 };
        // backoff prob: use bigram KN p(w3 | w2)
        let backoff = p_kn_bigram(w2, w3).unwrap_or(0.0);
        let p = first + lambda * (backoff as f64);
        if p > 0.0 {
            model.insert_trigram(w1, w2, w3, p.ln());
        }
    }

    // serialize to data/ngram.bincode
    std::fs::create_dir_all("data")?;
    let out = File::create("data/ngram.bincode").context("create output")?;
    bincode::serialize_into(out, &model).context("serialize model")?;
    println!("Wrote data/ngram.bincode (MLE normalized)");
    Ok(())
}

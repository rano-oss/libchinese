use anyhow::Result;
use clap::Parser;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use convert_tables::bigram_db::BigramDB;

// Simple port of estimate_interpolation: read a tagged model dump with
// \1-gram and \2-gram sections where lines are like:
// \item <id> <phrase> count <n>
// \item <id1> <phrase1> <id2> <phrase2> count <n>
// We'll compute per-token backoff/interpolation lambdas using a simple
// estimator: for each token t (id), compute the fraction of bigram mass
// where t appears as the left token, i.e. sum count(t, s) / sum count(t, *)
// and clamp/regularize to avoid zeros. This yields a single lambda in (0,1)
// which we'll print as `token:<id> lambda:<value>`.

#[derive(Parser)]
struct Args {
    /// interpolation model dump (.text)
    input: String,
    /// write output (estimator lines) to this file (default stdout)
    #[arg(long)]
    out: Option<String>,
    /// optional bigram db (bincode) to use for global normalization
    #[arg(long)]
    bigram_db: Option<PathBuf>,
    /// mode for normalization: 'left' (per-left conditional) or 'global' (use bigram db)
    #[arg(long, default_value = "left")]
    mode: String,
    // (no debug flag)
}

fn parse_item_line(line: &str) -> Option<(Vec<u64>, u64)> {
    // Expect lines like: "\\item 16801570 詞句 count 54"
    // or: "\\item 16801570 詞句 16779997 的 count 6"
    // We'll extract the numeric ids present and the final count.
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 3 { return None; }
    if parts[0] != "\\item" { return None; }
    // find trailing "count" token
    if let Some(count_pos) = parts.iter().rposition(|p| *p == "count") {
        if count_pos + 1 >= parts.len() { return None; }
        if let Ok(cnt) = parts[count_pos + 1].parse::<u64>() {
            // numeric ids are those parts that parse as u64 and are before the word-count
            let mut ids = Vec::new();
            for &p in &parts[1..count_pos] {
                if let Ok(idv) = p.parse::<u64>() {
                    ids.push(idv);
                }
            }
            return Some((ids, cnt));
        }
    }
    None
}

fn main() -> Result<()> {
    let args = Args::parse();
    let f = File::open(&args.input)?;
    let rdr = BufReader::new(f);

    let mut in_1gram = false;
    let mut in_2gram = false;

    // counts: unigram id -> count, bigram (left id) -> map right id -> count
    let mut uni_counts: HashMap<u64, u64> = HashMap::new();
    let mut bi_left_counts: HashMap<u64, HashMap<u64, u64>> = HashMap::new();
    let mut total_unigram_counts: u64 = 0;

    for line in rdr.lines() {
        let line = line?;
        let s = line.trim();
        if s.is_empty() { continue; }
        if s.starts_with("\\1-gram") { in_1gram = true; in_2gram = false; continue; }
        if s.starts_with("\\2-gram") { in_1gram = false; in_2gram = true; continue; }
    if s.starts_with("\\end") { break; }
        if in_1gram {
            if let Some((ids, cnt)) = parse_item_line(s) {
                if ids.len() >= 1 {
                    let id = ids[0];
                    let e = uni_counts.entry(id).or_insert(0);
                    *e += cnt;
                    total_unigram_counts += cnt;
                }
            }
        } else if in_2gram {
            if let Some((ids, cnt)) = parse_item_line(s) {
                if ids.len() >= 2 {
                    let left = ids[0];
                    let right = ids[1];
                    let map = bi_left_counts.entry(left).or_insert_with(HashMap::new);
                    let e = map.entry(right).or_insert(0);
                    *e += cnt;
                    // store bigram count per (left->right) in bi_left_counts
                }
            }
        }
    }

    // Load bigram DB if requested (for global mode)
    let mut maybe_db: Option<BigramDB> = None;
    if args.mode == "global" {
        if let Some(dbpath) = &args.bigram_db {
            maybe_db = Some(BigramDB::load_bincode(dbpath)?);
        } else {
            eprintln!("mode=global requested but --bigram-db not provided; falling back to left-mode");
        }
    }

    // Now implement a closer port of compute_interpolation: for each left token (deleted_bigram key)
    // iterate lambda until convergence using the formula from upstream.
    let mut out_lines: Vec<String> = Vec::new();

    // (no debug/verbose output)

    if bi_left_counts.is_empty() {
        // No bigram data available in the dump. Fall back to a unigram-only heuristic:
        // lambda = uni_count / (uni_count + mean_unigram_count)
        // This gives higher lambda for frequent tokens (they're likelier to have richer bigram behavior).
        if !uni_counts.is_empty() {
            let mean_uni = (total_unigram_counts as f64) / (uni_counts.len() as f64);
            for (&id, &u_cnt) in uni_counts.iter() {
                let lambda = (u_cnt as f64) / ((u_cnt as f64) + mean_uni);
                let lambda = lambda.max(1e-6).min(1.0 - 1e-6);
                out_lines.push(format!("token:{} lambda:{:.6}", id, lambda));
            }
        }
    } else {
        for (&left_id, right_map) in bi_left_counts.iter() {
            let table_total: u64 = right_map.values().copied().sum();
            if table_total == 0 { continue; }

            // per-left conditional normalization: P(r | l) = count(l,r) / sum_r count(l,r)
            let left_total_f = table_total as f64;

            let mut lambda: f64 = 0.0;
            let mut next_lambda: f64 = 0.6;
            let eps = 0.001_f64;

            // iterate until convergence
            while (lambda - next_lambda).abs() > eps {
                lambda = next_lambda;
                next_lambda = 0.0;

                // for each deleted bigram item (right token)
                for (&right_id, &deleted_count) in right_map.iter() {
                    // choose elem_poss_bigram based on mode
                    let elem_poss_bigram = if args.mode == "global" {
                        if let Some(db) = &maybe_db {
                            let freq = *db.bigram_right_freqs.get(&right_id).unwrap_or(&0u128) as f64;
                            if db.total_bigram_counts > 0 {
                                freq / (db.total_bigram_counts as f64)
                            } else { 0.0 }
                        } else {
                            // fallback to per-left
                            let freq_lr = *right_map.get(&right_id).unwrap_or(&0u64) as f64;
                            if left_total_f > 0.0 { freq_lr / left_total_f } else { 0.0 }
                        }
                    } else {
                        let freq_lr = *right_map.get(&right_id).unwrap_or(&0u64) as f64;
                        if left_total_f > 0.0 { freq_lr / left_total_f } else { 0.0 }
                    };

                    // unigram probability for right token
                    let elem_poss_unigram = if total_unigram_counts > 0 {
                        (*uni_counts.get(&right_id).unwrap_or(&0u64) as f64) / (total_unigram_counts as f64)
                    } else { 0.0 };

                    let numerator = lambda * elem_poss_bigram;
                    let part_of_denominator = (1.0 - lambda) * elem_poss_unigram;

                    let denom = numerator + part_of_denominator;
                    if denom == 0.0 { continue; }

                    next_lambda += (deleted_count as f64) * (numerator / denom);
                }

                // divide by table_total (deleted_bigram total freq)
                next_lambda /= table_total as f64;
            }

            // clamp and emit
            let final_lambda = next_lambda.max(1e-6).min(1.0 - 1e-6);
            out_lines.push(format!("token:{} lambda:{:.6}", left_id, final_lambda));
        }
    }

    // If no bigram data for a token, we could still emit a small lambda based on unigram freq; skip for now.

    if let Some(outp) = args.out {
        let mut of = File::create(outp)?;
        for l in out_lines.iter() { writeln!(of, "{}", l)?; }
    } else {
        let mut stdout = std::io::stdout();
        for l in out_lines.iter() { writeln!(stdout, "{}", l)?; }
    }

    Ok(())
}

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

    // Build unigram, bigram and trigram counts from payloads
    let mut unigram_counts: HashMap<String, u64> = HashMap::new();
    let mut bigram_counts: HashMap<(String, String), u64> = HashMap::new();
    let mut trigram_counts: HashMap<(String, String, String), u64> = HashMap::new();

    for list in payloads.iter() {
        for pe in list.iter() {
            let text = &pe.0;
            let chars: Vec<String> = text.chars().map(|c| c.to_string()).collect();
            
            // Count unigrams
            for ch in chars.iter() {
                *unigram_counts.entry(ch.clone()).or_default() += 1;
            }
            
            // Count bigrams
            for i in 0..chars.len().saturating_sub(1) {
                *bigram_counts.entry((chars[i].clone(), chars[i + 1].clone())).or_default() += 1;
            }
            
            // Count trigrams
            for i in 0..chars.len().saturating_sub(2) {
                *trigram_counts.entry((chars[i].clone(), chars[i + 1].clone(), chars[i + 2].clone())).or_default() += 1;
            }
        }
    }

    // Compute deleted pairs/triples for cross-validation
    let deleted_bigrams = bigram_counts.clone();
    let deleted_trigrams = trigram_counts.clone();

    // Compute unique prefixes list (character-level prefixes from bigrams)
    let mut prefixes: Vec<String> = {
        let mut s = HashSet::new();
        for ((w1, _), _) in deleted_bigrams.iter() {
            s.insert(w1.clone());
        }
        let mut v: Vec<_> = s.into_iter().collect();
        v.sort();
        v
    };

    // prepare outputs: write fst and a bincode lambdas vector
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

    // Prepare a bincode vector for all lambdas
    let mut b_vec: Vec<Lambdas> = Vec::with_capacity(prefixes.len());
    for key in prefixes.iter() {
        let (l1, l2, l3) = compute_lambda_for_prefix(
            &deleted_bigrams,
            &deleted_trigrams,
            key,
            &unigram_counts,
            &bigram_counts,
            &trigram_counts,
        );
        b_vec.push(Lambdas([l1, l2, l3]));
    }
    let bbytes = bincode::serialize(&b_vec)?;
    std::fs::write(&bincode_path, &bbytes)?;

    println!("wrote {} + {}", fst_path.display(), bincode_path.display());
    Ok(())
}

fn compute_lambda_for_prefix(
    deleted_bigrams: &HashMap<(String, String), u64>,
    deleted_trigrams: &HashMap<(String, String, String), u64>,
    prefix: &str,
    unigram_counts: &HashMap<String, u64>,
    bigram_counts: &HashMap<(String, String), u64>,
    trigram_counts: &HashMap<(String, String, String), u64>,
) -> (f32, f32, f32) {
    // Implement 3-way deleted interpolation estimation
    // Based on maximum likelihood estimation using leave-one-out counts
    
    let total_uni: f64 = unigram_counts.values().map(|&v| v as f64).sum();
    let total_bi: f64 = bigram_counts.values().map(|&v| v as f64).sum();
    let total_tri: f64 = trigram_counts.values().map(|&v| v as f64).sum();
    
    if total_uni == 0.0 {
        return (1.0, 0.0, 0.0); // fallback to unigram only
    }
    
    // Collect relevant contexts for this prefix
    let mut l1_sum = 0.0;
    let mut l2_sum = 0.0;
    let mut l3_sum = 0.0;
    let mut count = 0;
    
    // For each trigram starting with prefix, compute which level predicts best
    for ((w1, w2, w3), &c) in deleted_trigrams.iter() {
        if w1 != prefix {
            continue;
        }
        
        count += 1;
        let c_tri = c as f64;
        
        // Leave-one-out counts (delete this trigram occurrence)
        let c_w1w2w3_minus = (c_tri - 1.0).max(0.0);
        let c_w1w2 = *bigram_counts.get(&(w1.clone(), w2.clone())).unwrap_or(&0) as f64;
        let c_w2w3 = *bigram_counts.get(&(w2.clone(), w3.clone())).unwrap_or(&0) as f64;
        let c_w2 = *unigram_counts.get(w2).unwrap_or(&0) as f64;
        let c_w3 = *unigram_counts.get(w3).unwrap_or(&0) as f64;
        
        // Compute probabilities using deleted counts
        let p_tri = if c_w1w2 > 1.0 { c_w1w2w3_minus / (c_w1w2 - 1.0) } else { 0.0 };
        let p_bi = if c_w2 > 0.0 { c_w2w3 / (c_w2 + total_bi * 0.001) } else { 0.0 };
        let p_uni = if total_uni > 0.0 { c_w3 / total_uni } else { 0.0 };
        
        // Assign weight to the level that gives highest probability
        if p_tri >= p_bi && p_tri >= p_uni {
            l3_sum += c_tri;
        } else if p_bi >= p_uni {
            l2_sum += c_tri;
        } else {
            l1_sum += c_tri;
        }
    }
    
    // Also consider bigrams (when no trigram context)
    for ((w1, w2), &c) in deleted_bigrams.iter() {
        if w1 != prefix {
            continue;
        }
        
        let c_bi = c as f64;
        let c_w1 = *unigram_counts.get(w1).unwrap_or(&0) as f64;
        let c_w2 = *unigram_counts.get(w2).unwrap_or(&0) as f64;
        
        // Leave-one-out
        let c_w1w2_minus = (c_bi - 1.0).max(0.0);
        let p_bi = if c_w1 > 1.0 { c_w1w2_minus / (c_w1 - 1.0) } else { 0.0 };
        let p_uni = if total_uni > 0.0 { c_w2 / total_uni } else { 0.0 };
        
        if p_bi >= p_uni {
            l2_sum += c_bi * 0.5; // weight bigram evidence less when no trigram
        } else {
            l1_sum += c_bi * 0.5;
        }
    }
    
    // Normalize
    let total = l1_sum + l2_sum + l3_sum;
    if total > 0.0 {
        let mut l1 = (l1_sum / total) as f32;
        let mut l2 = (l2_sum / total) as f32;
        let mut l3 = (l3_sum / total) as f32;
        
        // Ensure minimum weights and sum to 1.0
        let min_weight = 0.01;
        l1 = l1.max(min_weight);
        l2 = l2.max(min_weight);
        l3 = l3.max(min_weight);
        
        let sum = l1 + l2 + l3;
        (l1 / sum, l2 / sum, l3 / sum)
    } else {
        // Fallback: reasonable defaults favoring bigrams slightly
        (0.2, 0.5, 0.3)
    }
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

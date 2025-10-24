use anyhow::Result;
use fst::MapBuilder;
use serde::{Deserialize, Serialize};
use libchinese_core::{NGramModel, Lambdas};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{create_dir_all, File};
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
struct LexEntry {
    utf8: String,
    token: u32,
    freq: u32,
}
impl Clone for LexEntry {
    fn clone(&self) -> Self {
        Self { utf8: self.utf8.clone(), token: self.token, freq: self.freq }
    }
}

fn parse_table_line(line: &str) -> Option<(String, String, u32, u32)> {
    // expected: key\tchars\ttoken\tfreq
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() < 4 {
        return None;
    }
    let key = parts[0].to_string();
    let chars = parts[1].to_string();
    let token = parts[2].parse::<u32>().unwrap_or(0);
    let freq = parts[3].trim().parse::<u32>().unwrap_or(0);
    Some((key, chars, token, freq))
}

fn build_fst_and_bincode<P: AsRef<Path>>(table_paths: &[(&str, P)], out_prefix: &Path, token_mode: &str, key_type: &str) -> Result<()> {
    // Collect entries into a map keyed by pinyin/zhuyin key -> Vec<LexEntry>
    let mut grouped: BTreeMap<String, Vec<LexEntry>> = BTreeMap::new();

    for (name, path) in table_paths.iter() {
        let f = File::open(path)?;
        let reader = BufReader::new(f);
        for line in reader.lines() {
            let l = line?;
            if l.trim().is_empty() { continue; }
            if let Some((key, chars, token, freq)) = parse_table_line(&l) {
                // Determine the actual key based on key_type parameter:
                // - "pinyin": convert zhuyin keys to toneless pinyin
                // - "zhuyin": keep original zhuyin/bopomofo keys
                // - "original": keep keys as-is (for non-tsi tables)
                let actual_key = if name == &"tsi" {
                    match key_type {
                        "pinyin" => {
                            // normalize each syllable produced by conversion
                            let raw = convert_zhuyin_key_to_pinyin(&key);
                            let parts: Vec<String> = raw.split('\'')
                                .map(|p| normalize_pinyin_syllable(p))
                                .collect();
                            parts.join("'")
                        }
                        "zhuyin" => {
                            // Keep original bopomofo/zhuyin key WITH tone marks
                            key.clone()
                        }
                        _ => key.clone()
                    }
                } else {
                    // pinyin data already
                    key.clone()
                };
                grouped.entry(actual_key).or_default().push(LexEntry { utf8: chars, token, freq });
            }
        }
    }

    // Build FST map where each key maps to a monotonically increasing u64 index
    let fst_path = out_prefix.join("lexicon.fst");
    let bin_path = out_prefix.join("lexicon.bincode");
    create_dir_all(out_prefix)?;
    let mut w = File::create(&fst_path)?;
    let mut map_builder = MapBuilder::new(&mut w)?;

    // We'll also collect entries (key + payload) so we can build ngrams from keys when requested
    let mut entries: Vec<(String, Vec<LexEntry>)> = Vec::new();

    for (i, (k, v)) in grouped.into_iter().enumerate() {
        map_builder.insert(&k, i as u64)?;
        entries.push((k, v));
    }
    // payloads vector is the serialized lists in the same order
    let mut payloads: Vec<Vec<LexEntry>> = entries.iter().map(|(_, v)| v.clone()).collect();
    map_builder.finish()?;

    // write bincode vector (lexicon payloads)
    let mut binf = File::create(&bin_path)?;
    bincode::serialize_into(&mut binf, &payloads)?;

    // Compute unigram (token -> total freq) from payloads and write ngram.bincode
    // Build simple unigram/bigram/trigram counts from payload texts for NGramModel
    let mut unigram_counts: HashMap<String, u64> = HashMap::new();
    let mut bigram_counts: HashMap<(String, String), u64> = HashMap::new();
    let mut trigram_counts: HashMap<(String, String, String), u64> = HashMap::new();

    // Build ngram counts according to token_mode: if pinyin_syllable, use keys; otherwise use phrase text char tokens
    if token_mode == "pinyin_syllable" {
        for (key, _v) in entries.iter() {
            // split key on apostrophes into syllables
            let tokens: Vec<String> = key.split('\'').map(|s| s.to_string()).collect();
            for i in 0..tokens.len() {
                *unigram_counts.entry(tokens[i].clone()).or_default() += 1;
                if i + 1 < tokens.len() {
                    *bigram_counts.entry((tokens[i].clone(), tokens[i+1].clone())).or_default() += 1;
                }
                if i + 2 < tokens.len() {
                    *trigram_counts.entry((tokens[i].clone(), tokens[i+1].clone(), tokens[i+2].clone())).or_default() += 1;
                }
            }
        }
    } else {
        for list in payloads.iter() {
            for e in list.iter() {
                let text = &e.utf8;
                let tokens: Vec<String> = tokenize_text(text, "char");
                for i in 0..tokens.len() {
                    *unigram_counts.entry(tokens[i].clone()).or_default() += 1;
                    if i + 1 < tokens.len() {
                        *bigram_counts.entry((tokens[i].clone(), tokens[i+1].clone())).or_default() += 1;
                    }
                    if i + 2 < tokens.len() {
                        *trigram_counts.entry((tokens[i].clone(), tokens[i+1].clone(), tokens[i+2].clone())).or_default() += 1;
                    }
                }
            }
        }
    }

    // Build NGramModel using serialize_ngram logic (Modified Kneser-Ney)
    let mut model = NGramModel::new();

    // Precompute continuation counts similar to serialize_ngram
    use std::collections::HashSet;
    let mut left_sets: HashMap<String, HashSet<String>> = HashMap::new();
    let mut right_sets_by_left: HashMap<String, HashSet<String>> = HashMap::new();
    let mut right_sets_by_bigram: HashMap<(String, String), HashSet<String>> = HashMap::new();

    for ((w1, w2), _) in &bigram_counts {
        left_sets.entry(w2.clone()).or_default().insert(w1.clone());
        right_sets_by_left.entry(w1.clone()).or_default().insert(w2.clone());
    }
    for ((w1, w2, w3), _) in &trigram_counts {
        right_sets_by_bigram.entry((w1.clone(), w2.clone())).or_default().insert(w3.clone());
        right_sets_by_left.entry(w2.clone()).or_default().insert(w3.clone());
        left_sets.entry(w3.clone()).or_default().insert(w2.clone());
    }

    let mut uniq_left_for_w: HashMap<String, usize> = HashMap::new();
    let mut uniq_right_for_w1: HashMap<String, usize> = HashMap::new();
    let mut uniq_right_for_bigram: HashMap<(String, String), usize> = HashMap::new();

    for (w, s) in left_sets.into_iter() { uniq_left_for_w.insert(w, s.len()); }
    for (w1, s) in right_sets_by_left.into_iter() { uniq_right_for_w1.insert(w1, s.len()); }
    for (bg, s) in right_sets_by_bigram.into_iter() { uniq_right_for_bigram.insert(bg, s.len()); }

    let total_bigram_types = bigram_counts.len() as f64;

    // counts-of-counts
    let mut bc_of_c: HashMap<u64, usize> = HashMap::new();
    for &cnt in bigram_counts.values() { *bc_of_c.entry(cnt).or_default() += 1; }
    let mut tc_of_c: HashMap<u64, usize> = HashMap::new();
    for &cnt in trigram_counts.values() { *tc_of_c.entry(cnt).or_default() += 1; }

    let get_discount = |cc: &HashMap<u64, usize>| -> (f64, f64, f64) {
        let n1 = *cc.get(&1).unwrap_or(&0) as f64;
        let n2 = *cc.get(&2).unwrap_or(&0) as f64;
        let n3 = *cc.get(&3).unwrap_or(&0) as f64;
        let n4 = *cc.get(&4).unwrap_or(&0) as f64;
        if n1 == 0.0 || n2 == 0.0 { return (0.75, 0.75, 0.75); }
        let y = n1 / (n1 + 2.0 * n2);
        let d1 = (1.0 - 2.0 * y * (n2 / n1)).max(0.0);
        let d2 = if n2 > 0.0 { (2.0 - 3.0 * y * (n3 / n2)).max(0.0) } else { d1 };
        let d3 = if n3 > 0.0 { (3.0 - 4.0 * y * (n4 / n3)).max(0.0) } else { d2 };
        (d1, d2, d3)
    };

    let (bd1, bd2, bd3) = get_discount(&bc_of_c);
    let (td1, td2, td3) = get_discount(&tc_of_c);

    // Unigram continuation probs
    let total_continuation: usize = uniq_left_for_w.values().copied().sum();
    for (w, &uniq_left) in &uniq_left_for_w {
        if total_continuation > 0 && uniq_left > 0 {
            let p = (uniq_left as f64) / (total_continuation as f64);
            model.insert_unigram(w, p.ln());
        }
    }

    let p_kn_bigram = |w1: &str, w2: &str| -> Option<f32> {
        let c_w1 = unigram_counts.get(w1).copied().unwrap_or(0) as f64;
        let c_w1w2 = bigram_counts.get(&(w1.to_string(), w2.to_string())).copied().unwrap_or(0) as f64;
        if c_w1 <= 0.0 { return None; }
        let left_cont = uniq_right_for_w1.get(w1).copied().unwrap_or(0) as f64;
        let cont_w2 = uniq_left_for_w.get(w2).copied().unwrap_or(0) as f64;
        let p_cont_w2 = if total_bigram_types > 0.0 { cont_w2 / total_bigram_types } else { 0.0 };
        let cint = bigram_counts.get(&(w1.to_string(), w2.to_string())).copied().unwrap_or(0);
        let d = match cint { 0 => bd1, 1 => bd1, 2 => bd2, _ => bd3 };
        let first = ((c_w1w2 - d).max(0.0)) / c_w1;
        let lambda = if c_w1 > 0.0 { (d * left_cont) / c_w1 } else { 0.0 };
        Some(((first + lambda * p_cont_w2) as f32))
    };

    for ((w1, w2), &cnt) in &bigram_counts {
        if let Some(p) = p_kn_bigram(w1, w2) { if p > 0.0 { model.insert_bigram(w1, w2, (p as f64).ln()); } }
    }

    for ((w1, w2, w3), &cnt) in &trigram_counts {
        let c_w1w2 = bigram_counts.get(&(w1.clone(), w2.clone())).copied().unwrap_or(0) as f64;
        if c_w1w2 <= 0.0 { continue; }
        let left_cont = uniq_right_for_bigram.get(&(w1.clone(), w2.clone())).copied().unwrap_or(0) as f64;
        let cint = cnt;
        let d = match cint { 0 => td1, 1 => td1, 2 => td2, _ => td3 };
        let first = ((cnt as f64 - d).max(0.0)) / c_w1w2;
        let lambda = if c_w1w2 > 0.0 { (d * left_cont) / c_w1w2 } else { 0.0 };
        let backoff = p_kn_bigram(w2, w3).unwrap_or(0.0);
        let p = first + lambda * (backoff as f64);
        if p > 0.0 { model.insert_trigram(w1, w2, w3, p.ln()); }
    }

    let ngram_path = out_prefix.join("ngram.bincode");
    let mut nbf = File::create(&ngram_path)?;
    bincode::serialize_into(&mut nbf, &model)?;

    println!("Wrote {} entries, fst={} bincode={}", payloads.len(), fst_path.display(), bin_path.display());
    
    // Now compute and write interpolation lambdas using character-level n-grams from phrase text
    estimate_lambdas_for_dataset(out_prefix, &payloads)?;
    
    Ok(())
}

/// Estimate interpolation lambda weights for a dataset using deleted interpolation
fn estimate_lambdas_for_dataset<P: AsRef<Path>>(dataset_dir: P, payloads: &[Vec<LexEntry>]) -> Result<()> {
    let dataset_dir = dataset_dir.as_ref();

    // Build character-level unigram, bigram and trigram counts from phrase texts
    let mut unigram_counts: HashMap<String, u64> = HashMap::new();
    let mut bigram_counts: HashMap<(String, String), u64> = HashMap::new();
    let mut trigram_counts: HashMap<(String, String, String), u64> = HashMap::new();

    for list in payloads.iter() {
        for entry in list.iter() {
            let text = &entry.utf8;
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

    // Compute unique prefixes list (character-level prefixes from bigrams)
    let prefixes: Vec<String> = {
        let mut s = HashSet::new();
        for ((w1, _), _) in bigram_counts.iter() {
            s.insert(w1.clone());
        }
        let mut v: Vec<_> = s.into_iter().collect();
        v.sort();
        v
    };

    // Build FST for lambdas
    let fst_path = dataset_dir.join("lambdas.fst");
    let bincode_path = dataset_dir.join("lambdas.bincode");

    let mut map_builder = MapBuilder::new(Vec::new())?;
    for (i, key) in prefixes.iter().enumerate() {
        map_builder.insert(key, i as u64)?;
    }
    let fst_bytes = map_builder.into_inner()?;
    std::fs::write(&fst_path, &fst_bytes)?;

    // Compute lambdas for each prefix
    let mut b_vec: Vec<Lambdas> = Vec::with_capacity(prefixes.len());
    for key in prefixes.iter() {
        let (l1, l2, l3) = compute_lambda_for_prefix(
            &bigram_counts,
            &trigram_counts,
            key,
            &unigram_counts,
        );
        b_vec.push(Lambdas([l1, l2, l3]));
    }
    let bbytes = bincode::serialize(&b_vec)?;
    std::fs::write(&bincode_path, &bbytes)?;

    println!("Wrote lambdas: {} + {}", fst_path.display(), bincode_path.display());
    Ok(())
}

/// Compute per-prefix lambda weights using 3-way deleted interpolation
fn compute_lambda_for_prefix(
    bigram_counts: &HashMap<(String, String), u64>,
    trigram_counts: &HashMap<(String, String, String), u64>,
    prefix: &str,
    unigram_counts: &HashMap<String, u64>,
) -> (f32, f32, f32) {
    let total_uni: f64 = unigram_counts.values().map(|&v| v as f64).sum();
    let total_bi: f64 = bigram_counts.values().map(|&v| v as f64).sum();
    
    if total_uni == 0.0 {
        return (1.0, 0.0, 0.0);
    }
    
    let mut l1_sum = 0.0;
    let mut l2_sum = 0.0;
    let mut l3_sum = 0.0;
    
    // For each trigram starting with prefix, compute which level predicts best
    for ((w1, w2, w3), &c) in trigram_counts.iter() {
        if w1 != prefix {
            continue;
        }
        
        let c_tri = c as f64;
        let c_w1w2 = *bigram_counts.get(&(w1.clone(), w2.clone())).unwrap_or(&0) as f64;
        let c_w2w3 = *bigram_counts.get(&(w2.clone(), w3.clone())).unwrap_or(&0) as f64;
        let c_w2 = *unigram_counts.get(w2).unwrap_or(&0) as f64;
        let c_w3 = *unigram_counts.get(w3).unwrap_or(&0) as f64;
        
        // Leave-one-out probabilities
        let c_w1w2w3_minus = (c_tri - 1.0).max(0.0);
        let p_tri = if c_w1w2 > 1.0 { c_w1w2w3_minus / (c_w1w2 - 1.0) } else { 0.0 };
        let p_bi = if c_w2 > 0.0 { c_w2w3 / (c_w2 + total_bi * 0.001) } else { 0.0 };
        let p_uni = if total_uni > 0.0 { c_w3 / total_uni } else { 0.0 };
        
        // Assign weight to best predictor
        if p_tri >= p_bi && p_tri >= p_uni {
            l3_sum += c_tri;
        } else if p_bi >= p_uni {
            l2_sum += c_tri;
        } else {
            l1_sum += c_tri;
        }
    }
    
    // Also consider bigrams (when no trigram context)
    for ((w1, w2), &c) in bigram_counts.iter() {
        if w1 != prefix {
            continue;
        }
        
        let c_bi = c as f64;
        let c_w1 = *unigram_counts.get(w1).unwrap_or(&0) as f64;
        let c_w2 = *unigram_counts.get(w2).unwrap_or(&0) as f64;
        
        let c_w1w2_minus = (c_bi - 1.0).max(0.0);
        let p_bi = if c_w1 > 1.0 { c_w1w2_minus / (c_w1 - 1.0) } else { 0.0 };
        let p_uni = if total_uni > 0.0 { c_w2 / total_uni } else { 0.0 };
        
        if p_bi >= p_uni {
            l2_sum += c_bi * 0.5;
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
        
        let min_weight = 0.01;
        l1 = l1.max(min_weight);
        l2 = l2.max(min_weight);
        l3 = l3.max(min_weight);
        
        let sum = l1 + l2 + l3;
        (l1 / sum, l2 / sum, l3 / sum)
    } else {
        (0.2, 0.5, 0.3)
    }
}

fn tokenize_text(text: &str, mode: &str) -> Vec<String> {
    match mode {
        "pinyin_syllable" => {
            // pinyin syllables are separated by apostrophes in keys; for phrase text we don't have syllable markers,
            // so as a heuristic split on characters but try to coalesce ASCII pinyin-like sequences if present.
            // In practice convert_table uses characters for payload text (Chinese characters), so for pinyin_syllable
            // we instead attempt to split keys elsewhere; returning char tokens as fallback.
            // fallback to char tokens for phrase text
            text.chars().map(|c| c.to_string()).collect()
        }
        _ => {
            text.chars().map(|c| c.to_string()).collect()
        }
    }
}

// Strip zhuyin tone marks and diacritics: ˊ ˇ ˋ ˙ and combining variants
fn strip_zhuyin_tone(s: &str) -> String {
    s.chars()
        .filter(|c| match *c {
            '\u{02CA}' | '\u{02C7}' | '\u{02CB}' | '\u{02D9}' | '\u{0304}' => false,
            _ => true,
        })
        .collect()
}

fn zhuyin_char_to_pinyin_fragment(ch: char) -> Option<&'static str> {
    // mapping table for individual bopomofo chars to pinyin fragments
    match ch {
        'ㄅ' => Some("b"), 'ㄆ' => Some("p"), 'ㄇ' => Some("m"), 'ㄈ' => Some("f"),
        'ㄉ' => Some("d"), 'ㄊ' => Some("t"), 'ㄋ' => Some("n"), 'ㄌ' => Some("l"),
        'ㄍ' => Some("g"), 'ㄎ' => Some("k"), 'ㄏ' => Some("h"), 'ㄐ' => Some("j"),
        'ㄑ' => Some("q"), 'ㄒ' => Some("x"), 'ㄓ' => Some("zh"), 'ㄔ' => Some("ch"),
        'ㄕ' => Some("sh"), 'ㄖ' => Some("r"), 'ㄗ' => Some("z"), 'ㄘ' => Some("c"),
        'ㄙ' => Some("s"),
        // finals & medial
        'ㄧ' => Some("i"), 'ㄨ' => Some("u"), 'ㄩ' => Some("v"),
        'ㄚ' => Some("a"), 'ㄛ' => Some("o"), 'ㄜ' => Some("e"), 'ㄝ' => Some("e"),
        'ㄞ' => Some("ai"), 'ㄟ' => Some("ei"), 'ㄠ' => Some("ao"), 'ㄡ' => Some("ou"),
        'ㄢ' => Some("an"), 'ㄣ' => Some("en"), 'ㄤ' => Some("ang"), 'ㄥ' => Some("eng"),
        'ㄦ' => Some("er"),
        // tonal marks and variation chars are filtered earlier
        _ => None,
    }
}

fn convert_zhuyin_syllable_to_pinyin(syll: &str) -> String {
    // strip tone marks
    let cleaned = strip_zhuyin_tone(syll);
    // build by mapping each zhuyin char
    let mut out = String::new();
    for ch in cleaned.chars() {
        if let Some(frag) = zhuyin_char_to_pinyin_fragment(ch) {
            out.push_str(frag);
        }
    }

    // Normalization rules for syllables starting with i/u/v
    // If starts with i + vowel -> replace leading i with y (e.g., ia -> ya, iou -> you)
    if out.starts_with('i') {
        if out.len() >= 2 {
            let rest = &out[1..];
            // Only convert when rest starts with a vowel
            if rest.starts_with('a') || rest.starts_with('o') || rest.starts_with('e') || rest.starts_with('u') || rest.starts_with('i') {
                out = format!("y{}", rest);
            }
        }
    }
    // If starts with u + vowel -> w prefix
    if out.starts_with('u') {
        if out.len() >= 2 {
            let rest = &out[1..];
            if rest.starts_with('a') || rest.starts_with('o') || rest.starts_with('e') || rest.starts_with('i') {
                out = format!("w{}", rest);
            }
        }
    }
    // If starts with v (we used v for ü), convert to yu or just u-like handling
    if out.starts_with('v') {
        let rest = &out[1..];
        out = format!("yu{}", rest);
    }

    out
}

fn convert_zhuyin_key_to_pinyin(key: &str) -> String {
    // split on apostrophe markers ' (U+0027) and also support U+2019?
    let parts: Vec<&str> = key.split('\'').collect();
    let mut out_parts: Vec<String> = Vec::new();
    for p in parts.iter() {
        let p_trim = p.trim();
        if p_trim.is_empty() { continue; }
        out_parts.push(convert_zhuyin_syllable_to_pinyin(p_trim));
    }
    out_parts.join("'")
}

fn normalize_pinyin_syllable(s: &str) -> String {
    // small set of normalization rules to map common outputs into canonical forms
    // e.g., 'yue' vs 'ue' boundaries, 'yu' handling already, 'iou' -> 'iu'
    let mut s = s.to_string();
    if s == "iou" { s = "iu".to_string(); }
    if s == "uei" { s = "ui".to_string(); }
    if s == "uen" { s = "un".to_string(); }
    if s.starts_with("y") && s.len() > 1 {
        // y + vowel -> leave as-is
    }
    if s.starts_with("w") && s.len() > 1 {
        // w + vowel -> leave
    }
    s
}

fn main() -> Result<()> {
    // Hardcoded paths (project-relative)
    // repo-root relative paths (run from repository root)
    let data_dir = Path::new("data");
    let zhuyin_dir = Path::new("data/zhuyin");
    let out_dir = Path::new("data/converted");

    // Cases:
    // 1) simplified pinyin: gb_char.table + merged.table + opengram.table + punct.table
    let simplified_tables = [
        ("gb_char", data_dir.join("gb_char.table")),
        ("merged", data_dir.join("merged.table")),
        ("opengram", data_dir.join("opengram.table")),
        ("punct", data_dir.join("punct.table")),
    ];

    // 2) traditional pinyin: use tsi.table (converted via zhuyin to pinyin mapping later)
    let traditional_tables = [
        ("tsi", zhuyin_dir.join("tsi.table")),
    ];

    // 3) zhuyin traditional: use tsi.table only
    let zhuyin_tables = [
        ("tsi", zhuyin_dir.join("tsi.table")),
    ];
    
    // 4) emoji: emoji.table (pinyin keywords)
    let emoji_tables = [
        ("emoji", data_dir.join("emoji.table")),
    ];

    // Build simplified (pinyin syllable tokenization)
    build_fst_and_bincode(&simplified_tables, &out_dir.join("simplified"), "pinyin_syllable", "original")?;

    // Build traditional (pinyin syllable tokenization, convert zhuyin keys to pinyin)
    build_fst_and_bincode(&traditional_tables, &out_dir.join("traditional"), "pinyin_syllable", "pinyin")?;

    // Build zhuyin (character tokenization, keep zhuyin/bopomofo keys)
    build_fst_and_bincode(&zhuyin_tables, &out_dir.join("zhuyin_traditional"), "char", "zhuyin")?;
    
    // Build emoji (pinyin syllable tokenization, original keys)
    if data_dir.join("emoji.table").exists() {
        println!("Building emoji lexicon...");
        build_fst_and_bincode(&emoji_tables, &out_dir.join("emoji"), "pinyin_syllable", "original")?;
    } else {
        println!("Skipping emoji (emoji.table not found)");
    }

    // No global placeholders here; each dataset has its own ngram/interp artifacts.

    Ok(())
}

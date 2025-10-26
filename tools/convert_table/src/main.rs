use anyhow::Result;
use fst::MapBuilder;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
        Self {
            utf8: self.utf8.clone(),
            token: self.token,
            freq: self.freq,
        }
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

fn build_fst_and_bincode<P: AsRef<Path>>(
    table_paths: &[(&str, P)],
    out_prefix: &Path,
    key_type: &str,
) -> Result<()> {
    // Collect entries into a map keyed by pinyin/zhuyin key -> Vec<LexEntry>
    let mut grouped: BTreeMap<String, Vec<LexEntry>> = BTreeMap::new();

    for (name, path) in table_paths.iter() {
        let f = File::open(path)?;
        let reader = BufReader::new(f);
        for line in reader.lines() {
            let l = line?;
            if l.trim().is_empty() {
                continue;
            }
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
                            let parts: Vec<String> =
                                raw.split('\'').map(normalize_pinyin_syllable).collect();
                            parts.join("'")
                        }
                        "zhuyin" => {
                            // Keep original bopomofo/zhuyin key WITH tone marks
                            key.clone()
                        }
                        _ => key.clone(),
                    }
                } else {
                    // pinyin data already
                    key.clone()
                };
                grouped.entry(actual_key).or_default().push(LexEntry {
                    utf8: chars,
                    token,
                    freq,
                });
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
    let payloads: Vec<Vec<LexEntry>> = entries.iter().map(|(_, v)| v.clone()).collect();
    map_builder.finish()?;

    // write bincode vector (lexicon payloads)
    let mut binf = File::create(&bin_path)?;
    bincode::serialize_into(&mut binf, &payloads)?;

    Ok(())
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
        'ㄅ' => Some("b"),
        'ㄆ' => Some("p"),
        'ㄇ' => Some("m"),
        'ㄈ' => Some("f"),
        'ㄉ' => Some("d"),
        'ㄊ' => Some("t"),
        'ㄋ' => Some("n"),
        'ㄌ' => Some("l"),
        'ㄍ' => Some("g"),
        'ㄎ' => Some("k"),
        'ㄏ' => Some("h"),
        'ㄐ' => Some("j"),
        'ㄑ' => Some("q"),
        'ㄒ' => Some("x"),
        'ㄓ' => Some("zh"),
        'ㄔ' => Some("ch"),
        'ㄕ' => Some("sh"),
        'ㄖ' => Some("r"),
        'ㄗ' => Some("z"),
        'ㄘ' => Some("c"),
        'ㄙ' => Some("s"),
        // finals & medial
        'ㄧ' => Some("i"),
        'ㄨ' => Some("u"),
        'ㄩ' => Some("v"),
        'ㄚ' => Some("a"),
        'ㄛ' => Some("o"),
        'ㄜ' => Some("e"),
        'ㄝ' => Some("e"),
        'ㄞ' => Some("ai"),
        'ㄟ' => Some("ei"),
        'ㄠ' => Some("ao"),
        'ㄡ' => Some("ou"),
        'ㄢ' => Some("an"),
        'ㄣ' => Some("en"),
        'ㄤ' => Some("ang"),
        'ㄥ' => Some("eng"),
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
    if out.starts_with('i') && out.len() >= 2 {
        let rest = &out[1..];
        // Only convert when rest starts with a vowel
        if rest.starts_with('a')
            || rest.starts_with('o')
            || rest.starts_with('e')
            || rest.starts_with('u')
            || rest.starts_with('i')
        {
            out = format!("y{}", rest);
        }
    }
    // If starts with u + vowel -> w prefix
    if out.starts_with('u') && out.len() >= 2 {
        let rest = &out[1..];
        if rest.starts_with('a')
            || rest.starts_with('o')
            || rest.starts_with('e')
            || rest.starts_with('i')
        {
            out = format!("w{}", rest);
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
        if p_trim.is_empty() {
            continue;
        }
        out_parts.push(convert_zhuyin_syllable_to_pinyin(p_trim));
    }
    out_parts.join("'")
}

fn normalize_pinyin_syllable(s: &str) -> String {
    // small set of normalization rules to map common outputs into canonical forms
    // e.g., 'yue' vs 'ue' boundaries, 'yu' handling already, 'iou' -> 'iu'
    let mut s = s.to_string();
    if s == "iou" {
        s = "iu".to_string();
    }
    if s == "uei" {
        s = "ui".to_string();
    }
    if s == "uen" {
        s = "un".to_string();
    }
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
    let traditional_tables = [("tsi", zhuyin_dir.join("tsi.table"))];

    // 3) zhuyin traditional: use tsi.table only
    let zhuyin_tables = [("tsi", zhuyin_dir.join("tsi.table"))];

    // 4) emoji: emoji.table (pinyin keywords)
    let emoji_tables = [("emoji", data_dir.join("emoji.table"))];

    // Build simplified (pinyin syllable tokenization)
    build_fst_and_bincode(&simplified_tables, &out_dir.join("simplified"), "original")?;

    // Build traditional (pinyin syllable tokenization, convert zhuyin keys to pinyin)
    build_fst_and_bincode(&traditional_tables, &out_dir.join("traditional"), "pinyin")?;

    // Build zhuyin (character tokenization, keep zhuyin/bopomofo keys)
    build_fst_and_bincode(
        &zhuyin_tables,
        &out_dir.join("zhuyin_traditional"),
        "zhuyin",
    )?;

    // Build emoji (pinyin syllable tokenization, original keys)
    if data_dir.join("emoji.table").exists() {
        println!("Building emoji lexicon...");
        build_fst_and_bincode(&emoji_tables, &out_dir.join("emoji"), "original")?;
    } else {
        println!("Skipping emoji (emoji.table not found)");
    }

    // No global placeholders here; each dataset has its own ngram/interp artifacts.

    Ok(())
}

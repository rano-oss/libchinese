//! Converts raw .table files directly to mmap-ready runtime format.
//!
//! Output files (per variant directory):
//!   - lexicon.fst (FST mapping keys to monotonic indices)
//!   - lexicon.dat (flat binary payloads, LXPD format)
//!
//! Variants built:
//!   - simplified: gb_char + merged + opengram + punct tables (pinyin keys)
//!   - traditional: tsi.table with zhuyin keys converted to pinyin
//!   - zhuyin_traditional: tsi.table with raw zhuyin keys
//!   - emoji: emoji.table (pinyin keywords)

use anyhow::Result;
use fst::MapBuilder;
use std::collections::BTreeMap;
use std::fs::{create_dir_all, File};
use std::io::{BufRead, BufReader};
use std::path::Path;
use tools_common::LexEntry;

fn parse_table_line(line: &str) -> Option<(String, String, u32, u32)> {
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

fn build_lexicon<P: AsRef<Path>>(
    table_paths: &[(&str, P)],
    out_dir: &Path,
    key_type: &str,
) -> Result<()> {
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
                let actual_key = if name == &"tsi" {
                    match key_type {
                        "pinyin" => {
                            let raw = convert_zhuyin_key_to_pinyin(&key);
                            let parts: Vec<String> =
                                raw.split('\'').map(normalize_pinyin_syllable).collect();
                            parts.join("'")
                        }
                        "zhuyin" => key.clone(),
                        _ => key.clone(),
                    }
                } else {
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

    create_dir_all(out_dir)?;

    // Build FST
    let fst_path = out_dir.join("lexicon.fst");
    let mut w = File::create(&fst_path)?;
    let mut map_builder = MapBuilder::new(&mut w)?;

    let mut payloads: Vec<&Vec<LexEntry>> = Vec::with_capacity(grouped.len());

    for (i, (k, v)) in grouped.iter().enumerate() {
        map_builder.insert(k, i as u64)?;
        payloads.push(v);
    }
    map_builder.finish()?;

    // Build lexicon.dat directly (LXPD format)
    let num_keys = payloads.len() as u32;
    let mut key_offsets: Vec<u32> = Vec::with_capacity(payloads.len() + 1);
    let mut entries_data: Vec<u8> = Vec::new();

    for entries in &payloads {
        key_offsets.push(entries_data.len() as u32);
        for entry in entries.iter() {
            let bytes = entry.utf8.as_bytes();
            entries_data.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            entries_data.extend_from_slice(bytes);
            entries_data.extend_from_slice(&entry.freq.to_le_bytes());
        }
    }
    key_offsets.push(entries_data.len() as u32);

    let dat_path = out_dir.join("lexicon.dat");
    tools_common::mmap_write::write_lexicon_dat(&dat_path, num_keys, &key_offsets, &entries_data)?;

    println!(
        "  {} keys -> {} + {}",
        num_keys,
        fst_path.display(),
        dat_path.display()
    );

    Ok(())
}

// === Zhuyin -> Pinyin conversion ===

fn strip_zhuyin_tone(s: &str) -> String {
    s.chars()
        .filter(|c| {
            !matches!(
                *c,
                '\u{02CA}' | '\u{02C7}' | '\u{02CB}' | '\u{02D9}' | '\u{0304}'
            )
        })
        .collect()
}

fn zhuyin_char_to_pinyin_fragment(ch: char) -> Option<&'static str> {
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
        _ => None,
    }
}

fn convert_zhuyin_syllable_to_pinyin(syll: &str) -> String {
    let cleaned = strip_zhuyin_tone(syll);
    let mut out = String::new();
    for ch in cleaned.chars() {
        if let Some(frag) = zhuyin_char_to_pinyin_fragment(ch) {
            out.push_str(frag);
        }
    }

    if out.starts_with('i') && out.len() >= 2 {
        let rest = &out[1..];
        if rest.starts_with('a')
            || rest.starts_with('o')
            || rest.starts_with('e')
            || rest.starts_with('u')
            || rest.starts_with('i')
        {
            out = format!("y{}", rest);
        }
    }
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
    if out.starts_with('v') {
        let rest = &out[1..];
        out = format!("yu{}", rest);
    }

    out
}

fn convert_zhuyin_key_to_pinyin(key: &str) -> String {
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
    s
}

fn main() -> Result<()> {
    let data_dir = Path::new("data");
    let zhuyin_dir = Path::new("data/zhuyin");
    let out_dir = Path::new("data/converted");

    // 1) Simplified pinyin
    let simplified_tables = [
        ("gb_char", data_dir.join("gb_char.table")),
        ("merged", data_dir.join("merged.table")),
        ("opengram", data_dir.join("opengram.table")),
        ("punct", data_dir.join("punct.table")),
    ];
    println!("Building simplified lexicon...");
    build_lexicon(&simplified_tables, &out_dir.join("simplified"), "original")?;

    // 2) Traditional pinyin (zhuyin keys -> pinyin)
    let traditional_tables = [("tsi", zhuyin_dir.join("tsi.table"))];
    println!("Building traditional lexicon...");
    build_lexicon(&traditional_tables, &out_dir.join("traditional"), "pinyin")?;

    // 3) Zhuyin traditional (raw zhuyin keys)
    let zhuyin_tables = [("tsi", zhuyin_dir.join("tsi.table"))];
    println!("Building zhuyin_traditional lexicon...");
    build_lexicon(
        &zhuyin_tables,
        &out_dir.join("zhuyin_traditional"),
        "zhuyin",
    )?;

    // 4) Emoji
    if data_dir.join("emoji.table").exists() {
        let emoji_tables = [("emoji", data_dir.join("emoji.table"))];
        println!("Building emoji lexicon...");
        build_lexicon(&emoji_tables, &out_dir.join("emoji"), "original")?;
    } else {
        println!("Skipping emoji (emoji.table not found)");
    }

    println!("\nDone!");
    Ok(())
}

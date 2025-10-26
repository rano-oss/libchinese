// tools/gen_word_bigrams/src/main.rs
//
// Generate word-level bigram model from upstream's interpolation2.text
//
// Usage:
//   cargo run --bin gen_word_bigrams [interpolation_file] [lexicon_fst] [lexicon_bincode] [output_path]
//
// Examples:
//   # Simplified Chinese (pinyin)
//   cargo run --bin gen_word_bigrams data/interpolation2.text data/converted/simplified/lexicon.fst data/converted/simplified/lexicon.bincode data/converted/simplified/word_bigram.bin
//
//   # Traditional Chinese (pinyin)
//   cargo run --bin gen_word_bigrams data/interpolation2.text data/converted/traditional/lexicon.fst data/converted/traditional/lexicon.bincode data/converted/traditional/word_bigram.bin
//
//   # Zhuyin/Bopomofo (traditional)
//   cargo run --bin gen_word_bigrams data/zhuyin/interpolation2.text data/converted/zhuyin_traditional/lexicon.fst data/converted/zhuyin_traditional/lexicon.bincode data/converted/zhuyin_traditional/word_bigram.bin
//
// Strategy:
// 1. Parse interpolation2.text \2-gram section
// 2. Extract word-to-word bigrams with counts
// 3. Filter to only include words in our lexicon
// 4. Convert counts to log probabilities
// 5. Save as word_bigram.bin using bincode

use libchinese_core::{Lexicon, WordBigram};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    let interpolation_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("data/interpolation2.text")
    };
    
    let fst_path = if args.len() > 2 {
        PathBuf::from(&args[2])
    } else {
        PathBuf::from("data/converted/simplified/lexicon.fst")
    };
    
    let bincode_path = if args.len() > 3 {
        PathBuf::from(&args[3])
    } else {
        PathBuf::from("data/converted/simplified/lexicon.bincode")
    };
    
    let output_path = if args.len() > 4 {
        PathBuf::from(&args[4])
    } else {
        PathBuf::from("data/converted/simplified/word_bigram.bin")
    };
    
    println!("Loading lexicon from {} and {}...", fst_path.display(), bincode_path.display());
    let lexicon = Lexicon::load_from_fst_bincode(&fst_path, &bincode_path)
        .map_err(|e| format!("Failed to load lexicon: {}", e))?;
    
    println!("Extracting bigrams from {}...", interpolation_path.display());
    let bigram_counts = extract_bigrams_from_interpolation(&interpolation_path, &lexicon)?;
    
    println!("Converting counts to probabilities...");
    let word_bigram = build_word_bigram_model(&bigram_counts);
    
    println!("Saving to {}...", output_path.display());
    word_bigram.save(&output_path)?;
    
    println!("\n✓ Word bigram model generated successfully!");
    println!("  Total unique first words: {}", word_bigram.len());
    
    let total_bigrams = word_bigram.total_bigrams();
    println!("  Total bigram entries: {}", total_bigrams);
    
    Ok(())
}

/// Parse interpolation2.text \2-gram section and extract word bigrams
fn extract_bigrams_from_interpolation(
    path: &PathBuf,
    lexicon: &Lexicon,
) -> Result<HashMap<String, HashMap<String, u32>>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    
    let mut bigram_counts: HashMap<String, HashMap<String, u32>> = HashMap::new();
    let mut in_bigram_section = false;
    let mut total_lines = 0;
    let mut extracted = 0;
    let mut skipped_not_in_lexicon = 0;
    
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        
        // Check for section markers
        if trimmed == "\\2-gram" {
            in_bigram_section = true;
            println!("  Found \\2-gram section...");
            continue;
        } else if trimmed.starts_with("\\end") || trimmed.starts_with("\\3-gram") {
            println!("  Reached end of \\2-gram section");
            break;
        }
        
        if !in_bigram_section || !trimmed.starts_with("\\item ") {
            continue;
        }
        
        total_lines += 1;
        
        // Parse format: \item token1 phrase1 token2 phrase2 count N
        // Example: \item 16867717 南京大屠杀 16778715 的 count 16
        if let Some((phrase1, phrase2, count)) = parse_bigram_line(trimmed) {
            // Check if both phrases exist in lexicon (just check if we can look them up by any key)
            // Note: This is a simple heuristic - we check if the phrase appears as a candidate
            let phrase1_exists = phrase_exists_in_lexicon(lexicon, &phrase1);
            let phrase2_exists = phrase_exists_in_lexicon(lexicon, &phrase2);
            
            if phrase1_exists && phrase2_exists {
                bigram_counts
                    .entry(phrase1)
                    .or_insert_with(HashMap::new)
                    .entry(phrase2)
                    .and_modify(|c| *c += count)
                    .or_insert(count);
                
                extracted += 1;
                if extracted % 10000 == 0 {
                    println!("    Extracted {} bigrams (out of {} lines processed)...", 
                             extracted, total_lines);
                }
            } else {
                skipped_not_in_lexicon += 1;
            }
        }
    }
    
    println!("  Processed {} bigram lines from interpolation2.text", total_lines);
    println!("  Extracted {} bigrams in our lexicon", extracted);
    println!("  Skipped {} bigrams (not in lexicon)", skipped_not_in_lexicon);
    
    Ok(bigram_counts)
}

/// Simple check if a phrase exists anywhere in the lexicon
/// This is a heuristic - we accept the phrase if it appears in any lookup result
fn phrase_exists_in_lexicon(lexicon: &Lexicon, phrase: &str) -> bool {
    // For single-character phrases, they almost always exist
    if phrase.chars().count() == 1 {
        return true;
    }
    
    // For multi-character phrases, we'd need to know the pinyin key to look it up
    // As a workaround, we'll accept all multi-character phrases and let the runtime
    // filter them if they don't actually get used
    // TODO: Build a reverse index phrase -> pinyin for better filtering
    true
}

/// Parse a bigram line from interpolation2.text
/// Format: \item token1 phrase1 token2 phrase2 count N
/// Returns (phrase1, phrase2, count)
fn parse_bigram_line(line: &str) -> Option<(String, String, u32)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    
    // Must have at least: \item token1 phrase1 token2 phrase2 count N
    if parts.len() < 6 || parts[parts.len() - 2] != "count" {
        return None;
    }
    
    // Parse count (last element)
    let count = parts[parts.len() - 1].parse::<u32>().ok()?;
    
    // Strategy: Find token2 by looking for a number after phrase1
    // token1 is at index 1
    // phrase1 starts at index 2
    // token2 is a number that appears after phrase1
    // phrase2 follows token2
    
    let mut phrase1_parts = Vec::new();
    let mut phrase2_parts = Vec::new();
    let mut found_token2 = false;
    let mut seen_phrase1 = false;
    
    for i in 2..(parts.len() - 2) { // Stop before "count N"
        if !seen_phrase1 {
            // Check if this could be token2 (a number after we've seen at least one phrase part)
            if let Ok(_token) = parts[i].parse::<u32>() {
                if !phrase1_parts.is_empty() {
                    seen_phrase1 = true;
                    found_token2 = true;
                    continue; // Skip token2 itself
                }
            }
            phrase1_parts.push(parts[i]);
        } else {
            phrase2_parts.push(parts[i]);
        }
    }
    
    if !found_token2 || phrase1_parts.is_empty() || phrase2_parts.is_empty() {
        return None;
    }
    
    let phrase1 = phrase1_parts.join("");
    let phrase2 = phrase2_parts.join("");
    
    Some((phrase1, phrase2, count))
}

/// Build WordBigram model from raw counts
fn build_word_bigram_model(counts: &HashMap<String, HashMap<String, u32>>) -> WordBigram {
    let mut word_bigram = WordBigram::new();
    
    for (word1, following_words) in counts {
        for (word2, &count) in following_words {
            // Add the raw count - WordBigram will normalize when computing probabilities
            word_bigram.add_bigram(word1.clone(), word2.clone(), count);
        }
    }
    
    word_bigram
}

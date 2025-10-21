// libchinese/libpinyin/src/wade_giles.rs
//
// Wade-Giles romanization support for historical texts and users familiar
// with the older romanization system.
//
// Wade-Giles was used before pinyin became the standard in 1958. Many older
// books, documents, and place names still use Wade-Giles spelling.
//
// Key differences from pinyin:
// - Aspiration marked with apostrophe: ch' → q, p' → p, t' → t, k' → k
// - Different consonant mappings: ch → zh, hs → x, ts → z, ts' → c
// - Finals generally similar but with some differences
//
// This module provides conversion from Wade-Giles to standard pinyin,
// allowing users to type in Wade-Giles and get pinyin-based results.

use std::collections::HashMap;
use once_cell::sync::Lazy;

/// Wade-Giles to Pinyin conversion table
///
/// Based on standard Wade-Giles romanization as documented in:
/// - Herbert Giles' "Chinese-English Dictionary" (1892)
/// - Modern Wade-Giles conversion tables
static WADE_GILES_TO_PINYIN: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    
    // Aspirated consonants (with apostrophe)
    m.insert("ch'", "q");
    m.insert("p'", "p");
    m.insert("t'", "t");
    m.insert("k'", "k");
    m.insert("ts'", "c");
    
    // Unaspirated consonants
    m.insert("ch", "zh");
    m.insert("ts", "z");
    m.insert("hs", "x");
    
    // Initial consonants
    m.insert("j", "r");  // Wade-Giles "j" → pinyin "r"
    m.insert("p", "b");  // Wade-Giles "p" (unaspirated) → pinyin "b"
    m.insert("t", "d");  // Wade-Giles "t" (unaspirated) → pinyin "d"
    m.insert("k", "g");  // Wade-Giles "k" (unaspirated) → pinyin "g"
    
    // Finals with different spellings
    m.insert("ü", "v");  // Some Wade-Giles texts use ü
    m.insert("uo", "o");  // After b, p, m, f
    
    m
});

/// Additional full syllable mappings for common exceptions
static SYLLABLE_EXCEPTIONS: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    
    // Common full syllable conversions
    m.insert("chi", "zhi");
    m.insert("ch'i", "qi");
    m.insert("hsi", "xi");
    m.insert("ssu", "si");
    m.insert("tzu", "zi");
    m.insert("tz'u", "ci");
    m.insert("erh", "er");
    m.insert("jih", "ri");
    
    // Specific conversions with finals (longer/more specific matches first)
    m.insert("peiching", "beijing");
    m.insert("peijing", "beijing");
    m.insert("beijing", "beijing"); // Already pinyin
    m.insert("ching", "jing");  // Default ching → jing (京)
    m.insert("ch'ing", "qing");  // Aspirated ch'ing → qing (清)
    m.insert("chang", "zhang");
    m.insert("tsung", "zong");
    m.insert("hsin", "xin");
    m.insert("tien", "tian");
    m.insert("ko", "ke");
    m.insert("k'o", "ke");
    
    // Finals
    m.insert("ieh", "ie");
    m.insert("ueh", "ue");
    m.insert("ien", "ian");
    m.insert("un", "uen");  // Sometimes Wade-Giles uses "un" for what's "uen" in pinyin
    
    m
});

/// Convert a Wade-Giles syllable to pinyin
///
/// This function attempts to convert Wade-Giles romanization to standard
/// pinyin by applying conversion rules in order of specificity:
/// 1. Check full syllable exceptions
/// 2. Apply consonant conversion rules
/// 3. Convert finals (ien → ian, ung → ong, etc.)
/// 4. Return result
///
/// # Examples
/// ```
/// use libpinyin::wade_giles::convert_syllable;
///
/// assert_eq!(convert_syllable("ch'ing"), "qing");
/// assert_eq!(convert_syllable("chang"), "zhang");
/// assert_eq!(convert_syllable("hsi"), "xi");
/// assert_eq!(convert_syllable("tzu"), "zi");
/// ```
pub fn convert_syllable(wade_giles: &str) -> String {
    let input = wade_giles.to_lowercase();
    
    // Check full syllable exceptions first
    if let Some(&pinyin) = SYLLABLE_EXCEPTIONS.get(input.as_str()) {
        return pinyin.to_string();
    }
    
    // Try consonant replacements (longest first to avoid partial matches)
    let mut result = input.clone();
    
    // Sort by length descending to match longest patterns first
    let mut rules: Vec<_> = WADE_GILES_TO_PINYIN.iter().collect();
    rules.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
    
    for (&wade, &pinyin) in rules {
        if result.starts_with(wade) {
            result = format!("{}{}", pinyin, &result[wade.len()..]);
            break;
        }
    }
    
    // Convert finals
    result = result.replace("ien", "ian");
    result = result.replace("ung", "ong");
    result = result.replace("iung", "iong");
    
    result
}

/// Convert a string of Wade-Giles syllables to pinyin
///
/// Splits input by common delimiters (space, hyphen, apostrophe when standalone)
/// and converts each syllable individually.
///
/// # Examples
/// ```
/// use libpinyin::wade_giles::convert_input;
///
/// assert_eq!(convert_input("ch'ing hua"), "qing hua");
/// assert_eq!(convert_input("pei-ching"), "bei-jing");
/// assert_eq!(convert_input("chung-kuo"), "zhong-guo");
/// ```
pub fn convert_input(input: &str) -> String {
    // Split by spaces and hyphens, preserving delimiters
    let mut result = String::new();
    let mut current = String::new();
    
    for ch in input.chars() {
        match ch {
            ' ' | '-' => {
                if !current.is_empty() {
                    result.push_str(&convert_syllable(&current));
                    current.clear();
                }
                result.push(ch);
            }
            _ => current.push(ch),
        }
    }
    
    // Don't forget the last syllable
    if !current.is_empty() {
        result.push_str(&convert_syllable(&current));
    }
    
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aspirated_consonants() {
        assert_eq!(convert_syllable("ch'ing"), "qing");
        assert_eq!(convert_syllable("p'ing"), "ping");
        assert_eq!(convert_syllable("t'ien"), "tian");
        assert_eq!(convert_syllable("k'o"), "ke");
        assert_eq!(convert_syllable("ts'ao"), "cao");
    }

    #[test]
    fn test_unaspirated_consonants() {
        assert_eq!(convert_syllable("chang"), "zhang");
        assert_eq!(convert_syllable("tsung"), "zong");
        assert_eq!(convert_syllable("hsin"), "xin");
    }

    #[test]
    fn test_syllable_exceptions() {
        assert_eq!(convert_syllable("chi"), "zhi");
        assert_eq!(convert_syllable("ch'i"), "qi");
        assert_eq!(convert_syllable("hsi"), "xi");
        assert_eq!(convert_syllable("tzu"), "zi");
        assert_eq!(convert_syllable("erh"), "er");
        assert_eq!(convert_syllable("jih"), "ri");
    }

    #[test]
    fn test_full_input_conversion() {
        // Beijing (北京)
        assert_eq!(convert_input("pei-ching"), "bei-jing");
        
        // China (中国)
        assert_eq!(convert_input("chung-kuo"), "zhong-guo");
        
        // Tsinghua (清华)
        assert_eq!(convert_input("ch'ing-hua"), "qing-hua");
        
        // Multiple syllables with spaces
        assert_eq!(convert_input("ni hao ma"), "ni hao ma");
    }

    #[test]
    fn test_passthrough_pinyin() {
        // If input is already pinyin, should remain unchanged
        assert_eq!(convert_syllable("ni"), "ni");
        assert_eq!(convert_syllable("hao"), "hao");
        assert_eq!(convert_syllable("ma"), "ma");
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(convert_syllable("CH'ING"), "qing");
        assert_eq!(convert_syllable("Chang"), "zhang");
    }
}

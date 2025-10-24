//! Double Pinyin (Shuangpin 双拼) input method support
//!
//! Double pinyin is an alternative input method where each syllable is represented
//! by exactly 2 keys (or 1 key for special syllables like 'a', 'e', 'o').
//! This makes typing faster and more efficient than full pinyin.
//!
//! ## Supported Schemes
//!
//! 1. **Microsoft Shuangpin** (微软双拼) - Most popular
//! 2. **ZiRanMa** (自然码) - Natural input method
//! 3. **ZiGuang** (紫光) - Purple light scheme
//! 4. **ABC** - ABC input method scheme
//! 5. **XiaoHe** (小鹤) - Little crane scheme
//!
//! ## How it works
//!
//! Each full pinyin syllable is represented by 2 keys:
//! - First key: Initial consonant (shengmu 声母)
//! - Second key: Final (yunmu 韵母)
//!
//! Example (Microsoft scheme):
//! - "zh" + "ang" = "zhang" → typed as "uh" (u=zh, h=ang)
//! - "sh" + "i" = "shi" → typed as "ui" (u=sh, i=i)
//!
//! ## References
//! - Upstream libpinyin: `src/storage/pinyin_parser2.cpp` DoublePinyinParser2
//! - Microsoft scheme: Most widely used in Windows
//! - ZiRanMa: Popular alternative with different mappings

use std::collections::HashMap;

/// Double pinyin schemes supported by the parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoublePinyinScheme {
    /// Microsoft Shuangpin (微软双拼) - Most popular scheme
    Microsoft,
    /// ZiRanMa (自然码) - Natural input method
    ZiRanMa,
    /// ZiGuang (紫光) - Purple light scheme
    ZiGuang,
    /// ABC input method scheme
    ABC,
    /// XiaoHe (小鹤) - Little crane scheme  
    XiaoHe,
    /// PinYin++ scheme
    PinYinPlusPlus,
}

/// Mapping tables for a double pinyin scheme.
/// Maps 2-key input to full pinyin syllables.
#[derive(Debug, Clone)]
pub struct DoublePinyinSchemeData {
    /// Scheme name
    pub name: &'static str,

    /// Shengmu (initial consonant) mappings: key -> full initial
    /// e.g., 'u' -> "sh" in Microsoft scheme
    pub shengmu: HashMap<char, &'static str>,

    /// Yunmu (final) mappings: key -> full final
    /// e.g., 'h' -> "ang" in Microsoft scheme
    pub yunmu: HashMap<char, &'static str>,

    /// Special single-key syllables (a, e, o, etc.)
    /// These are typed with the syllable letter twice: aa, ee, oo
    pub special: HashMap<char, &'static str>,
}

impl DoublePinyinScheme {
    /// Get the mapping data for this scheme.
    pub fn data(&self) -> DoublePinyinSchemeData {
        match self {
            DoublePinyinScheme::Microsoft => microsoft_scheme(),
            DoublePinyinScheme::ZiRanMa => ziranma_scheme(),
            DoublePinyinScheme::ZiGuang => ziguang_scheme(),
            DoublePinyinScheme::ABC => abc_scheme(),
            DoublePinyinScheme::XiaoHe => xiaohe_scheme(),
            DoublePinyinScheme::PinYinPlusPlus => pinyinpp_scheme(),
        }
    }
}

/// Get scheme data for a given scheme (convenience function).
pub fn get_scheme_data(scheme: &DoublePinyinScheme) -> DoublePinyinSchemeData {
    scheme.data()
}

/// Microsoft Shuangpin (微软双拼) scheme mappings
/// This is the most popular double pinyin scheme in China.
fn microsoft_scheme() -> DoublePinyinSchemeData {
    let mut shengmu = HashMap::new();
    let mut yunmu = HashMap::new();
    let mut special = HashMap::new();

    // Shengmu (initials): most are unchanged
    // Special multi-letter initials:
    shengmu.insert('u', "sh"); // u -> sh
    shengmu.insert('i', "ch"); // i -> ch
    shengmu.insert('v', "zh"); // v -> zh

    // Yunmu (finals) - Microsoft scheme
    yunmu.insert('a', "a");
    yunmu.insert('o', "o");
    yunmu.insert('e', "e");
    yunmu.insert('i', "i");
    yunmu.insert('u', "u");
    yunmu.insert('v', "v"); // ü

    yunmu.insert('b', "ou");
    yunmu.insert('c', "iao");
    yunmu.insert('d', "uang");
    yunmu.insert('f', "en");
    yunmu.insert('g', "eng");
    yunmu.insert('h', "ang");
    yunmu.insert('j', "an");
    yunmu.insert('k', "ao");
    yunmu.insert('l', "ai");
    yunmu.insert('m', "ian");
    yunmu.insert('n', "in");
    yunmu.insert('p', "un");
    yunmu.insert('q', "iu");
    yunmu.insert('r', "uan");
    yunmu.insert('s', "ong");
    yunmu.insert('t', "ue");
    yunmu.insert('w', "ia");
    yunmu.insert('x', "ie");
    yunmu.insert('y', "uai");
    yunmu.insert('z', "ei");

    // Special single vowels - type the letter twice
    special.insert('a', "a");
    special.insert('e', "e");
    special.insert('o', "o");

    DoublePinyinSchemeData {
        name: "Microsoft",
        shengmu,
        yunmu,
        special,
    }
}

/// ZiRanMa (自然码) scheme mappings
/// Popular alternative scheme with different key mappings.
fn ziranma_scheme() -> DoublePinyinSchemeData {
    let mut shengmu = HashMap::new();
    let mut yunmu = HashMap::new();
    let mut special = HashMap::new();

    // ZiRanMa shengmu
    shengmu.insert('u', "sh");
    shengmu.insert('i', "ch");
    shengmu.insert('v', "zh");

    // ZiRanMa yunmu (different from Microsoft)
    yunmu.insert('a', "a");
    yunmu.insert('o', "o");
    yunmu.insert('e', "e");
    yunmu.insert('i', "i");
    yunmu.insert('u', "u");
    yunmu.insert('v', "v");

    yunmu.insert('b', "ia");
    yunmu.insert('c', "ua");
    yunmu.insert('d', "ao");
    yunmu.insert('f', "an");
    yunmu.insert('g', "ang");
    yunmu.insert('h', "iang");
    yunmu.insert('j', "ian");
    yunmu.insert('k', "uai");
    yunmu.insert('l', "uan");
    yunmu.insert('m', "in");
    yunmu.insert('n', "iao");
    yunmu.insert('p', "ie");
    yunmu.insert('q', "iu");
    yunmu.insert('r', "er");
    yunmu.insert('s', "ong");
    yunmu.insert('t', "ue");
    yunmu.insert('w', "en");
    yunmu.insert('x', "uang");
    yunmu.insert('y', "ing");
    yunmu.insert('z', "ou");

    special.insert('a', "a");
    special.insert('e', "e");
    special.insert('o', "o");

    DoublePinyinSchemeData {
        name: "ZiRanMa",
        shengmu,
        yunmu,
        special,
    }
}

/// ZiGuang (紫光) scheme
/// Used in ZiGuang Pinyin input method, popular in early 2000s
fn ziguang_scheme() -> DoublePinyinSchemeData {
    let mut shengmu = HashMap::new();
    let mut yunmu = HashMap::new();
    let mut special = HashMap::new();

    // Shengmu (same multi-letter initials as Microsoft)
    shengmu.insert('u', "sh");
    shengmu.insert('i', "ch");
    shengmu.insert('v', "zh");

    // Yunmu - ZiGuang has its own mappings, similar to Microsoft but with differences
    yunmu.insert('a', "a");
    yunmu.insert('o', "o");
    yunmu.insert('e', "e");
    yunmu.insert('i', "i");
    yunmu.insert('u', "u");
    yunmu.insert('v', "v");

    yunmu.insert('b', "ia"); // Different from Microsoft (ou)
    yunmu.insert('c', "uan"); // Different from Microsoft (iao)
    yunmu.insert('d', "ao"); // Different from Microsoft (uang)
    yunmu.insert('f', "en");
    yunmu.insert('g', "eng");
    yunmu.insert('h', "ang");
    yunmu.insert('j', "an");
    yunmu.insert('k', "uai"); // Different from Microsoft (ao)
    yunmu.insert('l', "ai");
    yunmu.insert('m', "ian");
    yunmu.insert('n', "in");
    yunmu.insert('p', "iao"); // Different from Microsoft (un)
    yunmu.insert('q', "iu");
    yunmu.insert('r', "er"); // Different from Microsoft (uan)
    yunmu.insert('s', "ong");
    yunmu.insert('t', "ue");
    yunmu.insert('w', "ei"); // Different from Microsoft (ia)
    yunmu.insert('x', "ie");
    yunmu.insert('y', "un"); // Different from Microsoft (uai)
    yunmu.insert('z', "ou"); // Different from Microsoft (ei)

    special.insert('a', "a");
    special.insert('e', "e");
    special.insert('o', "o");

    DoublePinyinSchemeData {
        name: "ZiGuang",
        shengmu,
        yunmu,
        special,
    }
}

/// ABC scheme
/// Used in ABC input method, one of the earliest Chinese input methods
fn abc_scheme() -> DoublePinyinSchemeData {
    let mut shengmu = HashMap::new();
    let mut yunmu = HashMap::new();
    let mut special = HashMap::new();

    // ABC uses different shengmu mappings
    shengmu.insert('a', "zh"); // Different from others
    shengmu.insert('e', "ch"); // Different from others
    shengmu.insert('v', "sh"); // Different from others

    // Yunmu - ABC scheme
    yunmu.insert('a', "a");
    yunmu.insert('o', "o");
    yunmu.insert('e', "e");
    yunmu.insert('i', "i");
    yunmu.insert('u', "u");
    yunmu.insert('v', "v");

    yunmu.insert('b', "ou");
    yunmu.insert('c', "in"); // Different
    yunmu.insert('d', "ia"); // Different
    yunmu.insert('f', "en");
    yunmu.insert('g', "eng");
    yunmu.insert('h', "ang");
    yunmu.insert('j', "an");
    yunmu.insert('k', "ao");
    yunmu.insert('l', "ai");
    yunmu.insert('m', "ian");
    yunmu.insert('n', "iao"); // Different
    yunmu.insert('p', "ie"); // Different
    yunmu.insert('q', "iu");
    yunmu.insert('r', "uan");
    yunmu.insert('s', "ong");
    yunmu.insert('t', "ue");
    yunmu.insert('w', "ei"); // Different
    yunmu.insert('x', "uai"); // Different
    yunmu.insert('y', "ing"); // Different
    yunmu.insert('z', "un"); // Different

    special.insert('a', "a");
    special.insert('e', "e");
    special.insert('o', "o");

    DoublePinyinSchemeData {
        name: "ABC",
        shengmu,
        yunmu,
        special,
    }
}

/// XiaoHe (小鹤) scheme
/// Popular modern scheme with phonetic-based mappings
fn xiaohe_scheme() -> DoublePinyinSchemeData {
    let mut shengmu = HashMap::new();
    let mut yunmu = HashMap::new();
    let mut special = HashMap::new();

    // XiaoHe uses same multi-letter initials as Microsoft
    shengmu.insert('u', "sh");
    shengmu.insert('i', "ch");
    shengmu.insert('v', "zh");

    // Yunmu - XiaoHe scheme (phonetic-based, easier to remember)
    yunmu.insert('a', "a");
    yunmu.insert('o', "o");
    yunmu.insert('e', "e");
    yunmu.insert('i', "i");
    yunmu.insert('u', "u");
    yunmu.insert('v', "v");

    yunmu.insert('b', "ou");
    yunmu.insert('c', "iao");
    yunmu.insert('d', "uang");
    yunmu.insert('f', "en");
    yunmu.insert('g', "eng");
    yunmu.insert('h', "ang");
    yunmu.insert('j', "an");
    yunmu.insert('k', "ao");
    yunmu.insert('l', "ai");
    yunmu.insert('m', "ian");
    yunmu.insert('n', "in");
    yunmu.insert('p', "un");
    yunmu.insert('q', "iu");
    yunmu.insert('r', "uan");
    yunmu.insert('s', "iong"); // Different - XiaoHe uses iong instead of ong
    yunmu.insert('t', "ue");
    yunmu.insert('w', "ei"); // Different
    yunmu.insert('x', "ie");
    yunmu.insert('y', "uai");
    yunmu.insert('z', "ou"); // Different - actually maps to 'ou' differently

    special.insert('a', "a");
    special.insert('e', "e");
    special.insert('o', "o");

    DoublePinyinSchemeData {
        name: "XiaoHe",
        shengmu,
        yunmu,
        special,
    }
}

/// PinYin++ scheme
/// Modern scheme with optimized key positions
fn pinyinpp_scheme() -> DoublePinyinSchemeData {
    let mut shengmu = HashMap::new();
    let mut yunmu = HashMap::new();
    let mut special = HashMap::new();

    // PinYin++ uses Microsoft-style shengmu
    shengmu.insert('u', "sh");
    shengmu.insert('i', "ch");
    shengmu.insert('v', "zh");

    // Yunmu - PinYin++ optimized mappings
    yunmu.insert('a', "a");
    yunmu.insert('o', "o");
    yunmu.insert('e', "e");
    yunmu.insert('i', "i");
    yunmu.insert('u', "u");
    yunmu.insert('v', "v");

    yunmu.insert('b', "ou");
    yunmu.insert('c', "iao");
    yunmu.insert('d', "uang");
    yunmu.insert('f', "en");
    yunmu.insert('g', "eng");
    yunmu.insert('h', "ang");
    yunmu.insert('j', "an");
    yunmu.insert('k', "ao");
    yunmu.insert('l', "ai");
    yunmu.insert('m', "ian");
    yunmu.insert('n', "in");
    yunmu.insert('p', "un");
    yunmu.insert('q', "iu");
    yunmu.insert('r', "uan");
    yunmu.insert('s', "ong");
    yunmu.insert('t', "ve"); // Different - uses ve
    yunmu.insert('w', "ia");
    yunmu.insert('x', "ua"); // Different - optimized
    yunmu.insert('y', "ing"); // Different
    yunmu.insert('z', "ei");

    special.insert('a', "a");
    special.insert('e', "e");
    special.insert('o', "o");

    DoublePinyinSchemeData {
        name: "PinYin++",
        shengmu,
        yunmu,
        special,
    }
}

/// Convert 2-key double pinyin input to full pinyin syllable.
///
/// Returns the full pinyin syllable if valid, None otherwise.
pub fn double_to_full_pinyin(
    first: char,
    second: char,
    scheme: &DoublePinyinSchemeData,
) -> Option<String> {
    // Special case: single vowels typed twice (aa, ee, oo)
    if first == second {
        if let Some(&syllable) = scheme.special.get(&first) {
            return Some(syllable.to_string());
        }
    }

    // Validate input
    if !first.is_ascii_lowercase() || !second.is_ascii_lowercase() {
        return None;
    }

    // Get shengmu (initial)
    let initial: &str = if let Some(&multi_initial) = scheme.shengmu.get(&first) {
        // Multi-letter initial (sh, ch, zh)
        multi_initial
    } else if "aeiouv".contains(first) {
        // Vowels have zero initial
        ""
    } else {
        // Single letter initials - validate they're consonants
        match first {
            'b' | 'p' | 'm' | 'f' | 'd' | 't' | 'n' | 'l' | 'g' | 'k' | 'h' | 'j' | 'q' | 'x'
            | 'z' | 'c' | 's' | 'r' | 'y' | 'w' => {
                // Use a static lookup for single chars
                get_single_char_initial(first)?
            }
            _ => return None, // Invalid initial
        }
    };

    // Get yunmu (final)
    let final_part = scheme.yunmu.get(&second)?;

    // Combine initial + final
    Some(format!("{}{}", initial, final_part))
}

/// Helper to get single-char initials as static str
fn get_single_char_initial(c: char) -> Option<&'static str> {
    match c {
        'b' => Some("b"),
        'p' => Some("p"),
        'm' => Some("m"),
        'f' => Some("f"),
        'd' => Some("d"),
        't' => Some("t"),
        'n' => Some("n"),
        'l' => Some("l"),
        'g' => Some("g"),
        'k' => Some("k"),
        'h' => Some("h"),
        'j' => Some("j"),
        'q' => Some("q"),
        'x' => Some("x"),
        'z' => Some("z"),
        'c' => Some("c"),
        's' => Some("s"),
        'r' => Some("r"),
        'y' => Some("y"),
        'w' => Some("w"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microsoft_scheme_basic() {
        let scheme = DoublePinyinScheme::Microsoft.data();

        // Test basic conversion: "uh" -> "shang"
        // u=sh, h=ang
        let result = double_to_full_pinyin('u', 'h', &scheme);
        assert_eq!(result, Some("shang".to_string()));

        // Test: "ui" -> "shi"
        // u=sh, i=i
        let result = double_to_full_pinyin('u', 'i', &scheme);
        assert_eq!(result, Some("shi".to_string()));
    }

    #[test]
    fn microsoft_scheme_regular_initial() {
        let scheme = DoublePinyinScheme::Microsoft.data();

        // "bh" -> "bang" (b=b, h=ang)
        let result = double_to_full_pinyin('b', 'h', &scheme);
        assert_eq!(result, Some("bang".to_string()));
    }

    #[test]
    fn microsoft_scheme_special_vowels() {
        let scheme = DoublePinyinScheme::Microsoft.data();

        // "aa" -> "a"
        let result = double_to_full_pinyin('a', 'a', &scheme);
        assert_eq!(result, Some("a".to_string()));

        // "ee" -> "e"
        let result = double_to_full_pinyin('e', 'e', &scheme);
        assert_eq!(result, Some("e".to_string()));
    }

    #[test]
    fn ziranma_scheme_basic() {
        let scheme = DoublePinyinScheme::ZiRanMa.data();

        // ZiRanMa has different yunmu mappings
        // "ug" -> "shang" (u=sh, g=ang)
        let result = double_to_full_pinyin('u', 'g', &scheme);
        assert_eq!(result, Some("shang".to_string()));
    }
}

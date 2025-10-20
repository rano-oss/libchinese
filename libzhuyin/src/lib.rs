//! # libzhuyin
//!
//! Zhuyin/Bopomofo input method engine built on libchinese-core.

pub mod parser;
pub mod engine;

pub use parser::ZhuyinParser;
pub use engine::Engine;

/// Configuration for standard zhuyin/bopomofo fuzzy matching rules.
///
/// These rules handle common keyboard layout corrections:
/// - HSU keyboard layout: ㄓ/ㄐ, ㄔ/ㄑ, ㄕ/ㄒ, ㄛ/ㄏ, ㄜ/ㄍ, ㄢ/ㄇ, ㄣ/ㄋ, ㄤ/ㄎ, ㄥ/ㄌ
/// - ETEN26 keyboard layout: similar corrections for different key mappings
/// - Shuffle corrections: common typing errors in bopomofo order
///
/// Note: The actual symbols should be in bopomofo characters (ㄅㄆㄇㄈ...).
/// This is a placeholder - users should configure based on their keyboard scheme.
pub fn standard_fuzzy_rules() -> Vec<String> {
    let mut rules = Vec::new();
    
    // HSU keyboard layout corrections - penalty 1.5
    // These map keys that are confused on HSU layout
    let hsu = [
        "ㄓ=ㄐ:1.5", "ㄔ=ㄑ:1.5", "ㄕ=ㄒ:1.5",
        "ㄛ=ㄏ:1.5", "ㄜ=ㄍ:1.5",
        "ㄢ=ㄇ:1.5", "ㄣ=ㄋ:1.5", "ㄤ=ㄎ:1.5", "ㄥ=ㄌ:1.5",
    ];
    rules.extend(hsu.iter().map(|s| s.to_string()));
    
    // ETEN26 keyboard layout corrections - penalty 1.5
    // Similar to HSU but different key positions
    let eten26 = [
        "ㄓ=ㄐ:1.5", "ㄔ=ㄑ:1.5", "ㄕ=ㄒ:1.5",
        // Add more ETEN26-specific rules as needed
    ];
    rules.extend(eten26.iter().map(|s| s.to_string()));
    
    // Shuffle corrections - common misorderings - penalty 2.0
    // Example: typing initials and finals in wrong order
    // These would need to be expanded based on actual usage patterns
    
    rules
}

/// Create an empty fuzzy rules configuration.
///
/// Use this when you want no fuzzy matching (strict input only).
pub fn no_fuzzy_rules() -> Vec<String> {
    Vec::new()
}

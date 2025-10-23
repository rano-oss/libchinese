//! Fuzzy matching rule presets for different Zhuyin keyboard layouts.
//!
//! Different zhuyin keyboard layouts (HSU, Standard, ETEN, IBM, etc.) place
//! bopomofo symbols on different QWERTY keys, leading to different common
//! typing errors. These presets provide fuzzy matching rules tailored to
//! each layout's error patterns.
//!
//! ## Keyboard Layouts Overview
//!
//! - **HSU (許氏)**: Popular layout designed by Hsu Chung-kuo. Places finals
//!   on home row, requires fewer keystrokes. Common confusions: ㄓ/ㄐ on 'j',
//!   ㄔ/ㄑ on 'q', ㄕ/ㄒ on 'x'.
//!
//! - **Standard (標準)**: Most common layout, follows traditional bopomofo
//!   order somewhat. Common confusions: ㄣ/ㄥ (adjacent keys), ㄢ/ㄤ, ㄧㄣ/ㄧㄥ.
//!
//! - **ETEN (倚天)**: Layout from ETEN Chinese System. Similar to Standard
//!   but with some key differences. Common confusions match keyboard adjacency.
//!
//! ## Penalty Weights
//!
//! - **1.0**: Very common error (adjacent keys, same finger)
//! - **1.5**: Common error (keyboard layout confusion)
//! - **2.0**: Less common error (phonetically similar)
//! - **3.0**: Rare error (fallback matching)

/// Fuzzy matching rules for HSU (許氏) keyboard layout.
///
/// HSU layout characteristics:
/// - Designed for efficiency, fewer keystrokes
/// - Finals on home row
/// - Some initials share keys with different tones
///
/// Common typing errors:
/// - ㄓ/ㄐ confusion (both use 'j' key with different shift states)
/// - ㄔ/ㄑ confusion (both use 'q' key)
/// - ㄕ/ㄒ confusion (both use 'x' key)
/// - Finals: ㄛ/ㄏ, ㄜ/ㄍ, ㄢ/ㄇ, ㄣ/ㄋ, ㄤ/ㄎ, ㄥ/ㄌ (home row placement)
pub fn hsu_fuzzy_rules() -> Vec<String> {
    let mut rules = Vec::new();
    
    // Zhong/Qing/Xing group - same key confusions - penalty 1.5
    // These are THE most common HSU errors because they share physical keys
    rules.extend([
        "ㄓ=ㄐ:1.5", "ㄐ=ㄓ:1.5",  // j key
        "ㄔ=ㄑ:1.5", "ㄑ=ㄔ:1.5",  // q key
        "ㄕ=ㄒ:1.5", "ㄒ=ㄕ:1.5",  // x key
    ].iter().map(|s| s.to_string()));
    
    // Finals on home row - adjacent key errors - penalty 1.0
    rules.extend([
        "ㄛ=ㄏ:1.0", "ㄏ=ㄛ:1.0",  // 'h' key area
        "ㄜ=ㄍ:1.0", "ㄍ=ㄜ:1.0",  // 'g' key area
        "ㄢ=ㄇ:1.0", "ㄇ=ㄢ:1.0",  // 'm' key area
        "ㄣ=ㄋ:1.0", "ㄋ=ㄣ:1.0",  // 'n' key area
        "ㄤ=ㄎ:1.0", "ㄎ=ㄤ:1.0",  // 'k' key area
        "ㄥ=ㄌ:1.0", "ㄌ=ㄥ:1.0",  // 'l' key area
    ].iter().map(|s| s.to_string()));
    
    // Nasal finals confusion (phonetically similar) - penalty 2.0
    rules.extend([
        "ㄢ=ㄤ:2.0", "ㄤ=ㄢ:2.0",  // an/ang
        "ㄣ=ㄥ:2.0", "ㄥ=ㄣ:2.0",  // en/eng
    ].iter().map(|s| s.to_string()));
    
    rules
}

/// Fuzzy matching rules for Standard (標準) keyboard layout.
///
/// Standard layout characteristics:
/// - Traditional bopomofo key arrangement
/// - Follows phonetic grouping
/// - Most widely taught and used
///
/// Common typing errors:
/// - Adjacent key confusions (keyboard proximity)
/// - Nasal finals: ㄢ/ㄤ, ㄣ/ㄥ, ㄧㄣ/ㄧㄥ
/// - Retroflex/palatal: ㄓ/ㄐ, ㄔ/ㄑ, ㄕ/ㄒ (less common than HSU)
pub fn standard_fuzzy_rules() -> Vec<String> {
    let mut rules = Vec::new();
    
    // Nasal finals - THE most common error in Standard layout - penalty 1.0
    rules.extend([
        "ㄢ=ㄤ:1.0", "ㄤ=ㄢ:1.0",  // an/ang
        "ㄣ=ㄥ:1.0", "ㄥ=ㄣ:1.0",  // en/eng
    ].iter().map(|s| s.to_string()));
    
    // Medial + nasal combinations - penalty 1.5
    rules.extend([
        "ㄧㄢ=ㄧㄤ:1.5", "ㄧㄤ=ㄧㄢ:1.5",  // ian/iang
        "ㄧㄣ=ㄧㄥ:1.5", "ㄧㄥ=ㄧㄣ:1.5",  // in/ing
        "ㄨㄢ=ㄨㄤ:1.5", "ㄨㄤ=ㄨㄢ:1.5",  // uan/uang
        "ㄨㄣ=ㄨㄥ:1.5", "ㄨㄥ=ㄨㄣ:1.5",  // un/ong (less common but possible)
    ].iter().map(|s| s.to_string()));
    
    // Retroflex/palatal confusion (keyboard proximity) - penalty 2.0
    rules.extend([
        "ㄓ=ㄐ:2.0", "ㄐ=ㄓ:2.0",  // zh/j
        "ㄔ=ㄑ:2.0", "ㄑ=ㄔ:2.0",  // ch/q
        "ㄕ=ㄒ:2.0", "ㄒ=ㄕ:2.0",  // sh/x
    ].iter().map(|s| s.to_string()));
    
    // Sibilant confusion - penalty 2.0
    rules.extend([
        "ㄗ=ㄓ:2.0", "ㄓ=ㄗ:2.0",  // z/zh
        "ㄘ=ㄔ:2.0", "ㄔ=ㄘ:2.0",  // c/ch
        "ㄙ=ㄕ:2.0", "ㄕ=ㄙ:2.0",  // s/sh
    ].iter().map(|s| s.to_string()));
    
    // Medial confusion (less common) - penalty 2.5
    rules.extend([
        "ㄧ=ㄩ:2.5", "ㄩ=ㄧ:2.5",  // i/ü (rare but happens)
    ].iter().map(|s| s.to_string()));
    
    rules
}

/// Fuzzy matching rules for ETEN (倚天) keyboard layout.
///
/// ETEN layout characteristics:
/// - Used in ETEN Chinese System (倚天中文系統)
/// - Similar to Standard but with some differences
/// - Popular in certain regions
///
/// Common typing errors similar to Standard but with layout-specific patterns.
pub fn eten_fuzzy_rules() -> Vec<String> {
    let mut rules = Vec::new();
    
    // ETEN shares most errors with Standard layout
    // Nasal finals - penalty 1.0
    rules.extend([
        "ㄢ=ㄤ:1.0", "ㄤ=ㄢ:1.0",  // an/ang
        "ㄣ=ㄥ:1.0", "ㄥ=ㄣ:1.0",  // en/eng
    ].iter().map(|s| s.to_string()));
    
    // Medial + nasal combinations - penalty 1.5
    rules.extend([
        "ㄧㄢ=ㄧㄤ:1.5", "ㄧㄤ=ㄧㄢ:1.5",  // ian/iang
        "ㄧㄣ=ㄧㄥ:1.5", "ㄧㄥ=ㄧㄣ:1.5",  // in/ing
        "ㄨㄢ=ㄨㄤ:1.5", "ㄨㄤ=ㄨㄢ:1.5",  // uan/uang
    ].iter().map(|s| s.to_string()));
    
    // Retroflex/palatal - penalty 2.0
    rules.extend([
        "ㄓ=ㄐ:2.0", "ㄐ=ㄓ:2.0",  // zh/j
        "ㄔ=ㄑ:2.0", "ㄑ=ㄔ:2.0",  // ch/q
        "ㄕ=ㄒ:2.0", "ㄒ=ㄕ:2.0",  // sh/x
    ].iter().map(|s| s.to_string()));
    
    // ETEN-specific: Some keys may have different placements
    // Add more rules here based on actual ETEN layout research
    
    rules
}

/// Minimal fuzzy matching (disabled).
///
/// Returns an empty ruleset. Use this when you want exact matching only,
/// without any fuzzy corrections.
pub fn no_fuzzy_rules() -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hsu_rules_format() {
        let rules = hsu_fuzzy_rules();
        assert!(!rules.is_empty(), "HSU rules should not be empty");
        
        // Check format: "X=Y:penalty"
        for rule in &rules {
            let parts: Vec<&str> = rule.split(':').collect();
            assert_eq!(parts.len(), 2, "Rule should have format 'X=Y:penalty': {}", rule);
            
            let mapping_parts: Vec<&str> = parts[0].split('=').collect();
            assert_eq!(mapping_parts.len(), 2, "Mapping should have format 'X=Y': {}", rule);
            
            // Check penalty is a valid float
            let _penalty: f32 = parts[1].parse().expect(&format!("Penalty should be valid float: {}", rule));
        }
    }

    #[test]
    fn test_standard_rules_format() {
        let rules = standard_fuzzy_rules();
        assert!(!rules.is_empty(), "Standard rules should not be empty");
        
        for rule in &rules {
            assert!(rule.contains('='), "Rule should contain '=': {}", rule);
            assert!(rule.contains(':'), "Rule should contain ':': {}", rule);
        }
    }

    #[test]
    fn test_eten_rules_format() {
        let rules = eten_fuzzy_rules();
        assert!(!rules.is_empty(), "ETEN rules should not be empty");
    }

    #[test]
    fn test_no_fuzzy_rules() {
        let rules = no_fuzzy_rules();
        assert!(rules.is_empty(), "No fuzzy rules should return empty vec");
    }

    #[test]
    fn test_hsu_key_confusions() {
        let rules = hsu_fuzzy_rules();
        
        // HSU's most distinctive feature: j/q/x key sharing
        assert!(rules.iter().any(|r| r.contains("ㄓ=ㄐ")), "HSU should have ㄓ/ㄐ confusion");
        assert!(rules.iter().any(|r| r.contains("ㄔ=ㄑ")), "HSU should have ㄔ/ㄑ confusion");
        assert!(rules.iter().any(|r| r.contains("ㄕ=ㄒ")), "HSU should have ㄕ/ㄒ confusion");
    }

    #[test]
    fn test_standard_nasal_confusion() {
        let rules = standard_fuzzy_rules();
        
        // Standard's most common error: nasal finals
        assert!(rules.iter().any(|r| r.contains("ㄢ=ㄤ")), "Standard should have ㄢ/ㄤ confusion");
        assert!(rules.iter().any(|r| r.contains("ㄣ=ㄥ")), "Standard should have ㄣ/ㄥ confusion");
    }

    #[test]
    fn test_bidirectional_rules() {
        // HSU j-key confusion should work both ways
        let rules = hsu_fuzzy_rules();
        assert!(rules.iter().any(|r| r.starts_with("ㄓ=ㄐ")), "Should have ㄓ→ㄐ");
        assert!(rules.iter().any(|r| r.starts_with("ㄐ=ㄓ")), "Should have ㄐ→ㄓ");
    }
}

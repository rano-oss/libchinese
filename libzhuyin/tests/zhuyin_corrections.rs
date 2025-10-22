//! Tests for Zhuyin corrections implementation
//!
//! Tests all 4 zhuyin correction features:
//! - ZHUYIN_CORRECT_SHUFFLE: medial/final order corrections
//! - ZHUYIN_CORRECT_HSU: HSU keyboard layout corrections
//! - ZHUYIN_CORRECT_ETEN26: ETEN26 keyboard layout corrections
//! - ZHUYIN_INCOMPLETE: partial syllable matching

use libzhuyin::parser::{ZhuyinParser, ZhuyinSyllable};

#[test]
fn zhuyin_correct_shuffle_ui_medials() {
    // Test ㄨㄟ <-> ㄩㄟ correction (u vs ü medials with finals)
    let rules = libzhuyin::standard_fuzzy_rules();
    let parser = ZhuyinParser::new(rules, &["ㄉㄨㄟ", "ㄉㄩㄟ"]);
    
    // Test that corrections are generated
    let corrections = parser.apply_corrections("ㄉㄨㄟ");
    assert!(corrections.contains(&"ㄉㄩㄟ".to_string()), "Should suggest ㄩㄟ variant");
    
    let corrections2 = parser.apply_corrections("ㄉㄩㄟ");
    assert!(corrections2.contains(&"ㄉㄨㄟ".to_string()), "Should suggest ㄨㄟ variant");
}

#[test]
fn zhuyin_correct_shuffle_un_finals() {
    // Test ㄨㄣ <-> ㄩㄣ correction
    let rules = libzhuyin::standard_fuzzy_rules();
    let parser = ZhuyinParser::new(rules, &["ㄓㄨㄣ", "ㄓㄩㄣ"]);
    
    let corrections = parser.apply_corrections("ㄓㄨㄣ");
    assert!(corrections.contains(&"ㄓㄩㄣ".to_string()), "Should suggest ㄩㄣ variant");
    
    let corrections2 = parser.apply_corrections("ㄓㄩㄣ");
    assert!(corrections2.contains(&"ㄓㄨㄣ".to_string()), "Should suggest ㄨㄣ variant");
}

#[test]
fn zhuyin_correct_hsu_zh_j_confusion() {
    // HSU keyboard: ㄓ and ㄐ map to same key
    let rules = libzhuyin::standard_fuzzy_rules();
    let parser = ZhuyinParser::new(rules, &["ㄓㄨ", "ㄐㄩ"]);
    
    let corrections = parser.apply_corrections("ㄓㄨ");
    assert!(corrections.contains(&"ㄐㄨ".to_string()), "HSU: Should suggest ㄐ variant");
    
    let corrections2 = parser.apply_corrections("ㄐㄩ");
    assert!(corrections2.contains(&"ㄓㄩ".to_string()), "HSU: Should suggest ㄓ variant");
}

#[test]
fn zhuyin_correct_hsu_ch_q_confusion() {
    // HSU keyboard: ㄔ and ㄑ map to same key
    let rules = libzhuyin::standard_fuzzy_rules();
    let parser = ZhuyinParser::new(rules, &["ㄔㄨ", "ㄑㄩ"]);
    
    let corrections = parser.apply_corrections("ㄔㄨ");
    assert!(corrections.contains(&"ㄑㄨ".to_string()), "HSU: Should suggest ㄑ variant");
    
    let corrections2 = parser.apply_corrections("ㄑㄩ");
    assert!(corrections2.contains(&"ㄔㄩ".to_string()), "HSU: Should suggest ㄔ variant");
}

#[test]
fn zhuyin_correct_hsu_sh_x_confusion() {
    // HSU keyboard: ㄕ and ㄒ map to same key
    let rules = libzhuyin::standard_fuzzy_rules();
    let parser = ZhuyinParser::new(rules, &["ㄕㄨ", "ㄒㄩ"]);
    
    let corrections = parser.apply_corrections("ㄕㄨ");
    assert!(corrections.contains(&"ㄒㄨ".to_string()), "HSU: Should suggest ㄒ variant");
    
    let corrections2 = parser.apply_corrections("ㄒㄩ");
    assert!(corrections2.contains(&"ㄕㄩ".to_string()), "HSU: Should suggest ㄕ variant");
}

#[test]
fn zhuyin_correct_eten26_zh_z_confusion() {
    // ETEN26 keyboard: ㄓ and ㄗ confusion
    let rules = libzhuyin::standard_fuzzy_rules();
    let parser = ZhuyinParser::new(rules, &["ㄓㄨ", "ㄗㄨ"]);
    
    let corrections = parser.apply_corrections("ㄓㄨ");
    assert!(corrections.contains(&"ㄗㄨ".to_string()), "ETEN26: Should suggest ㄗ variant");
    
    let corrections2 = parser.apply_corrections("ㄗㄨ");
    assert!(corrections2.contains(&"ㄓㄨ".to_string()), "ETEN26: Should suggest ㄓ variant");
}

#[test]
fn zhuyin_correct_eten26_ch_c_confusion() {
    // ETEN26 keyboard: ㄔ and ㄘ confusion
    let rules = libzhuyin::standard_fuzzy_rules();
    let parser = ZhuyinParser::new(rules, &["ㄔㄨ", "ㄘㄨ"]);

    let corrections = parser.apply_corrections("ㄔㄨ");
    assert!(corrections.contains(&"ㄘㄨ".to_string()), "ETEN26: Should suggest ㄘ variant");
    
    let corrections2 = parser.apply_corrections("ㄘㄨ");
    assert!(corrections2.contains(&"ㄔㄨ".to_string()), "ETEN26: Should suggest ㄔ variant");
}

#[test]
fn zhuyin_correct_eten26_sh_s_confusion() {
    // ETEN26 keyboard: ㄕ and ㄙ confusion
    let rules = libzhuyin::standard_fuzzy_rules();
    let parser = ZhuyinParser::new(rules, &["ㄕㄨ", "ㄙㄨ"]);
    
    let corrections = parser.apply_corrections("ㄕㄨ");
    assert!(corrections.contains(&"ㄙㄨ".to_string()), "ETEN26: Should suggest ㄙ variant");
    
    let corrections2 = parser.apply_corrections("ㄙㄨ");
    assert!(corrections2.contains(&"ㄕㄨ".to_string()), "ETEN26: Should suggest ㄕ variant");
}

#[test]
fn zhuyin_corrections_in_segmentation() {
    // Test that corrections work during actual segmentation with fuzzy enabled
    let rules = libzhuyin::standard_fuzzy_rules();
    let parser = ZhuyinParser::new(rules, &["ㄉㄨㄟ", "ㄓㄨ"]);
    
    // Input with error that should be corrected: ㄉㄩㄟ instead of ㄉㄨㄟ
    let result = parser.segment_best("ㄉㄩㄟㄓㄨ", true);
    
    // Should find two syllables (correction applied)
    assert_eq!(result.len(), 2, "Should segment into 2 syllables with correction");
    
    // First syllable should be corrected to ㄉㄨㄟ
    assert_eq!(result[0].text, "ㄉㄨㄟ", "Should correct ㄉㄩㄟ to ㄉㄨㄟ");
    assert_eq!(result[1].text, "ㄓㄨ");
}

#[test]
fn zhuyin_no_corrections_when_exact_match_exists() {
    // When exact match exists, it should be found (though correction may also match)
    let rules = libzhuyin::standard_fuzzy_rules();
    let parser = ZhuyinParser::new(rules, &["ㄉㄨㄟ", "ㄉㄩㄟ"]);
    
    // Should find the input
    let result = parser.segment_best("ㄉㄩㄟ", true);
    assert_eq!(result.len(), 1);
    // Could be either exact match or correction, but should segment correctly
    assert!(result[0].text == "ㄉㄩㄟ" || result[0].text == "ㄉㄨㄟ", 
            "Should find a valid syllable");
}

#[test]
fn zhuyin_multiple_corrections_tried() {
    // Test that multiple correction types can be applied
    let rules = libzhuyin::standard_fuzzy_rules();
    let parser = ZhuyinParser::new(rules, &["ㄓㄨ", "ㄐㄩ", "ㄗㄨ"]);
    
    // Input: ㄓㄨ could be corrected via HSU (ㄓ->ㄐ) OR ETEN26 (ㄓ->ㄗ)
    let corrections = parser.apply_corrections("ㄓㄨ");
    
    // Should include both HSU and ETEN26 corrections
    assert!(corrections.contains(&"ㄐㄨ".to_string()), "Should have HSU correction");
    assert!(corrections.contains(&"ㄗㄨ".to_string()), "Should have ETEN26 correction");
    assert!(corrections.len() >= 2, "Should have multiple corrections");
}

#[test]
fn zhuyin_corrections_empty_for_no_patterns() {
    // Corrections should return empty vec when no patterns match
    let rules = libzhuyin::standard_fuzzy_rules();
    let parser = ZhuyinParser::new(rules, &[""]);

    let corrections = parser.apply_corrections("ㄅㄚ");  // ba - no correction patterns
    assert!(corrections.is_empty(), "Should have no corrections for ㄅㄚ");
}

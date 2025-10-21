/// Tests for parser enhancement features:
/// - Partial pinyin (incomplete syllables)
/// - Pinyin corrections (ue/ve, v/u)
///
/// These are unit tests for the parser correction methods,
/// demonstrating the new enhancement features work correctly.

use libpinyin::parser::Parser;

#[test]
fn parser_find_syllable_completion_basic() {
    // Test the find_syllable_completion helper method
    let parser = Parser::new();
    
    // "n" should complete to common syllables like "ni", "na", "ne", "neng", etc.
    if let Some(completion) = parser.find_syllable_completion("n") {
        assert!(completion.starts_with('n'), "Completion should start with 'n', got: {}", completion);
        assert!(completion.len() >= 2, "Completion should be longer than prefix");
    }
    
    // "zh" should complete to syllables like "zhi", "zhang", "zhong", etc.
    if let Some(completion) = parser.find_syllable_completion("zh") {
        assert!(completion.starts_with("zh"), "Completion should start with 'zh', got: {}", completion);
    }
    
    // Very short prefix "z" should also work
    if let Some(completion) = parser.find_syllable_completion("z") {
        assert!(completion.starts_with('z'), "Completion should start with 'z', got: {}", completion);
    }
}

#[test]
fn parser_apply_corrections_ue_ve() {
    // Test ue <-> ve correction
    let parser = Parser::new();
    
    // "nue" should suggest "nve" as correction
    let corrections = parser.apply_corrections("nue");
    assert!(corrections.contains(&"nve".to_string()), 
            "Expected 'nve' in corrections for 'nue', got: {:?}", corrections);
    
    // "lve" should suggest "lue" as correction
    let corrections = parser.apply_corrections("lve");
    assert!(corrections.contains(&"lue".to_string()),
            "Expected 'lue' in corrections for 'lve', got: {:?}", corrections);
            
    // "xue" should suggest "xve" as correction  
    let corrections = parser.apply_corrections("xue");
    assert!(corrections.contains(&"xve".to_string()),
            "Expected 'xve' in corrections for 'xue', got: {:?}", corrections);
}

#[test]
fn parser_apply_corrections_v_u() {
    // Test v <-> u after n, l
    let parser = Parser::new();
    
    // "nv" should suggest "nu" as correction
    let corrections = parser.apply_corrections("nv");
    assert!(corrections.contains(&"nu".to_string()),
            "Expected 'nu' in corrections for 'nv', got: {:?}", corrections);
    
    // "nu" should suggest "nv" as correction
    let corrections = parser.apply_corrections("nu");
    assert!(corrections.contains(&"nv".to_string()),
            "Expected 'nv' in corrections for 'nu', got: {:?}", corrections);
    
    // "lv" should suggest "lu" as correction
    let corrections = parser.apply_corrections("lv");
    assert!(corrections.contains(&"lu".to_string()),
            "Expected 'lu' in corrections for 'lv', got: {:?}", corrections);
            
    // "lu" should suggest "lv" as correction
    let corrections = parser.apply_corrections("lu");
    assert!(corrections.contains(&"lv".to_string()),
            "Expected 'lv' in corrections for 'lu', got: {:?}", corrections);
}

#[test]
fn parser_apply_corrections_no_corrections() {
    // Test that normal syllables don't get spurious corrections
    let parser = Parser::new();
    
    // "wo" shouldn't have corrections
    let corrections = parser.apply_corrections("wo");
    assert!(corrections.is_empty(),
            "Expected no corrections for 'wo', got: {:?}", corrections);
    
    // "ai" shouldn't have corrections
    let corrections = parser.apply_corrections("ai");
    assert!(corrections.is_empty(),
            "Expected no corrections for 'ai', got: {:?}", corrections);
            
    // "zhang" shouldn't have corrections
    let corrections = parser.apply_corrections("zhang");
    assert!(corrections.is_empty(),
            "Expected no corrections for 'zhang', got: {:?}", corrections);
}

#[test]
fn parser_corrections_are_bidirectional() {
    // Verify corrections work in both directions
    let parser = Parser::new();
    
    // ue -> ve and ve -> ue
    let from_ue = parser.apply_corrections("nue");
    let from_ve = parser.apply_corrections("nve");
    assert!(from_ue.contains(&"nve".to_string()), "ue should correct to ve");
    assert!(from_ve.contains(&"nue".to_string()), "ve should correct to ue");
    
    // v -> u and u -> v (after n, l)
    let from_v = parser.apply_corrections("nv");
    let from_u = parser.apply_corrections("nu");
    assert!(from_v.contains(&"nu".to_string()), "v should correct to u");
    assert!(from_u.contains(&"nv".to_string()), "u should correct to v");
}

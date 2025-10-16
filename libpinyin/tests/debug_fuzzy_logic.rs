/// Debug the fuzzy matching logic step by step
use libpinyin::parser::{Parser, FuzzyMap};

#[test]
fn debug_fuzzy_logic_step_by_step() {
    let parser = Parser::with_syllables(&["zi", "zhi", "si", "shi"]);
    
    // Test FuzzyMap directly
    let fuzzy_map = FuzzyMap::new();
    let zi_alts = fuzzy_map.alternatives("zi");
    println!("FuzzyMap alternatives for 'zi': {:?}", zi_alts);
    
    // Test if "zhi" is considered an alternative to "zi"
    let contains_zhi = zi_alts.contains(&"zhi".to_string());
    println!("Does 'zi' alternatives contain 'zhi'? {}", contains_zhi);
    
    // Test the segment_best method directly
    let best_zi = parser.segment_best("zi", false);
    println!("Best segmentation for 'zi' (no fuzzy): {:?}", best_zi);
    
    let best_zi_fuzzy = parser.segment_best("zi", true);
    println!("Best segmentation for 'zi' (with fuzzy): {:?}", best_zi_fuzzy);
    
    // Test manually what happens with "zhi"
    let best_zhi = parser.segment_best("zhi", false);
    println!("Best segmentation for 'zhi' (no fuzzy): {:?}", best_zhi);
    
    // The key question: when we process "zi" with fuzzy=true, 
    // does the fuzzy alternative "zhi" get considered?
    // Let's see if we can get multiple alternatives
    let all_zi_fuzzy = parser.segment_top_k("zi", 10, true);
    println!("All segmentations for 'zi' (with fuzzy, k=10): {:?}", all_zi_fuzzy);
}
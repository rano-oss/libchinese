/// Debug the exact fuzzy matching execution path
use libpinyin::parser::{Parser, FuzzyMap};

#[test]
fn debug_fuzzy_execution_path() {
    let parser = Parser::with_syllables(&["zi", "zhi", "si", "shi"]);
    
    // Test the fuzzy map
    let fuzzy_map = FuzzyMap::new();
    let zi_alts = fuzzy_map.alternatives("zi");
    println!("FuzzyMap alternatives for 'zi': {:?}", zi_alts);
    
    // Let's manually trace what should happen:
    // 1. Input "zi" (length 2)
    // 2. At position 0, length 2, we get substring "zi"
    // 3. fuzzy.alternatives("zi") should return ["zi", "zhi"]
    // 4. For alt="zhi": trie.contains_word("zhi") should be true
    // 5. alt != substr: "zhi" != "zi" should be true
    // 6. We should generate a fuzzy match
    
    // Let's test each step:
    println!("Does fuzzy alternatives contain 'zhi'? {}", zi_alts.contains(&"zhi".to_string()));
    
    // We can't directly test trie.contains_word, but let's test if "zhi" segments correctly
    let zhi_segments = parser.segment_best("zhi", false);
    println!("'zhi' segments correctly: {:?}", zhi_segments);
    
    // The critical test: can we get different results?
    let zi_no_fuzzy = parser.segment_best("zi", false);
    let zi_with_fuzzy = parser.segment_best("zi", true);
    
    println!("zi (no fuzzy): {:?}", zi_no_fuzzy);
    println!("zi (with fuzzy): {:?}", zi_with_fuzzy);
    
    // Let's also test all possible segmentations
    let all_zi = parser.segment_top_k("zi", 20, true);
    println!("All zi segmentations (k=20): {:#?}", all_zi);
    
    // Test the reverse: does "zhi" -> "zi" work?
    let all_zhi = parser.segment_top_k("zhi", 20, true);
    println!("All zhi segmentations (k=20): {:#?}", all_zhi);
}
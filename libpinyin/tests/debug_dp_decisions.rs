/// Debug the fuzzy DP decision logic with verbose output
use libpinyin::parser::{Parser};

#[test]
fn debug_fuzzy_dp_decisions() {
    let parser = Parser::with_syllables(&["zi", "zhi"]);
    
    // Test that both syllables exist
    let zi_exact = parser.segment_best("zi", false);
    let zhi_exact = parser.segment_best("zhi", false);
    println!("zi exact: {:?}", zi_exact);
    println!("zhi exact: {:?}", zhi_exact);
    
    // Now test fuzzy matching
    println!("\n=== Testing fuzzy matching for 'zi' ===");
    
    // The key insight: let's see what segment_top_k returns
    let all_zi_fuzzy = parser.segment_top_k("zi", 10, true);
    for (i, seg) in all_zi_fuzzy.iter().enumerate() {
        println!("Option {}: {:?}", i, seg);
    }
    
    // Test with a different input to see the pattern
    println!("\n=== Testing fuzzy matching for 'zhi' ===");
    let all_zhi_fuzzy = parser.segment_top_k("zhi", 10, true);
    for (i, seg) in all_zhi_fuzzy.iter().enumerate() {
        println!("Option {}: {:?}", i, seg);
    }
    
    // Let's test if there's any cross-mapping by testing both directions
    // If zi->zhi doesn't work, maybe zhi->zi does?
    println!("\n=== Cross-mapping test ===");
    
    let zi_matches_zhi = all_zi_fuzzy.iter().any(|seg| {
        seg.len() == 1 && seg[0].text == "zhi"
    });
    
    let zhi_matches_zi = all_zhi_fuzzy.iter().any(|seg| {
        seg.len() == 1 && seg[0].text == "zi"
    });
    
    println!("Does 'zi' fuzzy matching include 'zhi'? {}", zi_matches_zhi);
    println!("Does 'zhi' fuzzy matching include 'zi'? {}", zhi_matches_zi);
}
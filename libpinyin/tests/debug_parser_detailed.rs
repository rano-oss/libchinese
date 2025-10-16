/// Detailed debug test for parser fuzzy matching
use libpinyin::parser::Parser;

#[test]
fn debug_parser_fuzzy_detailed() {
    let parser = Parser::with_syllables(&["zi", "zhi", "si", "shi"]);
    
    // Test if "zhi" is in the trie
    println!("Testing trie contains:");
    // We can't access the trie directly, but we can test segmentation
    
    // Test segmentation of exact matches
    let segs_zi = parser.segment_top_k("zi", 5, false);
    println!("Segmentations for 'zi' (no fuzzy): {:?}", segs_zi);
    
    let segs_zhi = parser.segment_top_k("zhi", 5, false);
    println!("Segmentations for 'zhi' (no fuzzy): {:?}", segs_zhi);
    
    // Test fuzzy segmentation
    let segs_zi_fuzzy = parser.segment_top_k("zi", 5, true);
    println!("Segmentations for 'zi' (with fuzzy): {:?}", segs_zi_fuzzy);
    
    // Check if we can get "zhi" when looking for "zi"
    let segs_mixed = parser.segment_top_k("zhi", 5, true);
    println!("Segmentations for 'zhi' (with fuzzy): {:?}", segs_mixed);
}
/// Debug test for fuzzy matching
use libpinyin::parser::Parser;

#[test]
fn debug_fuzzy_alternatives() {
    let parser = Parser::with_syllables(&["zi", "zhi", "si", "shi"]);
    
    // Test parser fuzzy matching
    let segmentations = parser.segment_top_k("zi", 3, true);
    println!("Segmentations for 'zi' with fuzzy=true: {:?}", segmentations);
    
    // Test basic parser functionality
    let segmentations_no_fuzzy = parser.segment_top_k("zi", 3, false);
    println!("Segmentations for 'zi' with fuzzy=false: {:?}", segmentations_no_fuzzy);
}
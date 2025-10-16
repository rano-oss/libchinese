/// Test just segment_best to trigger debug output
use libpinyin::parser::{Parser};

#[test]
fn debug_segment_best_direct() {
    let parser = Parser::with_syllables(&["zi", "zhi"]);
    
    println!("=== Testing segment_best('zi', true) directly ===");
    let result = parser.segment_best("zi", true);
    println!("Result: {:?}", result);
}
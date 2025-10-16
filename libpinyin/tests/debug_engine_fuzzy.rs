/// Debug the Engine fuzzy key generation
use libpinyin::{Engine, parser::Parser};
use libchinese_core::{Config, Lexicon, Model, NGramModel, UserDict};

#[test]
fn debug_engine_fuzzy_keys() {
    // Set up the same test case as the failing test
    let mut lex = Lexicon::new();
    lex.insert("zi", "字");
    lex.insert("zhi", "知");
    lex.insert("si", "丝");
    lex.insert("shi", "是");
    
    let ng = NGramModel::new();
    let user = UserDict::new();
    let cfg = Config::default(); // Should include comprehensive fuzzy rules
    
    // Test the Config fuzzy rules directly 
    println!("=== Config fuzzy rules ===");
    println!("Config fuzzy rules: {:?}", cfg.fuzzy);
    
    let model = Model::new(lex, ng, user, cfg, None);

    let parser = Parser::with_syllables(&[
        "zi", "zhi", "si", "shi"
    ]);
    let engine = Engine::new(model, parser);

    // Test what segmentations the parser produces for "zi"
    println!("\n=== Parser segmentations for 'zi' ===");
    let debug_parser = Parser::with_syllables(&["zi", "zhi", "si", "shi"]);
    let segmentations = debug_parser.segment_top_k("zi", 5, true);
    for (i, seg) in segmentations.iter().enumerate() {
        println!("Seg {}: {:?}", i, seg);
    }

    // Test the engine input processing
    println!("\n=== Engine candidates for 'zi' ===");
    let candidates = engine.input("zi");
    for (i, cand) in candidates.iter().enumerate() {
        println!("Candidate {}: {} (score: {})", i, cand.text, cand.score);
    }
    
    // Check if we get both 字 and 知
    let candidate_texts: Vec<&str> = candidates.iter().map(|c| c.text.as_str()).collect();
    println!("Candidate texts: {:?}", candidate_texts);
    println!("Contains '字'? {}", candidate_texts.contains(&"字"));
    println!("Contains '知'? {}", candidate_texts.contains(&"知"));
}
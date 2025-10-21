//! Wade-Giles input example
//!
//! Demonstrates using Wade-Giles romanization with libpinyin.
//! Users can type in Wade-Giles and get pinyin-based results.
//!
//! Run:
//!   cargo run --example wade_giles_input

use libpinyin::wade_giles;

fn main() {
    println!("=== Wade-Giles to Pinyin Conversion Demo ===\n");

    // Common Wade-Giles inputs and their pinyin equivalents
    let examples = vec![
        ("pei-ching", "Beijing (北京)"),
        ("chung-kuo", "China (中国)"),
        ("ch'ing-hua", "Tsinghua (清华)"),
        ("shang-hai", "Shanghai (上海)"),
        ("nan-ching", "Nanjing (南京)"),
        ("kuang-chou", "Guangzhou (广州)"),
        ("t'ai-pei", "Taipei (台北)"),
        ("hsi-an", "Xi'an (西安)"),
    ];

    println!("Historical Place Names:");
    println!("{:<20} {:<15} {}", "Wade-Giles", "Pinyin", "Name");
    println!("{}", "-".repeat(60));
    
    for (wade_giles, name) in examples {
        let pinyin = wade_giles::convert_input(wade_giles);
        println!("{:<20} {:<15} {}", wade_giles, pinyin, name);
    }

    println!("\n=== Common Syllables ===\n");
    
    let syllables = vec![
        ("ch'ing", "qing (清)"),
        ("chang", "zhang (张)"),
        ("hsi", "xi (西)"),
        ("tzu", "zi (子)"),
        ("erh", "er (二)"),
        ("jih", "ri (日)"),
    ];

    println!("{:<15} {:<15} {}", "Wade-Giles", "Pinyin", "Meaning");
    println!("{}", "-".repeat(45));
    
    for (wade, meaning) in syllables {
        let pinyin = wade_giles::convert_syllable(wade);
        println!("{:<15} {:<15} {}", wade, pinyin, meaning);
    }

    println!("\n=== Interactive Mode ===");
    println!("(In a real IME, this would be integrated into the engine)\n");

    // Simulate IME workflow
    let user_inputs = vec![
        "ni hao",
        "ch'ing wen",
        "hsieh hsieh",
    ];

    for input in user_inputs {
        let converted = wade_giles::convert_input(input);
        println!("User types (Wade-Giles): {}", input);
        println!("Engine receives (Pinyin): {}", converted);
        println!("  → IME would look up '{}' in lexicon", converted);
        println!();
    }

    println!("=== Usage Notes ===");
    println!("
1. Wade-Giles uses apostrophes (') for aspiration:
   - ch' → q (aspirated)
   - ch → zh (unaspirated)

2. Common conversions:
   - p' → p, p → b
   - t' → t, t → d  
   - k' → k, k → g
   - ts' → c, ts → z
   - hs → x
   - j → r

3. This converter handles both individual syllables and full phrases.

4. For IME integration, call wade_giles::convert_input() before
   passing input to the parser.
    ");
}

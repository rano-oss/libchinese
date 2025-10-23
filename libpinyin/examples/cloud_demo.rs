//! Example demonstrating cloud input usage.
//!
//! Run with: cargo run --example cloud_demo nihao

use libpinyin::{CloudInput, CloudProvider};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <pinyin>", args[0]);
        eprintln!("Example: {} nihao", args[0]);
        std::process::exit(1);
    }
    
    let pinyin = &args[1];
    
    println!("🌐 Cloud Input Demo");
    println!("==================\n");
    println!("Querying for: {}\n", pinyin);
    
    // Create cloud input client (Baidu provider)
    let mut cloud = CloudInput::new(CloudProvider::Baidu);
    
    // Enable cloud input
    cloud.set_enabled(true);
    
    // Set timeout to 2 seconds for demo (default is 500ms)
    cloud.set_timeout(2000);
    
    println!("⏳ Sending request to Baidu Input API...");
    
    // Query cloud service (blocking call with timeout)
    let results = cloud.query(pinyin);
    
    if results.is_empty() {
        println!("\n❌ No results found (check network connection)");
        return;
    }
    
    println!("\n✅ Got {} candidates:\n", results.len());
    
    for (i, candidate) in results.iter().enumerate() {
        println!("  {}. {} (confidence: {:.2})", 
                 i + 1, 
                 candidate.text, 
                 candidate.confidence);
    }
    
    println!("\n💡 Tip: Cloud input provides rare phrases and proper names");
    println!("   that might not be in the local dictionary.");
}

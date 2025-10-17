use libchinese_core::{Candidate, Config, Lexicon, Model, NGramModel, UserDict, Interpolator};
use std::io::{self, BufRead};
use std::path::Path;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;

fn build_demo_model() -> Model {
    // Prefer loading runtime artifacts from `data/` if they exist.
    let data_dir = Path::new("data");
    let fst_path = data_dir.join("pinyin.fst");
    let redb_path = data_dir.join("pinyin.redb");

    if fst_path.exists() && redb_path.exists() {
        // Try to load lexicon on-demand from fst + redb
        match Lexicon::load_from_fst_redb(&fst_path, &redb_path) {
            Ok(lx) => {
                println!("✓ Loaded lexicon from artifacts");
                
                // Load ngram model
                let ng = if let Ok(mut f) = File::open("data/ngram.bincode") {
                    let mut b = Vec::new();
                    if f.read_to_end(&mut b).is_ok() {
                        if let Ok(m) = bincode::deserialize::<NGramModel>(&b) {
                            println!("✓ Loaded n-gram model");
                            m
                        } else {
                            NGramModel::new()
                        }
                    } else {
                        NGramModel::new()
                    }
                } else {
                    NGramModel::new()
                };

                // Load userdict
                let home = std::env::var("HOME")
                    .or_else(|_| std::env::var("USERPROFILE"))
                    .unwrap_or_else(|_| ".".to_string());
                let user_path = std::path::PathBuf::from(home)
                    .join(".pinyin")
                    .join("userdict.redb");
                let user = UserDict::new(&user_path).unwrap_or_else(|e| {
                    eprintln!("⚠ Failed to create userdict at {:?}: {}", user_path, e);
                    let temp_path = std::env::temp_dir().join(format!(
                        "libpinyin_userdict_{}.redb",
                        std::process::id()
                    ));
                    UserDict::new(&temp_path).expect("failed to create temp userdict")
                });
                
                // Load interpolator if available
                let lambdas_fst = Path::new("data").join("pinyin.lambdas.fst");
                let lambdas_redb = Path::new("data").join("pinyin.lambdas.redb");
                let interp = if lambdas_fst.exists() && lambdas_redb.exists() {
                    match Interpolator::load(&lambdas_fst, &lambdas_redb) {
                        Ok(i) => {
                            println!("✓ Loaded interpolator");
                            Some(Arc::new(i))
                        }
                        Err(e) => { 
                            eprintln!("⚠ Failed to load interpolator: {}", e); 
                            None 
                        }
                    }
                } else { 
                    None 
                };

                let cfg = Config::default();
                return Model::new(lx, ng, user, cfg, interp);
            }
            Err(e) => eprintln!("⚠ Failed to load lexicon: {}", e),
        }
    }

    // Fallback: in-memory demo model
    println!("ℹ Using fallback demo model");
    let lx = Lexicon::load_demo();

    let mut ng = NGramModel::new();
    ng.insert_unigram("你", -1.0);
    ng.insert_unigram("好", -1.2);
    ng.insert_unigram("号", -2.0);
    ng.insert_unigram("中", -1.1);
    ng.insert_unigram("国", -1.3);

    let temp_path = std::env::temp_dir().join(format!(
        "libpinyin_fallback_userdict_{}.redb",
        std::process::id()
    ));
    let user = UserDict::new(&temp_path).expect("create fallback userdict");
    user.learn("你好");

    let cfg = Config::default();
    Model::new(lx, ng, user, cfg, None)
}

fn main() {
    println!("═══════════════════════════════════════════════════");
    println!("  libpinyin - Interactive Pinyin Input Test");
    println!("═══════════════════════════════════════════════════");
    println!();
    
    let model = build_demo_model();
    let parser = libpinyin::parser::Parser::with_syllables(&[
        "ni", "hao", "zhong", "guo", "wo", "ai", "men", "de", "shi", "jie"
    ]);
    let engine = libpinyin::Engine::new(model, parser);
    
    println!("Ready! Type pinyin and press Enter.");
    println!("Examples: nihao, zhongguo, woaini");
    println!("Press Ctrl+C to exit.");
    println!();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(raw) => {
                let input = raw.trim();
                if input.is_empty() {
                    continue;
                }
                
                let cands = engine.input(input);
                if cands.is_empty() {
                    println!("  → (no candidates found)\n");
                } else {
                    for (i, c) in cands.iter().enumerate().take(5) {
                        println!("  {}. {} (score: {:.1})", i + 1, c.text, c.score);
                    }
                    println!();
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }
}

use libchinese_core::{Config, Lexicon, Model, NGramModel, UserDict};use libchinese_core::{Candidate, Config, Lexicon, Model, NGramModel, UserDict, Interpolator};use libzhuyin::parser::ZhuyinParser;use libchinese_core::{Config, Lexicon, Model, NGramModel, UserDict, Interpolator};

use std::io::{self, BufRead};

use std::path::Path;use std::io::{self, BufRead};

use std::fs::File;

use std::io::Read;use std::path::Path;use libchinese_core::{Model, NGram};use std::io::{self, BufRead};



fn build_demo_model() -> Model {use std::fs::File;

    // Try to load runtime artifacts from `data/zhuyin/` if they exist

    let data_dir = Path::new("data/zhuyin");use std::io::Read;use std::io::{self, BufRead, Write};use std::path::Path;

    let fst_path = data_dir.join("zhuyin.fst");

    let redb_path = data_dir.join("zhuyin.redb");use std::sync::Arc;



    if fst_path.exists() && redb_path.exists() {use std::fs::File;

        match Lexicon::load_from_fst_redb(&fst_path, &redb_path) {

            Ok(lx) => {fn build_demo_model() -> Model {

                println!("✓ Loaded zhuyin lexicon from artifacts");

                    // Prefer loading runtime artifacts from `data/zhuyin/` if they exist.fn build_demo_model() -> Model {use std::io::Read;

                // Load n-gram model

                let ng = if let Ok(mut f) = File::open("data/ngram.bincode") {    let data_dir = Path::new("data/zhuyin");

                    let mut b = Vec::new();

                    if f.read_to_end(&mut b).is_ok() {    let fst_path = data_dir.join("zhuyin.fst");    let parser = ZhuyinParser::from_table("data/zhuyin/tsi.redb").expect("failed to load parser");use std::sync::Arc;

                        if let Ok(m) = bincode::deserialize::<NGramModel>(&b) {

                            println!("✓ Loaded n-gram model");    let redb_path = data_dir.join("zhuyin.redb");

                            m

                        } else {    let lexicon = libchinese_core::Lexicon::from_fst("data/zhuyin/zhuyin.fst")

                            NGramModel::new()

                        }    if fst_path.exists() && redb_path.exists() {

                    } else {

                        NGramModel::new()        // Try to load lexicon on-demand from fst + redb        .expect("failed to load lexicon");mod parser;

                    }

                } else {        match Lexicon::load_from_fst_redb(&fst_path, &redb_path) {

                    NGramModel::new()

                };            Ok(lx) => {    let ngram = NGram::load("data/ngram.bincode").expect("failed to load ngram");mod engine;



                // Load userdict                println!("✓ Loaded lexicon from artifacts");

                let home = std::env::var("HOME")

                    .or_else(|_| std::env::var("USERPROFILE"))                    Model::new(parser, lexicon, ngram)

                    .unwrap_or_else(|_| ".".to_string());

                let user_path = std::path::PathBuf::from(home)                // Load ngram model

                    .join(".zhuyin")

                    .join("userdict.redb");                let ng = if let Ok(mut f) = File::open("data/ngram.bincode") {}use parser::ZhuyinParser;

                let user = UserDict::new(&user_path).unwrap_or_else(|e| {

                    eprintln!("⚠ Failed to create userdict: {}", e);                    let mut b = Vec::new();

                    let temp_path = std::env::temp_dir().join(format!(

                        "libzhuyin_userdict_{}.redb",                    if f.read_to_end(&mut b).is_ok() {use engine::Engine;

                        std::process::id()

                    ));                        if let Ok(m) = bincode::deserialize::<NGramModel>(&b) {

                    UserDict::new(&temp_path).expect("failed to create temp userdict")

                });                            println!("✓ Loaded n-gram model");fn main() -> Result<(), Box<dyn std::error::Error>> {



                let cfg = Config::default();                            m

                return Model::new(lx, ng, user, cfg, None);

            }                        } else {    println!("=== libzhuyin Interactive Demo ===");fn build_demo_model() -> Model {

            Err(e) => eprintln!("⚠ Failed to load lexicon: {}", e),

        }                            NGramModel::new()

    }

                        }    println!("Type Zhuyin syllables to get Chinese candidates.");    // Try to load runtime artifacts from `data/zhuyin/` if they exist

    // Fallback: in-memory demo model

    println!("ℹ Using fallback demo model");                    } else {

    let lx = Lexicon::load_demo();

    let mut ng = NGramModel::new();                        NGramModel::new()    println!("Examples:");    let data_dir = Path::new("data/zhuyin");

    ng.insert_unigram("你", -1.0);

    ng.insert_unigram("好", -1.2);                    }

    

    let temp_path = std::env::temp_dir().join(format!(                } else {    println!("  ㄋㄧˇ ㄏㄠˇ  -> 你好");    let fst_path = data_dir.join("zhuyin.fst");

        "libzhuyin_fallback_userdict_{}.redb",

        std::process::id()                    NGramModel::new()

    ));

    let user = UserDict::new(&temp_path).expect("create fallback userdict");                };    println!("  ㄓㄨㄥ ㄨㄣˊ  -> 中文");    let redb_path = data_dir.join("zhuyin.redb");



    let cfg = Config::default();

    Model::new(lx, ng, user, cfg, None)

}                // Load userdict    println!("Type 'quit' or 'exit' to quit.\n");



fn main() {                let home = std::env::var("HOME")

    println!("═══════════════════════════════════════════════════");

    println!("  libzhuyin - Interactive Zhuyin Input Test");                    .or_else(|_| std::env::var("USERPROFILE"))    if fst_path.exists() && redb_path.exists() {

    println!("═══════════════════════════════════════════════════");

    println!();                    .unwrap_or_else(|_| ".".to_string());

    

    let model = build_demo_model();                let user_path = std::path::PathBuf::from(home)    let model = build_demo_model();        match Lexicon::load_from_fst_redb(&fst_path, &redb_path) {

    

    println!("Ready! Type Zhuyin syllables and press Enter.");                    .join(".zhuyin")

    println!("Examples: ㄋㄧˇㄏㄠˇ, ㄓㄨㄥㄨㄣˊ");

    println!("Press Ctrl+C to exit.");                    .join("userdict.redb");    let stdin = io::stdin();            Ok(lx) => {

    println!();

                let user = UserDict::new(&user_path).unwrap_or_else(|e| {

    let stdin = io::stdin();

    for line in stdin.lock().lines() {                    eprintln!("⚠ Failed to create userdict at {:?}: {}", user_path, e);    let mut stdout = io::stdout();                println!("✓ Loaded zhuyin lexicon from artifacts");

        match line {

            Ok(raw) => {                    let temp_path = std::env::temp_dir().join(format!(

                let input = raw.trim();

                if input.is_empty() {                        "libzhuyin_userdict_{}.redb",                

                    continue;

                }                        std::process::id()

                

                match model.candidates(input) {                    ));    loop {                // Load n-gram model

                    Ok(cands) => {

                        if cands.is_empty() {                    UserDict::new(&temp_path).expect("failed to create temp userdict")

                            println!("  → (no candidates found)\n");

                        } else {                });        print!("> ");                let ng = if let Ok(mut f) = File::open("data/zhuyin/ngram.bincode") {

                            for (i, (text, score)) in cands.iter().enumerate().take(5) {

                                println!("  {}. {} (score: {:.1})", i + 1, text, score);                

                            }

                            println!();                // Load interpolator if available        stdout.flush()?;                    let mut b = Vec::new();

                        }

                    }                let lambdas_redb = Path::new("data/zhuyin").join("zhuyin.lambdas.redb");

                    Err(e) => {

                        eprintln!("  Error: {}\n", e);                let interp = if lambdas_redb.exists() {                    if f.read_to_end(&mut b).is_ok() {

                    }

                }                    // Note: zhuyin doesn't have a separate lambdas.fst, just the redb

            }

            Err(e) => {                    let dummy_fst = Path::new("data/zhuyin/zhuyin.fst");        let mut line = String::new();                        if let Ok(m) = bincode::deserialize::<NGramModel>(&b) {

                eprintln!("Error reading input: {}", e);

                break;                    match Interpolator::load(dummy_fst, &lambdas_redb) {

            }

        }                        Ok(i) => {        stdin.lock().read_line(&mut line)?;                            println!("✓ Loaded n-gram model");

    }

}                            println!("✓ Loaded interpolator");


                            Some(Arc::new(i))        let input = line.trim();                            m

                        }

                        Err(e) => {                         } else {

                            eprintln!("⚠ Failed to load interpolator: {}", e); 

                            None         if input.is_empty() {                            NGramModel::new()

                        }

                    }            continue;                        }

                } else { 

                    None         }                    } else {

                };

                        NGramModel::new()

                let cfg = Config::default();

                return Model::new(lx, ng, user, cfg, interp);        if input == "quit" || input == "exit" {                    }

            }

            Err(e) => eprintln!("⚠ Failed to load lexicon: {}", e),            println!("再見！");                } else {

        }

    }            break;                    NGramModel::new()



    // Fallback: in-memory demo model        }                };

    println!("ℹ Using fallback demo model");

    let lx = Lexicon::load_demo();



    let mut ng = NGramModel::new();        match model.candidates(input) {                // Load userdict

    ng.insert_unigram("你", -1.0);

    ng.insert_unigram("好", -1.2);            Ok(candidates) => {                let home = std::env::var("HOME")

    ng.insert_unigram("中", -1.1);

    ng.insert_unigram("文", -1.3);                if candidates.is_empty() {                    .or_else(|_| std::env::var("USERPROFILE"))



    let temp_path = std::env::temp_dir().join(format!(                    println!("  (no candidates)");                    .unwrap_or_else(|_| ".".to_string());

        "libzhuyin_fallback_userdict_{}.redb",

        std::process::id()                } else {                let user_path = std::path::PathBuf::from(home)

    ));

    let user = UserDict::new(&temp_path).expect("create fallback userdict");                    for (i, (phrase, _score)) in candidates.iter().take(10).enumerate() {                    .join(".zhuyin")



    let cfg = Config::default();                        println!("  {}. {}", i + 1, phrase);                    .join("userdict.redb");

    Model::new(lx, ng, user, cfg, None)

}                    }                let user = UserDict::new(&user_path).unwrap_or_else(|e| {



fn main() {                }                    eprintln!("⚠ Failed to create userdict at {:?}: {}", user_path, e);

    println!("═══════════════════════════════════════════════════");

    println!("  libzhuyin - Interactive Zhuyin Input Test");            }                    let temp_path = std::env::temp_dir().join(format!(

    println!("═══════════════════════════════════════════════════");

    println!();            Err(e) => {                        "libzhuyin_userdict_{}.redb",

    

    let model = build_demo_model();                eprintln!("  Error: {}", e);                        std::process::id()

    let parser = libzhuyin::parser::ZhuyinParser::with_syllables(&[

        "ㄋㄧˇ", "ㄏㄠˇ", "ㄓㄨㄥ", "ㄨㄣˊ"            }                    ));

    ]);

    let engine = libzhuyin::Engine::new(model, parser);        }                    UserDict::new(&temp_path).expect("failed to create temp userdict")

    

    println!("Ready! Type Zhuyin and press Enter.");    }                });

    println!("Examples: ㄋㄧˇㄏㄠˇ (nihao), ㄓㄨㄥㄨㄣˊ (zhongwen)");

    println!("Press Ctrl+C to exit.");                

    println!();

    Ok(())                // Load interpolator if available

    let stdin = io::stdin();

    for line in stdin.lock().lines() {}                let lambdas_fst = data_dir.join("zhuyin.lambdas.fst");

        match line {

            Ok(raw) => {                let lambdas_redb = data_dir.join("zhuyin.lambdas.redb");

                let input = raw.trim();                let interp = if lambdas_fst.exists() && lambdas_redb.exists() {

                if input.is_empty() {                    match Interpolator::load(&lambdas_fst, &lambdas_redb) {

                    continue;                        Ok(i) => {

                }                            println!("✓ Loaded interpolator");

                                            Some(Arc::new(i))

                let cands = engine.input(input);                        }

                if cands.is_empty() {                        Err(e) => { 

                    println!("  → (no candidates found)\n");                            eprintln!("⚠ Failed to load interpolator: {}", e); 

                } else {                            None 

                    for (i, c) in cands.iter().enumerate().take(5) {                        }

                        println!("  {}. {} (score: {:.1})", i + 1, c.text, c.score);                    }

                    }                } else { 

                    println!();                    None 

                }                };

            }

            Err(e) => {                let cfg = Config::default();

                eprintln!("Error: {}", e);                return Model::new(lx, ng, user, cfg, interp);

                break;            }

            }            Err(e) => eprintln!("⚠ Failed to load lexicon: {}", e),

        }        }

    }    }

}

    // Fallback: demo model with zhuyin entries
    println!("ℹ Using fallback demo model");
    let mut lx = Lexicon::new();
    lx.insert("ㄋㄧˇㄏㄠˇ", "你好");
    lx.insert("ㄋㄧˇㄏㄠˋ", "你号");
    lx.insert("ㄓㄨㄥㄍㄨㄛˊ", "中国");

    let mut ng = NGramModel::new();
    ng.insert_unigram("你", -1.0);
    ng.insert_unigram("好", -1.2);
    ng.insert_unigram("号", -2.0);
    ng.insert_unigram("中", -1.1);
    ng.insert_unigram("国", -1.3);

    let temp_path = std::env::temp_dir().join(format!(
        "libzhuyin_fallback_userdict_{}.redb",
        std::process::id()
    ));
    let user = UserDict::new(&temp_path).expect("create fallback userdict");
    user.learn("你好");

    let cfg = Config::default();
    Model::new(lx, ng, user, cfg, None)
}

fn main() {
    println!("═══════════════════════════════════════════════════");
    println!("  libzhuyin - Interactive Zhuyin/Bopomofo Input Test");
    println!("═══════════════════════════════════════════════════");
    println!();
    
    let model = build_demo_model();
    let parser = ZhuyinParser::with_syllables(&[
        "ㄋㄧˇ", "ㄏㄠˇ", "ㄓㄨㄥ", "ㄍㄨㄛˊ"
    ]);
    let engine = Engine::new(model, parser);
    
    println!("Ready! Type zhuyin/bopomofo and press Enter.");
    println!("Examples: ㄋㄧˇㄏㄠˇ, ㄓㄨㄥㄍㄨㄛˊ");
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

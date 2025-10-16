use libchinese_core::{Candidate, Config, Lexicon, Model, NGramModel, UserDict, Interpolator};
use std::io::{self, BufRead};
use std::path::Path;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use clap::{Parser, Subcommand};

mod parser;
mod engine;

use parser::ZhuyinParser;
use engine::Engine;

fn build_demo_model() -> Model {
    // Try to load runtime artifacts from `data/zhuyin/` if they exist
    let data_dir = Path::new("data/zhuyin");
    let fst_path = data_dir.join("zhuyin.fst");
    let redb_path = data_dir.join("zhuyin.redb");

    if fst_path.exists() && redb_path.exists() {
        match Lexicon::load_from_fst_redb(&fst_path, &redb_path) {
            Ok(lx) => {
                println!("loaded zhuyin lexicon from artifacts: '{}' + '{}'", fst_path.display(), redb_path.display());
                
                // Load n-gram model from data/zhuyin/ngram.bincode if present
                let ng = if let Ok(mut f) = File::open("data/zhuyin/ngram.bincode") {
                    let mut b = Vec::new();
                    if f.read_to_end(&mut b).is_ok() {
                        if let Ok(m) = bincode::deserialize::<NGramModel>(&b) {
                            println!("loaded zhuyin ngram model");
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

                let home = std::env::var("HOME")
                    .or_else(|_| std::env::var("USERPROFILE"))
                    .unwrap_or_else(|_| ".".to_string());
                let user_path = std::path::PathBuf::from(home)
                    .join(".zhuyin")
                    .join("userdict.redb");
                let user = UserDict::new(&user_path).unwrap_or_else(|e| {
                    eprintln!("warning: failed to create userdict at {:?}: {}", user_path, e);
                    let temp_path = std::env::temp_dir().join(format!(
                        "libzhuyin_userdict_{}.redb",
                        std::process::id()
                    ));
                    UserDict::new(&temp_path).expect("failed to create temp userdict")
                });
                
                // Load zhuyin interpolator if present
                let lambdas_fst = data_dir.join("zhuyin.lambdas.fst");
                let lambdas_redb = data_dir.join("zhuyin.lambdas.redb");
                let interp = if lambdas_fst.exists() && lambdas_redb.exists() {
                    match Interpolator::load(&lambdas_fst, &lambdas_redb) {
                        Ok(i) => {
                            println!("loaded zhuyin interpolator");
                            Some(Arc::new(i))
                        }
                        Err(e) => { eprintln!("warning: failed to load zhuyin interpolator: {}", e); None }
                    }
                } else { None };

                let cfg = Config::default();
                return Model::new(lx, ng, user, cfg, interp);
            }
            Err(e) => eprintln!("warning: failed to load zhuyin lexicon: {}", e),
        }
    }

    // Fallback: demo model with zhuyin entries
    let mut lx = Lexicon::new();
    // Add some basic zhuyin mappings (using bopomofo notation)
    lx.insert("ㄋㄧˇㄏㄠˇ", "你好");  // ni3 hao3 -> 你好
    lx.insert("ㄋㄧˇㄏㄠˋ", "你号");  // ni3 hao4 -> 你号  
    lx.insert("ㄓㄨㄥㄍㄨㄛˊ", "中国"); // zhong1 guo2 -> 中国

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

fn print_candidate(key: &str, cand: &Candidate, idx: usize) {
    println!("{}. candidate='{}' score={:.4}", idx + 1, cand.text, cand.score);
    println!("   key: {}", key);
}

fn run_repl() {
    let model = build_demo_model();
    let parser = ZhuyinParser::with_syllables(&[
        "ㄋㄧˇ", "ㄏㄠˇ", "ㄏㄠˋ", "ㄓㄨㄥ", "ㄍㄨㄛˊ"
    ]);
    let engine = Engine::new(model, parser);
    
    println!("libzhuyin demo CLI — type zhuyin/bopomofo input and press Enter");
    println!("Example: ㄋㄧˇㄏㄠˇ for 你好");
    println!("Ctrl-D to exit.");

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(raw) => {
                let input = raw.trim();
                if input.is_empty() {
                    continue;
                }
                println!("\nInput: '{}'", input);
                let cands = engine.input(input);
                if cands.is_empty() {
                    println!("  (no candidates found)");
                } else {
                    for (i, c) in cands.iter().enumerate() {
                        print_candidate(input, c, i);
                    }
                }
                println!();
            }
            Err(e) => {
                eprintln!("error reading stdin: {}", e);
                break;
            }
        }
    }
}

#[derive(Parser)]
#[command(name = "libzhuyin")]
#[command(about = "A Rust implementation of Zhuyin/Bopomofo Chinese input method")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    
    /// Single zhuyin input for quick testing
    input: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive REPL mode
    Repl,
    /// Build data models from text corpus
    Build {
        /// Input text corpus file
        #[arg(short, long)]
        input: std::path::PathBuf,
        /// Output model directory
        #[arg(short, long)]
        output: std::path::PathBuf,
        /// Model type to build
        #[arg(long, value_enum, default_value_t = ModelType::All)]
        model_type: ModelType,
    },
    /// Test and debug engine behavior
    Test {
        /// Test mode
        #[arg(long, value_enum, default_value_t = TestMode::Candidates)]
        mode: TestMode,
        /// Input zhuyin text to test
        input: String,
    },
    /// Convert data formats
    Convert {
        /// Input file path
        #[arg(short, long)]
        input: std::path::PathBuf,
        /// Output file path  
        #[arg(short, long)]
        output: std::path::PathBuf,
        /// Format to convert to
        #[arg(long, value_enum)]
        format: ConvertFormat,
    }
}

#[derive(clap::ValueEnum, Clone)]
enum ModelType {
    All,
    Lexicon,
    Ngram,
    Userdict,
}

#[derive(clap::ValueEnum, Clone)]
enum TestMode {
    Candidates,
    Segmentation,
    Scoring,
}

#[derive(clap::ValueEnum, Clone)]
enum ConvertFormat {
    Fst,
    Redb,
    Bincode,
    Toml,
}

fn handle_build_command(input: &Path, output: &Path, model_type: ModelType) {
    println!("🔨 Building {} zhuyin models from {} to {}", 
        match model_type {
            ModelType::All => "all",
            ModelType::Lexicon => "lexicon",  
            ModelType::Ngram => "n-gram",
            ModelType::Userdict => "user dictionary",
        },
        input.display(), 
        output.display()
    );
    
    match model_type {
        ModelType::All => {
            println!("📚 Building zhuyin lexicon from corpus...");
            println!("📊 Computing zhuyin n-gram frequencies...");
            println!("👤 Initializing zhuyin user dictionary...");
            println!("✅ All zhuyin models built successfully!");
        }
        ModelType::Lexicon => {
            println!("📚 Building zhuyin lexicon only...");
        }
        ModelType::Ngram => {
            println!("📊 Building zhuyin n-gram model only...");
        }
        ModelType::Userdict => {
            println!("👤 Building zhuyin user dictionary only...");
        }
    }
    
    // TODO: Implement actual building logic
    println!("⚠️  Zhuyin model building not yet implemented - placeholder for Step 7");
}

fn handle_test_command(mode: TestMode, input: &str) {
    println!("🧪 Testing zhuyin {} mode with input: '{}'", 
        match mode {
            TestMode::Candidates => "candidates",
            TestMode::Segmentation => "segmentation", 
            TestMode::Scoring => "scoring",
        },
        input
    );
    
    let model = build_demo_model();
    let parser = ZhuyinParser::with_syllables(&[
        "ㄋㄧˇ", "ㄏㄠˇ", "ㄏㄠˋ", "ㄓㄨㄥ", "ㄍㄨㄛˊ"
    ]);
    
    match mode {
        TestMode::Candidates => {
            let engine = Engine::new(model, parser);
            let cands = engine.input(input);
            println!("📝 Generated {} candidates:", cands.len());
            for (i, c) in cands.iter().enumerate() {
                print_candidate(input, c, i);
            }
        }
        TestMode::Segmentation => {
            println!("🔍 Zhuyin segmentation analysis:");
            let segs = parser.segment_top_k(input, 3, true);
            for (i, seg) in segs.iter().enumerate() {
                println!("  {}. {:?}", i + 1, seg.iter().map(|s| &s.text).collect::<Vec<_>>());
            }
        }
        TestMode::Scoring => {
            let engine = Engine::new(model, parser);
            println!("📊 Detailed zhuyin scoring analysis:");
            let cands = engine.input(input);
            for (i, c) in cands.iter().enumerate().take(3) {
                println!("  {}. '{}' -> score: {:.4}", i + 1, c.text, c.score);
            }
        }
    }
}

fn handle_convert_command(input: &Path, output: &Path, format: ConvertFormat) {
    println!("🔄 Converting zhuyin {} to {} format -> {}",
        input.display(),
        match format {
            ConvertFormat::Fst => "FST",
            ConvertFormat::Redb => "redb",
            ConvertFormat::Bincode => "bincode", 
            ConvertFormat::Toml => "TOML",
        },
        output.display()
    );
    
    // TODO: Implement actual conversion logic
    println!("⚠️  Zhuyin format conversion not yet implemented - placeholder for Step 7");
}

fn main() {
    let cli = Cli::parse();
    
    match cli.command {
        Some(Commands::Repl) => {
            run_repl();
        }
        Some(Commands::Build { input, output, model_type }) => {
            handle_build_command(&input, &output, model_type);
        }
        Some(Commands::Test { mode, input }) => {
            handle_test_command(mode, &input);
        }
        Some(Commands::Convert { input, output, format }) => {
            handle_convert_command(&input, &output, format);
        }
        None => {
            // Legacy behavior: if just an input argument, treat as single input test
            if let Some(input) = cli.input {
                handle_test_command(TestMode::Candidates, &input);
            } else {
                // No arguments, start REPL
                run_repl();
            }
        }
    }
}

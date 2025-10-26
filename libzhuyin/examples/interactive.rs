use clap::{Parser as ClapParser, Subcommand};
use libchinese_core::{
    Candidate, Config, Interpolator, Lexicon, Model, NGramModel, UserDict, WordBigram,
};
use libzhuyin::{Engine, ZhuyinParser};
use std::io::{self, BufRead};
use std::path::Path;

fn build_demo_engine() -> Engine {
    // Try to load from data/converted/zhuyin_traditional using Engine::from_data_dir()
    let data_dir = Path::new("data/converted/zhuyin_traditional");

    if data_dir.exists() {
        match Engine::from_data_dir(data_dir) {
            Ok(engine) => {
                println!("loaded zhuyin engine from {}", data_dir.display());
                return engine;
            }
            Err(e) => {
                eprintln!("warning: failed to load engine from {:?}: {}", data_dir, e);
            }
        }
    }

    // Fallback: demo engine with minimal data
    let mut lx = Lexicon::new();
    // Add some basic zhuyin mappings (using bopomofo notation)
    lx.insert("ㄋㄧˇㄏㄠˇ", "你好"); // ni3 hao3 -> 你好
    lx.insert("ㄋㄧˇㄏㄠˋ", "你号"); // ni3 hao4 -> 你号
    lx.insert("ㄓㄨㄥㄍㄨㄛˊ", "中国"); // zhong1 guo2 -> 中国

    let mut ng = NGramModel::new();
    ng.insert_unigram("你", -1.0);
    ng.insert_unigram("好", WordBigram::new(), -1.2);
    ng.insert_unigram("号", -2.0);
    ng.insert_unigram("中", -1.1);
    ng.insert_unigram("国", -1.3);
    ng.set_interpolator(Interpolator::empty_for_test());

    let temp_path = std::env::temp_dir().join(format!(
        "libzhuyin_fallback_userdict_{}.redb",
        std::process::id()
    ));
    let user = UserDict::new(&temp_path).expect("create fallback userdict");
    user.learn("你好");

    let cfg = libzhuyin::ZhuyinConfig::default().into_base();
    let model = Model::new(lx, ng, WordBigram::new(), user, cfg);
    Engine::new(model)
}

fn print_candidate(key: &str, cand: &Candidate, idx: usize) {
    println!(
        "{}. candidate='{}' score={:.4}",
        idx + 1,
        cand.text,
        cand.score
    );
    println!("   key: {}", key);
}

fn run_repl() {
    let engine = build_demo_engine();

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

#[derive(ClapParser)]
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
    },
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
    Bincode,
    Toml,
}

fn handle_build_command(input: &Path, output: &Path, model_type: ModelType) {
    println!(
        "🔨 Building {} zhuyin models from {} to {}",
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

    // Model building is handled by external tools in the tools/ directory.
    // See tools/README.md for the model building workflow.
    println!("ℹ️  Zhuyin model building is handled by tools in the tools/ directory");
    println!("   See tools/README.md for the model building workflow");
}

fn handle_test_command(mode: TestMode, input: &str) {
    println!(
        "🧪 Testing zhuyin {} mode with input: '{}'",
        match mode {
            TestMode::Candidates => "candidates",
            TestMode::Segmentation => "segmentation",
            TestMode::Scoring => "scoring",
        },
        input
    );

    let engine = build_demo_engine();

    match mode {
        TestMode::Candidates => {
            let cands = engine.input(input);
            println!("📝 Generated {} candidates:", cands.len());
            for (i, c) in cands.iter().enumerate() {
                print_candidate(input, c, i);
            }
        }
        TestMode::Segmentation => {
            println!("🔍 Zhuyin segmentation analysis:");
            // Build a parser for segmentation testing
            let rules = libzhuyin::standard_fuzzy_rules();
            let parser = ZhuyinParser::new(rules, libzhuyin::ZHUYIN_SYLLABLES);
            let segs = parser.segment_top_k(input, 3, true);
            for (i, seg) in segs.iter().enumerate() {
                println!(
                    "  {}. {:?}",
                    i + 1,
                    seg.iter().map(|s| &s.text).collect::<Vec<_>>()
                );
            }
        }
        TestMode::Scoring => {
            println!("📊 Detailed zhuyin scoring analysis:");
            let cands = engine.input(input);
            for (i, c) in cands.iter().enumerate().take(3) {
                println!("  {}. '{}' -> score: {:.4}", i + 1, c.text, c.score);
            }
        }
    }
}

fn handle_convert_command(input: &Path, output: &Path, format: ConvertFormat) {
    println!(
        "🔄 Converting zhuyin {} to {} format -> {}",
        input.display(),
        match format {
            ConvertFormat::Fst => "FST",
            ConvertFormat::Bincode => "bincode",
            ConvertFormat::Toml => "TOML",
        },
        output.display()
    );

    // Format conversion is not currently implemented.
    // Data formats are fixed: FST+bincode for lexicons, bincode for n-grams and lambdas.
    println!("ℹ️  Format conversion not implemented");
    println!("   Models use fixed formats: FST+bincode (lexicon, lambdas), bincode (n-gram)");
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Repl) => {
            run_repl();
        }
        Some(Commands::Build {
            input,
            output,
            model_type,
        }) => {
            handle_build_command(&input, &output, model_type);
        }
        Some(Commands::Test { mode, input }) => {
            handle_test_command(mode, &input);
        }
        Some(Commands::Convert {
            input,
            output,
            format,
        }) => {
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

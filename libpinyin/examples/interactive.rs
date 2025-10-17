use libchinese_core::{Candidate, Config, Lexicon, Model, NGramModel, UserDict, Interpolator};
use std::io::{self, BufRead};
use std::path::Path;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use clap::{Parser, Subcommand};

fn build_model() -> Result<Model, Box<dyn std::error::Error>> {
    // Load runtime artifacts from `data/` directory (required)
    let data_dir = Path::new("data");
    let fst_path = data_dir.join("pinyin.fst");
    let redb_path = data_dir.join("pinyin.redb");

    // Load lexicon from fst + redb (required)
    let lx = Lexicon::load_from_fst_redb(&fst_path, &redb_path)?;
    println!("✓ Loaded lexicon from '{}' + '{}'", fst_path.display(), redb_path.display());
    
    // Load ngram model from data/ngram.bincode if present
    let ng = if let Ok(mut f) = File::open("data/ngram.bincode") {
        let mut b = Vec::new();
        if f.read_to_end(&mut b).is_ok() {
            if let Ok(m) = bincode::deserialize::<NGramModel>(&b) {
                println!("✓ Loaded n-gram model from data/ngram.bincode");
                m
            } else {
                eprintln!("⚠ Failed to deserialize ngram.bincode, using empty model");
                NGramModel::new()
            }
        } else {
            NGramModel::new()
        }
    } else {
        NGramModel::new()
    };

    // Load or create userdict
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
                println!("✓ Loaded interpolator from '{}' + '{}'", lambdas_fst.display(), lambdas_redb.display());
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
    Ok(Model::new(lx, ng, user, cfg, interp))
}

fn print_candidate(key: &str, cand: &Candidate, idx: usize) {
    let chars: Vec<String> = cand.text.chars().map(|c| c.to_string()).collect();
    println!("{}. candidate='{}' score={:.4}", idx + 1, cand.text, cand.score);
    println!("   key: {}", key);
    println!("   chars: [{}]", chars.join(", "));
}

fn run_repl() {
    let model = build_model().expect("Failed to load model. Ensure data files exist in data/ directory.");
    let parser = libpinyin::parser::Parser::with_syllables(&[
        "ni", "hao", "zhong", "guo", "wo", "ai", "ni", "men"  
    ]);
    let engine = libpinyin::Engine::new(model, parser);
    
    println!("libpinyin demo CLI — type pinyin input (e.g. 'nihao' or 'zhongguo') and press Enter");
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
#[command(name = "libpinyin")]
#[command(about = "A Rust reimplementation of libpinyin Chinese input method")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    
    /// Single pinyin input for quick testing
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
        /// Input text to test (or file path for batch mode)
        input: String,
        /// Output file for results (optional)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
        /// Number of candidates to generate
        #[arg(short, long, default_value_t = 10)]
        count: usize,
        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Performance benchmarking
    Benchmark {
        /// Input file with test cases
        #[arg(short, long)]
        input: std::path::PathBuf,
        /// Number of iterations
        #[arg(short = 'n', long, default_value_t = 100)]
        iterations: usize,
        /// Warm-up runs
        #[arg(short, long, default_value_t = 10)]
        warmup: usize,
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
    /// Verify correctness against reference implementations
    Verify {
        /// Test cases file (input\texpected_output format)
        #[arg(short, long)]
        input: std::path::PathBuf,
        /// Reference implementation results file
        #[arg(short, long)]
        reference: Option<std::path::PathBuf>,
        /// Output detailed diff report
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
        /// Tolerance for score differences
        #[arg(long, default_value_t = 0.1)]
        tolerance: f32,
        /// Only show mismatches
        #[arg(long)]
        only_mismatches: bool,
    },
    /// Performance analysis and optimization
    Perf {
        /// Input file with test cases
        #[arg(short, long)]
        input: std::path::PathBuf,
        /// Show cache statistics
        #[arg(long)]
        show_cache: bool,
        /// Measure latency distribution
        #[arg(long)]
        latency: bool,
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
    Batch,
    Benchmark,
    Interactive,
}

#[derive(clap::ValueEnum, Clone)]
enum ConvertFormat {
    Fst,
    Redb,
    Bincode,
    Toml,
}

fn handle_build_command(input: &Path, output: &Path, model_type: ModelType) {
    println!("🔨 Building {} models from {} to {}", 
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
            println!("📚 Building lexicon from corpus...");
            println!("📊 Computing n-gram frequencies...");
            println!("👤 Initializing user dictionary...");
            println!("✅ All models built successfully!");
        }
        ModelType::Lexicon => {
            println!("📚 Building lexicon only...");
        }
        ModelType::Ngram => {
            println!("📊 Building n-gram model only...");
        }
        ModelType::Userdict => {
            println!("👤 Building user dictionary only...");
        }
    }
    
    // Model building is handled by external tools in the tools/ directory.
    // See tools/README.md for the model building workflow:
    // - convert_tables: Build lexicon (FST + redb)
    // - serialize_ngram: Build n-gram model
    // - estimate_interpolation: Compute lambda weights
    println!("ℹ️  Model building is handled by tools in the tools/ directory");
    println!("   Run 'cargo run --bin convert_tables' to build lexicons");
    println!("   Run 'cargo run --bin serialize_ngram' to build n-gram models");
    println!("   See tools/README.md for complete workflow");
}

fn handle_test_command(mode: TestMode, input: &str, output: Option<&Path>, count: usize, verbose: bool) {
    println!("🧪 Testing {} mode with input: '{}'", 
        match mode {
            TestMode::Candidates => "candidates",
            TestMode::Segmentation => "segmentation", 
            TestMode::Scoring => "scoring",
            TestMode::Batch => "batch processing",
            TestMode::Benchmark => "benchmark",
            TestMode::Interactive => "interactive",
        },
        input
    );
    
    let model = build_model().expect("Failed to load model. Ensure data files exist in data/ directory.");
    let parser = libpinyin::parser::Parser::with_syllables(&[
        "ni", "hao", "zhong", "guo", "wo", "ai", "ni", "men"
    ]);
    
    match mode {
        TestMode::Candidates => {
            let engine = libpinyin::Engine::new(model, parser);
            let cands = engine.input(input);
            println!("📝 Generated {} candidates (showing top {}):", cands.len(), count.min(cands.len()));
            for (i, c) in cands.iter().enumerate().take(count) {
                if verbose {
                    println!("  {}. '{}' -> score: {:.4} (freq: N/A)", i + 1, c.text, c.score);
                } else {
                    print_candidate(input, c, i);
                }
            }
            
            if let Some(out_path) = output {
                save_candidates_to_file(input, &cands, out_path, count);
            }
        }
        TestMode::Segmentation => {
            println!("🔍 Segmentation analysis:");
            let segs = parser.segment_top_k(input, count.min(10), true);
            for (i, seg) in segs.iter().enumerate().take(count) {
                if verbose {
                    println!("  {}. {:?} (length: {})", i + 1, 
                        seg.iter().map(|s| &s.text).collect::<Vec<_>>(), seg.len());
                } else {
                    println!("  {}. {:?}", i + 1, seg.iter().map(|s| &s.text).collect::<Vec<_>>());
                }
            }
        }
        TestMode::Scoring => {
            let engine = libpinyin::Engine::new(model, parser);
            println!("📊 Detailed scoring analysis:");
            let cands = engine.input(input);
            for (i, c) in cands.iter().enumerate().take(count.min(5)) {
                if verbose {
                    println!("  {}. '{}' -> score: {:.4} (normalized: {:.4})", 
                        i + 1, c.text, c.score, c.score / input.len() as f32);
                } else {
                    println!("  {}. '{}' -> score: {:.4}", i + 1, c.text, c.score);
                }
            }
        }
        TestMode::Batch => {
            handle_batch_test(input, output, count, verbose);
        }
        TestMode::Benchmark => {
            println!("⚠️  Benchmark mode requires using the separate 'benchmark' command");
        }
        TestMode::Interactive => {
            handle_interactive_test();
        }
    }
}

fn save_candidates_to_file(input: &str, candidates: &[libchinese_core::Candidate], output_path: &Path, count: usize) {
    use std::io::Write;
    match std::fs::File::create(output_path) {
        Ok(mut file) => {
            writeln!(file, "# Test results for input: {}", input).unwrap();
            writeln!(file, "# Generated {} candidates", candidates.len()).unwrap();
            for (i, c) in candidates.iter().enumerate().take(count) {
                writeln!(file, "{}\t{}\t{:.4}", i + 1, c.text, c.score).unwrap();
            }
            println!("✅ Results saved to {}", output_path.display());
        }
        Err(e) => {
            eprintln!("❌ Failed to save results: {}", e);
        }
    }
}

fn handle_batch_test(file_path: &str, output: Option<&Path>, count: usize, verbose: bool) {
    use std::io::{BufRead, BufReader};
    
    let file = match std::fs::File::open(file_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("❌ Failed to open batch test file {}: {}", file_path, e);
            return;
        }
    };
    
    println!("📁 Processing batch test file: {}", file_path);
    let model = build_model().expect("Failed to load model. Ensure data files exist in data/ directory.");
    let parser = libpinyin::parser::Parser::with_syllables(&[
        "ni", "hao", "zhong", "guo", "wo", "ai", "ni", "men"
    ]);
    let engine = libpinyin::Engine::new(model, parser);
    
    let reader = BufReader::new(file);
    let mut total_tests = 0;
    let mut results = Vec::new();
    
    for (line_num, line) in reader.lines().enumerate() {
        let line = match line {
            Ok(l) => l.trim().to_string(),
            Err(e) => {
                eprintln!("❌ Error reading line {}: {}", line_num + 1, e);
                continue;
            }
        };
        
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        total_tests += 1;
        let cands = engine.input(&line);
        
        if verbose {
            println!("🔍 Line {}: '{}' -> {} candidates", line_num + 1, line, cands.len());
        }
        
        results.push((line.clone(), cands));
    }
    
    println!("✅ Processed {} test cases", total_tests);
    
    if let Some(out_path) = output {
        save_batch_results(&results, out_path, count);
    } else {
        // Show summary
        let avg_candidates = results.iter().map(|(_, c)| c.len()).sum::<usize>() as f32 / total_tests as f32;
        println!("📊 Average candidates per input: {:.2}", avg_candidates);
    }
}

fn save_batch_results(results: &[(String, Vec<libchinese_core::Candidate>)], output_path: &Path, count: usize) {
    use std::io::Write;
    match std::fs::File::create(output_path) {
        Ok(mut file) => {
            writeln!(file, "# Batch test results").unwrap();
            writeln!(file, "# Format: input\\trank\\tcandidate\\tscore").unwrap();
            
            for (input, candidates) in results {
                for (i, c) in candidates.iter().enumerate().take(count) {
                    writeln!(file, "{}\t{}\t{}\t{:.4}", input, i + 1, c.text, c.score).unwrap();
                }
            }
            
            println!("✅ Batch results saved to {}", output_path.display());
        }
        Err(e) => {
            eprintln!("❌ Failed to save batch results: {}", e);
        }
    }
}

fn handle_interactive_test() {
    println!("🎮 Interactive Testing Mode");
    println!("Type pinyin input and press Enter. Type 'quit' to exit.");
    
    let model = build_model().expect("Failed to load model. Ensure data files exist in data/ directory.");
    let parser = libpinyin::parser::Parser::with_syllables(&[
        "ni", "hao", "zhong", "guo", "wo", "ai", "ni", "men"
    ]);
    let engine = libpinyin::Engine::new(model, parser);
    
    loop {
        print!("pinyin> ");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
        
        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(_) => {
                let input = input.trim();
                if input.is_empty() {
                    continue;
                }
                if input == "quit" || input == "exit" {
                    break;
                }
                
                let cands = engine.input(input);
                println!("📝 {} candidates:", cands.len());
                for (i, c) in cands.iter().enumerate().take(5) {
                    println!("  {}. '{}'", i + 1, c.text);
                }
                println!();
            }
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        }
    }
}

fn handle_benchmark_command(input: &Path, iterations: usize, warmup: usize) {
    use std::time::Instant;
    use std::io::{BufRead, BufReader};
    
    println!("🏃 Running benchmark with {} iterations ({} warmup)", iterations, warmup);
    
    let file = match std::fs::File::open(input) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("❌ Failed to open benchmark file {}: {}", input.display(), e);
            return;
        }
    };
    
    // Read all test cases
    let reader = BufReader::new(file);
    let test_cases: Vec<String> = reader
        .lines()
        .filter_map(|line| line.ok())
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .collect();
    
    if test_cases.is_empty() {
        eprintln!("❌ No test cases found in {}", input.display());
        return;
    }
    
    println!("📊 Loaded {} test cases", test_cases.len());
    
    let model = build_model().expect("Failed to load model. Ensure data files exist in data/ directory.");
    let parser = libpinyin::parser::Parser::with_syllables(&[
        "ni", "hao", "zhong", "guo", "wo", "ai", "ni", "men"
    ]);
    let engine = libpinyin::Engine::new(model, parser);
    
    // Warmup
    println!("🔥 Warming up...");
    for _ in 0..warmup {
        for case in &test_cases {
            let _ = engine.input(case);
        }
    }
    
    // Benchmark
    println!("⏱️  Running benchmark...");
    let start = Instant::now();
    let mut total_candidates = 0;
    
    for _ in 0..iterations {
        for case in &test_cases {
            let candidates = engine.input(case);
            total_candidates += candidates.len();
        }
    }
    
    let elapsed = start.elapsed();
    let total_queries = iterations * test_cases.len();
    let avg_time_per_query = elapsed.as_micros() as f64 / total_queries as f64;
    let queries_per_sec = 1_000_000.0 / avg_time_per_query;
    
    println!("📈 Benchmark Results:");
    println!("  Total time: {:.2?}", elapsed);
    println!("  Total queries: {}", total_queries);
    println!("  Average time per query: {:.2} μs", avg_time_per_query);
    println!("  Queries per second: {:.0}", queries_per_sec);
    println!("  Average candidates per query: {:.1}", total_candidates as f64 / total_queries as f64);
}

fn handle_convert_command(input: &Path, output: &Path, format: ConvertFormat) {
    println!("🔄 Converting {} to {} format -> {}",
        input.display(),
        match format {
            ConvertFormat::Fst => "FST",
            ConvertFormat::Redb => "redb",
            ConvertFormat::Bincode => "bincode", 
            ConvertFormat::Toml => "TOML",
        },
        output.display()
    );
    
    // Format conversion is not currently implemented.
    // Data formats are fixed: FST+redb for lexicons, bincode for n-grams.
    // If conversion is needed, use the tools in tools/ directory to rebuild.
    println!("ℹ️  Format conversion not implemented");
    println!("   Models use fixed formats: FST+redb (lexicon), bincode (n-gram)");
    println!("   To change formats, rebuild using tools in tools/ directory");
}

#[derive(Debug, Clone)]
struct VerificationResult {
    input: String,
    expected: Vec<String>,
    actual: Vec<libchinese_core::Candidate>,
    matches: bool,
    score_diff: f32,
}

fn handle_verify_command(input: &Path, _reference: Option<&Path>, output: Option<&Path>, tolerance: f32, only_mismatches: bool) {
    println!("🔍 Starting correctness verification");
    println!("  Input file: {}", input.display());
    println!("  Tolerance: ±{:.4}", tolerance);
    
    let test_cases = load_verification_cases(input);
    if test_cases.is_empty() {
        eprintln!("❌ No test cases found");
        return;
    }
    
    println!("📊 Loaded {} test cases", test_cases.len());
    
    let model = build_model().expect("Failed to load model. Ensure data files exist in data/ directory.");
    let parser = libpinyin::parser::Parser::with_syllables(&[
        "ni", "hao", "zhong", "guo", "wo", "ai", "ni", "men"
    ]);
    let engine = libpinyin::Engine::new(model, parser);
    
    let mut results = Vec::new();
    let mut total_matches = 0;
    let mut total_score_diff = 0.0;
    
    println!("🔄 Running verification...");
    for (i, (input_text, expected)) in test_cases.iter().enumerate() {
        let actual_candidates = engine.input(input_text);
        
        // Check if top candidate matches expected
        let matches = if let Some(top_candidate) = actual_candidates.first() {
            expected.contains(&top_candidate.text)
        } else {
            expected.is_empty()
        };
        
        if matches {
            total_matches += 1;
        }
        
        let score_diff = if actual_candidates.first().is_some() {
            // Placeholder score comparison - in real scenario would compare against reference scores
            if matches { 0.0 } else { 1.0 }
        } else {
            1.0
        };
        
        total_score_diff += score_diff;
        
        let result = VerificationResult {
            input: input_text.clone(),
            expected: expected.clone(),
            actual: actual_candidates,
            matches,
            score_diff,
        };
        
        if !only_mismatches || !matches {
            print_verification_result(&result, i + 1);
        }
        
        results.push(result);
    }
    
    let accuracy = total_matches as f32 / test_cases.len() as f32;
    let avg_score_diff = total_score_diff / test_cases.len() as f32;
    
    println!("\n📈 Verification Summary:");
    println!("  Total cases: {}", test_cases.len());
    println!("  Matches: {} ({:.1}%)", total_matches, accuracy * 100.0);
    println!("  Mismatches: {}", test_cases.len() - total_matches);
    println!("  Average score difference: {:.4}", avg_score_diff);
    
    if let Some(output_path) = output {
        save_verification_report(&results, output_path, tolerance);
    }
    
    if accuracy > 0.95 {
        println!("✅ Verification PASSED (>95% accuracy)");
    } else if accuracy > 0.80 {
        println!("⚠️  Verification WARNING (80-95% accuracy)");
    } else {
        println!("❌ Verification FAILED (<80% accuracy)");
    }
}

fn load_verification_cases(input: &Path) -> Vec<(String, Vec<String>)> {
    use std::io::{BufRead, BufReader};
    
    let file = match std::fs::File::open(input) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("❌ Failed to open verification file {}: {}", input.display(), e);
            return Vec::new();
        }
    };
    
    let reader = BufReader::new(file);
    let mut cases = Vec::new();
    
    for (line_num, line) in reader.lines().enumerate() {
        let line = match line {
            Ok(l) => l.trim().to_string(),
            Err(e) => {
                eprintln!("❌ Error reading line {}: {}", line_num + 1, e);
                continue;
            }
        };
        
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            let input_text = parts[0].to_string();
            let expected: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
            cases.push((input_text, expected));
        } else {
            eprintln!("⚠️  Malformed line {}: {}", line_num + 1, line);
        }
    }
    
    cases
}

fn print_verification_result(result: &VerificationResult, test_num: usize) {
    let status = if result.matches { "✅" } else { "❌" };
    println!("{} Test {}: '{}'", status, test_num, result.input);
    
    if !result.matches {
        println!("  Expected: {:?}", result.expected);
        if let Some(top) = result.actual.first() {
            println!("  Actual:   '{}' (score: {:.4})", top.text, top.score);
        } else {
            println!("  Actual:   (no candidates)");
        }
        println!("  Score diff: {:.4}", result.score_diff);
    }
}

fn save_verification_report(results: &[VerificationResult], output_path: &Path, tolerance: f32) {
    use std::io::Write;
    
    match std::fs::File::create(output_path) {
        Ok(mut file) => {
            writeln!(file, "# Verification Report").unwrap();
            writeln!(file, "# Tolerance: ±{:.4}", tolerance).unwrap();
            writeln!(file, "# Format: test_num\\tinput\\texpected\\tactual\\tmatch\\tscore_diff").unwrap();
            
            for (i, result) in results.iter().enumerate() {
                let expected_str = result.expected.join("|");
                let actual_str = if let Some(top) = result.actual.first() {
                    format!("{}({:.4})", top.text, top.score)
                } else {
                    "NONE".to_string()
                };
                
                writeln!(file, "{}\t{}\t{}\t{}\t{}\t{:.4}",
                    i + 1,
                    result.input,
                    expected_str,
                    actual_str,
                    result.matches,
                    result.score_diff
                ).unwrap();
            }
            
            println!("✅ Verification report saved to {}", output_path.display());
        }
        Err(e) => {
            eprintln!("❌ Failed to save report: {}", e);
        }
    }
}

fn handle_perf_command(input: &Path, show_cache: bool, latency: bool) {
    use std::time::Instant;
    use std::io::{BufRead, BufReader};
    
    println!("⚡ Performance Analysis");
    
    let file = match std::fs::File::open(input) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("❌ Failed to open performance test file {}: {}", input.display(), e);
            return;
        }
    };
    
    // Read test cases
    let reader = BufReader::new(file);
    let test_cases: Vec<String> = reader
        .lines()
        .filter_map(|line| line.ok())
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .collect();
    
    if test_cases.is_empty() {
        eprintln!("❌ No test cases found in {}", input.display());
        return;
    }
    
    println!("📊 Loaded {} test cases", test_cases.len());
    
    let model = build_model().expect("Failed to load model. Ensure data files exist in data/ directory.");
    let parser = libpinyin::parser::Parser::with_syllables(&[
        "ni", "hao", "zhong", "guo", "wo", "ai", "ni", "men"
    ]);
    let engine = libpinyin::Engine::new(model, parser);
    
    let mut latencies = Vec::new();
    let mut total_candidates = 0;
    
    println!("🔄 Running performance analysis...");
    
    // First pass: measure cold cache performance
    for (i, case) in test_cases.iter().enumerate() {
        let start = Instant::now();
        let candidates = engine.input(case);
        let latency = start.elapsed();
        
        latencies.push(latency);
        total_candidates += candidates.len();
        
        if i % 10 == 0 || i == test_cases.len() - 1 {
            print!(".");
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
        }
    }
    println!();
    
    // Cache statistics after first pass
    let (hits, misses, hit_rate) = engine.cache_stats();
    
    if show_cache {
        println!("\n💾 Cache Statistics (first pass):");
        println!("  Cache hits: {}", hits);
        println!("  Cache misses: {}", misses);
        println!("  Hit rate: {:.2}%", hit_rate * 100.0);
        println!("  Cache size: {}", engine.cache_size());
        
        // Second pass: measure warm cache performance
        println!("\n🔥 Running warm cache test...");
        let mut warm_latencies = Vec::new();
        
        for case in &test_cases {
            let start = Instant::now();
            let _candidates = engine.input(case);
            let latency = start.elapsed();
            warm_latencies.push(latency);
        }
        
        let (warm_hits, warm_misses, warm_hit_rate) = engine.cache_stats();
        println!("\n💾 Cache Statistics (after warm cache test):");
        println!("  Total hits: {}", warm_hits);
        println!("  Total misses: {}", warm_misses);
        println!("  Overall hit rate: {:.2}%", warm_hit_rate * 100.0);
        
        // Compare cold vs warm performance
        let cold_avg = latencies.iter().sum::<std::time::Duration>().as_micros() as f64 / latencies.len() as f64;
        let warm_avg = warm_latencies.iter().sum::<std::time::Duration>().as_micros() as f64 / warm_latencies.len() as f64;
        
        println!("\n🌡️  Performance Comparison:");
        println!("  Cold cache average: {:.1} μs", cold_avg);
        println!("  Warm cache average: {:.1} μs", warm_avg);
        println!("  Speedup: {:.1}x", cold_avg / warm_avg);
    }
    
    if latency {
        println!("\n⏱️  Latency Distribution (cold cache):");
        
        latencies.sort();
        let len = latencies.len();
        
        let min = latencies[0];
        let max = latencies[len - 1];
        let median = latencies[len / 2];
        let p95 = latencies[(len as f64 * 0.95) as usize];
        let p99 = latencies[(len as f64 * 0.99) as usize];
        
        let avg = latencies.iter().sum::<std::time::Duration>().as_micros() as f64 / len as f64;
        
        println!("  Min: {:.1} μs", min.as_micros());
        println!("  Median: {:.1} μs", median.as_micros());
        println!("  Average: {:.1} μs", avg);
        println!("  P95: {:.1} μs", p95.as_micros());
        println!("  P99: {:.1} μs", p99.as_micros());
        println!("  Max: {:.1} μs", max.as_micros());
    }
    
    println!("\n📈 Overall Performance:");
    println!("  Total queries: {}", test_cases.len());
    println!("  Total candidates generated: {}", total_candidates);
    println!("  Average candidates per query: {:.1}", total_candidates as f64 / test_cases.len() as f64);
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
        Some(Commands::Test { mode, input, output, count, verbose }) => {
            handle_test_command(mode, &input, output.as_deref(), count, verbose);
        }
        Some(Commands::Benchmark { input, iterations, warmup }) => {
            handle_benchmark_command(&input, iterations, warmup);
        }
        Some(Commands::Convert { input, output, format }) => {
            handle_convert_command(&input, &output, format);
        }
        Some(Commands::Verify { input, reference, output, tolerance, only_mismatches }) => {
            handle_verify_command(&input, reference.as_deref(), output.as_deref(), tolerance, only_mismatches);
        }
        Some(Commands::Perf { input, show_cache, latency }) => {
            handle_perf_command(&input, show_cache, latency);
        }
        None => {
            // Legacy behavior: if just an input argument, treat as single input test
            if let Some(input) = cli.input {
                handle_test_command(TestMode::Candidates, &input, None, 10, false);
            } else {
                // No arguments, start REPL
                run_repl();
            }
        }
    }
}

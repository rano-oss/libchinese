mod convert_table;
mod convert_interpolation;

use clap::Parser;
use std::path::PathBuf;
use anyhow::Result;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    #[arg(long, num_args=1..)]
    inputs: Vec<PathBuf>,

    #[arg(long)]
    id_map: Option<PathBuf>,
    #[arg(long)]
    phrase_fst: Option<PathBuf>,
    #[arg(long)]
    phrase_redb: Option<PathBuf>,

    #[arg(long, default_value = "lexicon.fst")]
    out_fst: PathBuf,

    #[arg(long, default_value = "lexicon.redb")]
    out_redb: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // If a single input and it's a .text file, treat it as interpolation-style.
    if args.inputs.len() == 1 {
        let input_ext = args.inputs[0].extension().and_then(|s| s.to_str()).unwrap_or("");
        if input_ext == "text" {
            convert_interpolation::run(&args.inputs[0], &args.id_map, &args.phrase_fst, &args.phrase_redb, &args.out_fst, &args.out_redb)?;
            println!("Wrote fst to {} and redb to {}", args.out_fst.display(), args.out_redb.display());
            return Ok(());
        }
    }

    // Otherwise treat inputs as one or more table files to be merged into a single fst+redb
    convert_table::run(&args.inputs, &args.out_fst, &args.out_redb)?;

    println!("Wrote fst to {} and redb to {}", args.out_fst.display(), args.out_redb.display());
    Ok(())
}

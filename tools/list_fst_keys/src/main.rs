use fst::Map;
use fst::Streamer;
use std::fs::File;
use std::io::Read;

fn main() -> anyhow::Result<()> {
    let mut f = File::open("data/pinyin.fst")?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let map = Map::new(buf)?;
    let mut stream = map.stream();
    use std::collections::HashMap;
    let mut stem_counts: HashMap<String, usize> = HashMap::new();
    let mut stem_samples: HashMap<String, Vec<String>> = HashMap::new();
    let mut total = 0usize;
    let mut found_nihao = Vec::new();
    let mut found_merged = 0usize;
    let mut found_pinyin = 0usize;
    while let Some((k, _v)) = stream.next() {
        if let Ok(s) = std::str::from_utf8(k) {
            let parts: Vec<&str> = s.splitn(2, '\t').collect();
            let stem = parts.get(0).map(|p| p.to_string()).unwrap_or_default();
            *stem_counts.entry(stem.clone()).or_default() += 1;
            let samples = stem_samples.entry(stem.clone()).or_default();
            if samples.len() < 5 {
                samples.push(s.to_string());
            }
            if parts.len() == 2 {
                let key = parts[1];
                if key == "nihao" || key.contains("nihao") {
                    found_nihao.push(s.to_string());
                }
                if parts[0] == "merged" { found_merged += 1; }
                if parts[0] == "pinyin" { found_pinyin += 1; }
            }
        }
        total += 1;
        // scan all keys
    }

    // print top stems
    let mut stems: Vec<(String, usize)> = stem_counts.into_iter().collect();
    stems.sort_by(|a, b| b.1.cmp(&a.1));
    println!("Total keys inspected: {}", total);
    println!("Top stems:");
    for (stem, cnt) in stems.iter().take(20) {
        println!("  {} -> {} keys", stem, cnt);
        if let Some(samps) = stem_samples.get(stem) {
            for s in samps.iter() {
                // show RHS if composite
                let parts: Vec<&str> = s.splitn(2, '\t').collect();
                if parts.len() == 2 {
                    println!("    sample: {} -> rhs='{}'", s, parts[1]);
                } else {
                    println!("    sample: {}", s);
                }
            }
        }
    }

    println!("\nAll stems:");
    for (stem, _) in stems.iter() {
        println!("  {}", stem);
    }

    println!("\nFound nihao occurrences: {}", found_nihao.len());
    for k in found_nihao.iter().take(10) { println!("  {}", k); }
    println!("merged stem count: {}", found_merged);
    println!("pinyin stem count: {}", found_pinyin);
    Ok(())
}

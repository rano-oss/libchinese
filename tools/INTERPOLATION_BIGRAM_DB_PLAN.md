# Plan: Reconstruct upstream bigram DB and wire to estimator

Goal
- Reproduce the upstream bigram DB construction (the data structures used by the C++ training tools) from the provided `.table` files and related lexicon artifacts, so the Rust estimator can use the same global bigram totals and per-right frequencies when computing interpolation lambdas.
- Make the implementation auditable, testable, and incremental: first create a reproducible DB builder, then adapt the Rust estimator to optionally consume that DB for the global-normalization path.

Why
- The estimator has two normalization modes: per-left conditional (P(r|l)) and a global bigram normalization used upstream (P_bigram(r) = freq(r) / total_bigram_counts).
- We implemented per-left normalization already. To match upstream numerics exactly we must reconstruct the same global bigram totals and right-token frequencies that upstream used when computing P_bigram.

Contract (inputs / outputs / error modes)
- Inputs:
  - One or more `.table` files (plain text) that encode phrase/token frequency lists and bigram information. Expected to be in the repository `data/` tree (e.g., `data/*.table`, `data/zhuyin/*.table`, `data/addon/*.table`).
  - Optional phrase/id mappings (existing redb `phrases` DB) if table rows use numeric ids instead of surface text.
- Outputs:
  - A serializable bigram DB file containing at least:
    - total_bigram_counts: u64 (sum of all bigram counts considered)
    - bigram_right_freqs: Map<u64, u64> mapping right-token-id -> total count across all left keys
    - Optionally: left->(right->count) materialized or stored separately (redb/fst) for other offline checks
  - Format: choose a simple, compact, versioned binary format using `bincode` + `serde` with a small header struct (allows future extension). Also provide an optional JSON dump for inspection.
- Error modes:
  - Missing/ill-formed `.table` files: error with clear message listing which files failed and file line numbers when parse errors occur.
  - ID mapping missing: if tables use numeric ids but no `phrases` mapping is available, fail or emit a warning and proceed with id-only DB.
  - Overflow: if totals exceed u64, fail (unlikely) or switch to f64 accumulation — decide based on observed totals.

High-level approach
1. Discover `.table` files to ingest.
   - Accept CLI arguments: either a list of files or a glob pattern (e.g., `data/*.table`).
   - If none provided, fall back to conventional locations used in this repo: `data/*.table`, `data/zhuyin/*.table`, `data/addon/*.table`.
2. Parse each `.table` file.
   - Detect file format variants and support the minimal superset observed in repo data.
   - Expect lines with either: `id freq` or `phrase freq` or `left_id right_id freq` depending on the table type. Use heuristics: number of space-separated tokens and whether the first token parses as u64.
   - For bigram-like files (contain 3 fields), treat as left,right,count entries and accumulate both per-left and per-right counts.
   - For unigram-like files (2 fields), treat as right/count data and add to global right frequencies.
3. Build data structures while parsing (in-memory):
   - bigram_right_freqs: HashMap<u64, u128> (accumulate in u128 to be safe), later downcast to u64 or stored as u128 in the file.
   - total_bigram_counts: u128
   - Optionally collect left->(right->count) maps (HashMap<u64, HashMap<u64, u64>>) if we want to produce a complete bigram DB used elsewhere.
4. Serialize DB to disk.
   - Use a stable struct with a version field and `serde` + `bincode` for compactness. Also emit a human-readable `.json` for debugging.
5. Wire into estimator.
   - Add CLI option to `tools/bin/estimate_interpolation.rs`: `--bigram-db <path>`.
   - If provided, load the DB and use `bigram_right_freqs` + `total_bigram_counts` to compute global P_bigram(r). Estimator should support both modes (per-left conditional and global bigram) via CLI flag `--mode {left|global}` or autodetection.
6. Tests & verification.
   - Unit tests for the table parser covering discovered table variants.
   - Small integration test: build a DB from a small hand-crafted set, run the estimator in both modes and confirm numeric differences.
   - Smoke test on the real `data/*.table` files to create a DB and run the estimator; compare key statistics with the previous per-left run (e.g., counts, non-empty lambdas).

Edge cases
- Tables with phrase text instead of numeric ids: parse but maintain mapping to text keys; allow exporter to map phrase->id via existing `phrases.redb` if the estimator/converter chain needs ids.
- Empty tables: emit an empty DB with zero totals and exit with a helpful message.
- Mixed formats in one file: attempt to parse lines leniently, skip malformed lines with warnings and a counter.

Files to add / edit (implementation roadmap)
- New bin: `tools/src/bin/build_bigram_db.rs` — CLI that builds and writes the DB.
- New module: `tools/src/bigram_db.rs` — parsing helpers, data structs, serde types, and small utilities.
- Edit: `tools/Cargo.toml` — add `bincode` and ensure `serde`/`serde_derive` are available for the `tools` crate.
- Edit: `tools/src/bin/estimate_interpolation.rs` — add `--bigram-db` and `--mode` flags; load DB when requested and compute P_bigram accordingly.
- Tests: `tools/tests/` — unit tests for parser and serialization.

Quality gates / verification
- Build: `cargo build --manifest-path tools/Cargo.toml` (should pass without warnings or unused deps).
- Lint/format: `cargo fmt` and `cargo clippy` run and fix obvious issues.
- Tests: `cargo test -p convert_tables` or appropriate package tests.
- Smoke run: run build_bigram_db on real tables, inspect generated `bigram_db.{bin,json}`, then run estimator with `--bigram-db` and `--mode global` to produce `interpolation2.estimated.global.txt`.

Performance and memory
- We'll stream-parse large table files and only keep essential aggregates in memory (right-side frequency map and optionally per-left maps). For extremely large corpora, the right-side map size should fit in memory (unique token count). Use u128 for safe accumulation then downcast on write.

Deliverables
- `tools/INTERPOLATION_BIGRAM_DB_PLAN.md` (this file)
- `tools/src/bigram_db.rs` (parsing + types)
- `tools/src/bin/build_bigram_db.rs` (CLI writer)
- `tools/src/bin/estimate_interpolation.rs` (small edits to accept DB)
- `tools/tests/*` (parser + integration tests)

Timeline (estimate)
- Plan & small design doc: saved (now).
- Implementation of parsing + DB serialization: 1-2 work hours.
- Wiring estimator and tests: 1-2 hours.
- Smoke runs on repo data and minor tuning: 30–60 minutes.

Next actions I will take (once you confirm):
1. Implement `tools/src/bigram_db.rs` + `tools/src/bin/build_bigram_db.rs` per the plan.
2. Add CLI options to `estimate_interpolation.rs` to load the DB and run in `global` mode.
3. Run tests and smoke-run on repo `data/` files and report results (file sizes and a few sample token lambdas).

Questions / assumptions
- I assume the `.table` files present in `data/` follow one of the simple formats (unigram or bigram rows). If you have an authoritative spec or upstream helper code snippets, share them and I will align parsing to that.
- I assume token ids in `.table` files correspond to the same id space used in `interpolation2.text`. If not, we'll need an id->phrase mapping step; the plan supports using the existing `phrases.redb` mapping.

If this plan looks good I’ll implement it now and produce the DB builder plus estimator wiring.

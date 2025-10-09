# Phase 2 — Detailed mapping: libpinyin (C++) → libchinese (Rust)

This document records a one-to-one mapping of relevant files and subsystems in the upstream `libpinyin` C++ repository to the planned Rust modules in the `libchinese` workspace. It focuses on the algorithmic responsibilities to preserve during the port: pinyin/zhuyin segmentation, fuzzy matching, n-gram training/scoring, phrase/lexicon lookup, and user-learning semantics. Where helpful, the mapping references the exact upstream paths found in the public repo.

Goals
- Identify the canonical C++ sources that implement each algorithm.
- Propose Rust module targets (crate + path) that mirror responsibilities.
- Capture algorithmic notes, migration hints, and test vectors to port.
- Prioritize pieces for an iterative port (skeleton → tests → implementation → optimization).

Top-level proposed Rust layout
- `core/` (crate: `libchinese-core`)
  - `src/ngram.rs` — `NGramModel` training/score primitives
  - `src/lexicon.rs` — `Lexicon` abstraction + model IO
  - `src/userdict.rs` — `UserDict` trait + `redb` backend
  - `src/config.rs` — `Config` (serde/TOML), fuzzy flags & weights
  - `src/model.rs` — `Model` container to bundle lexicon/ngram/userdict/config
  - `src/utils.rs` — helpers (normalization, tokenization, math)
- `libpinyin/` (crate)
  - `src/parser.rs` — pinyin segmentation (trie + DP)
  - `src/fuzzy.rs` — fuzzy equivalence rules + penalty API
  - `src/tables.rs` — language-specific table loaders / legacy format adapters
  - `src/engine.rs` — Engine: parser + model + candidate generation
  - `src/bin/build.rs` — builder CLI to create binary model from corpora
  - `src/bin/query.rs` — interactive query CLI to inspect candidates
- `libzhuyin/` (crate)
  - mirror `libpinyin` but with zhuyin-specific parser & tables

Upstream files (representative) and mapping
- `src/pinyin.cpp`, `src/pinyin.h`
  - Purpose: top-level pinyin handling, public API glue in libpinyin.
  - Rust target: `libpinyin::engine` + `libpinyin::api` (public Rust API).
  - Notes: contains high-level conversion flows; use as behavioral spec.

- `src/pinyin_internal.cpp`, `src/pinyin_internal.h`
  - Purpose: internal helpers and lower-level APIs used by `pinyin.*`.
  - Rust target: `libpinyin::internal` (private helpers).

- `src/storage/pinyin_parser2.cpp`, `src/storage/pinyin_parser2.h`, `src/storage/pinyin_parser_table.h`
  - Purpose: pinyin parsing tables and parser implementations used by storage and lookup.
  - Rust target: `libpinyin::parser` and `core::model::io` (table loader).
  - Notes: `pinyin_parser_table.h` is a large generated table (binary-like). We will:
    - Create a parser that consumes a compact exposure of the syllable set (text or generated Rust table).
    - Support reading legacy tables if users need migration.

- `src/storage/ngram.cpp`, `src/storage/ngram.h`, `src/storage/flexible_ngram.*`
  - Purpose: n-gram storage, training support, and scoring utilities.
  - Rust target: `core::ngram` (`NGramModel` + training helpers).
  - Notes: port the interpolation smoothing behavior, support count→prob transforms, and expose training helpers for the builder CLI.

- `src/storage/phrase_index.cpp`, `src/storage/phrase_index.h`
  - Purpose: phrase indexing and lookup within phrase tables.
  - Rust target: `core::lexicon` (lookup API) and `libpinyin::tables` (format adapters).
  - Notes: this code is large and optimized for disk-backed backends (KyotoCabinet, BDB). Our plan:
    - Phase 1: support `bincode` in-memory/bundled models for portability.
    - Phase 2: implement `fst::Map` + compressed phrase store for production.

- `src/storage/phrase_large_table2.*`, `src/storage/phrase_large_table3.*`, `src/storage/phrase_large_table3_bdb.cpp`
  - Purpose: multiple formats / optimizations for large phrase tables.
  - Rust target: `core::lexicon` (abstraction) + `libpinyin::tables` (format conversion).
  - Notes: Many details (offsets, binary formats). Prefer building new models via a Rust builder and supporting a converter for users of existing binaries.

- `src/lookup/pinyin_lookup2.cpp`, `src/lookup/pinyin_lookup2.h`, `src/lookup/phrase_lookup.*`, `src/lookup/phonetic_lookup.*`
  - Purpose: core lookup pipelines combining phonetic input → phrase retrieval and candidate ranking.
  - Rust target: `libpinyin::engine` (Engine, Candidate generator).
  - Notes: These files implement the search strategies and heap/priority logic. They are the behavioral reference for `Engine::input(&str) -> Vec<Candidate>`.

- `src/lookup/lookup.cpp`, `src/lookup/lookup.h`
  - Purpose: public lookup APIs; wrappers for different phonetic lookups.
  - Rust target: `libpinyin::api` (public-facing functions).

- `src/zhuyin.cpp`, `src/zhuyin.h`, `src/storage/zhuyin_parser2.*`, `src/storage/zhuyin_table.h`
  - Purpose: zhuyin (bopomofo) variant-specific parsing and storage.
  - Rust target: `libzhuyin::parser`, `libzhuyin::tables`, reuse `core::ngram` and `core::lexicon`.

- `src/storage/punct_table.*`, `src/storage/special_table.h`, `utils/training/*`
  - Purpose: punctuation and special token mapping plus training utilities.
  - Rust target: `libpinyin::tables` (punct), `core::ngram::training` (training helpers).
  - Notes: training utilities (estimate_interpolation, gen_ngram, gen_unigram) are useful references when implementing builder CLI.

- `tests/*` (many test files: `tests/test_pinyin.cpp`, `tests/lookup/*`, `tests/storage/*`)
  - Purpose: official test vectors and unit tests.
  - Rust target: `core/tests/`, `libpinyin/tests/` — port test inputs and expected outputs for parity validation.

Key algorithmic responsibilities to extract from specific files
1. Pinyin segmentation and parser tables
   - Files: `src/storage/pinyin_parser2.*`, `scripts2/templates/pinyin_parser_table.h.in`, `scripts2/*` generator scripts
   - Behavior:
     - Valid syllable set (including special multi-letter syllables like `zh`, `ch`, `sh`).
     - Trie/table representation for fast syllable lookup.
     - Dynamic programming (DP) to find segmentation(s) with cost minimization (to handle ambiguous inputs like `siang` -> `si-ang` vs `s-iang`).
     - Integrate fuzzy mapping during segmentation (allow alternate syllable matches with penalty).
   - Port plan:
     - Build a `Trie` of canonical syllables in Rust (either built at compile-time via generated code or loaded at runtime).
     - Implement DP segmentation returning candidate segmentations and per-syllable fuzzy flags.

2. Fuzzy matching logic
   - Files: scattered across `src/pinyin.*`, `src/lookup/*` and config.
   - Behavior:
     - Pre-defined fuzzy equivalence pairs (`zh↔z`, `ch↔c`, `sh↔s`, `l↔n`, plus user-configurable pairs).
     - Apply a penalty or reduction to match scores when fuzzy substitution is required.
     - Often applied at both parser and lookup phases.
   - Port plan:
     - Represent fuzzy rules in `core::config::Config` (TOML) and in `libpinyin::fuzzy`.
     - Provide `FuzzyMap` utilities: given a syllable/token, produce canonical and fuzzed alternatives with penalty weights.

3. N-gram model and smoothing
   - Files: `src/storage/ngram.*`, `utils/training/*`
   - Behavior:
     - N-gram counts → probabilities conversion.
     - Interpolation-style smoothing (mixture of unigram/bigram/trigram) — training tools estimate interpolation weights.
     - Trigram/bigram backoff/fallback behavior for missing n-grams; floor probability for totally OOV tokens.
   - Port plan:
     - `core::ngram::NGramModel` stores ln(prob) maps for unigram/bigram/trigram.
     - Expose `score_sequence(tokens: &[String], cfg: &Config) -> f32`.
     - Implement builder CLI to compute counts and derive ln-probabilities and optionally estimate interpolation weights (re-using the logic from `utils/training/estimate_interpolation.cpp` as a reference).

4. Phrase table / lexicon lookup
   - Files: `src/storage/phrase_index.*`, `src/storage/phrase_large_table*.cpp/h`
   - Behavior:
     - Key format: canonical pinyin joiner (many internal implementations use a packed key or joiner).
     - Phrase frequency and positional metadata.
     - Efficient disk-backed indexes (KyotoCabinet, BDB) used for large tables.
   - Port plan:
     - Expose `Lexicon::lookup(key: &str) -> Vec<PhraseEntry>`, where `PhraseEntry` contains `text`, `freq`, optional `attributes`.
     - Short-term format: `bincode`-serialized `Model` containing `HashMap<String, Vec<PhraseEntry>>`.
     - Long-term: `fst::Map` for key -> id and a compact phrase store blob for id -> list.

5. Lookup / candidate ranking
   - Files: `src/lookup/pinyin_lookup2.cpp`, `src/lookup/phrase_lookup.cpp`, `src/lookup/phonetic_lookup.*`
   - Behavior:
     - Combine segmentation candidates with lexicon lookup and n-gram scoring to produce ranked result lists.
     - Top-N selection using heaps / priority queues (efficient candidate enumeration).
     - Tie-breakers: frequency, n-gram score, userdict boost.
   - Port plan:
     - Implement `libpinyin::engine::Engine` with `input(&str) -> Vec<Candidate>`:
       - Use parser to produce segmented keys and alternatives.
       - Query `core::Lexicon` for each key variant.
       - Score each candidate: `score = ngram_score + lexicon_score + user_boost + (fuzzy_penalty)`.
       - Return top-N sorted `Vec<Candidate>`.

6. User-learning semantics
   - Files: `src/storage/*user*`, assorted logic in `pinyin.cpp`
   - Behavior:
     - On commit, increment user phrase frequency.
     - Merge semantics: user frequencies are summed; on conflicts, user entries may override.
     - User dict persistence via disk-backed DBs (BDB, Kyoto).
   - Port plan:
     - Provide `core::UserDict` trait + `redb`-based implementation `core::userdict::RedbUserDict`.
     - API: `learn(phrase: &str)`, `get_count(phrase: &str) -> u64`, `merge_from(other)`.
     - Ensure reads are safe for concurrent UI threads.

Files and tests to port first (high priority)
- Parser & segmentation tests:
  - `tests/storage/test_parser2.cpp` — segmentation expectations.
  - Port representative input→segmentation vectors as unit tests in `libpinyin/tests/parser_tests.rs`.
- N-gram tests:
  - `tests/storage/test_ngram.cpp` — implement parity checks for score computation after training.
  - Port small n-gram data + expected scores.
- Lookup tests:
  - `tests/lookup/test_pinyin_lookup.cpp`, `tests/lookup/test_phrase_lookup.cpp` — port representative cases to verify candidate generation order.
- Phrase table tests:
  - `tests/storage/test_phrase_table.cpp` — validate lexicon IO and lookups when using converted data.

Migration & interoperability considerations
- Binary model format
  - Upstream uses multiple disk-backed formats and generated tables.
  - We will provide a `builder` CLI to produce Rust-native models (`bincode` for n-gram + `fst` for lexicon).
  - If users require reading legacy binary blobs, implement a converter in `libpinyin::tables::legacy_convert`.
- Versioning
  - Add explicit header to model files: `magic` + `version` + `format_flags` to allow forward/backward compatibility.
- Performance
  - Phase 1: correctness-first — usable in-memory HashMap + bincode.
  - Phase 2: integrate `fst::Map` for lexicon and micro-optimizations (ahash) for hot maps.
  - Consider memory-mapped read-only layouts for large phrase stores.

Security and licensing
- Upstream `libpinyin` is GPL-3.0. Our reimplementation is planned under MIT for new code.
- Where algorithmic behavior is ported (not verbatim code), be mindful of license compatibility and include attribution in `LICENSE` and `NOTICE` per project policy.

Concrete next actions (what I will implement next)
1. Create skeleton Rust modules and function signatures:
   - `core/src/ngram.rs`, `core/src/lexicon.rs`, `core/src/userdict.rs`, `core/src/config.rs`, `libpinyin/src/parser.rs`, `libpinyin/src/fuzzy.rs`, `libpinyin/src/engine.rs`, `libpinyin/src/tables.rs`.
2. For each skeleton add TODOs referencing the corresponding upstream files (exact paths) and include minimal unit-test stubs that will be filled by ported test vectors from `tests/` upstream.
3. Begin implementing the parser module: build a Trie and DP segmentation algorithm in `libpinyin/src/parser.rs` with unit tests seeded from `tests/storage/test_parser2.cpp`.

Acceptance criteria for Phase 2 completion
- A complete mapping document (this file) containing:
  - Per-file mapping & responsibilities.
  - Clear porting notes for all major algorithms.
  - Prioritized test list to validate parity.
- Rust skeleton modules created and wired into workspace (next step).
- Proof plan for migration of binary models and tests.

Appendix — Representative upstream file references
(These are the files inspected in the upstream repository scan and used to derive the mapping above.)
- `src/pinyin.cpp`, `src/pinyin.h`, `src/pinyin_internal.*`
- `src/storage/pinyin_parser2.cpp`, `src/storage/pinyin_parser2.h`, `src/storage/pinyin_parser_table.h`
- `src/storage/ngram.cpp`, `src/storage/ngram.h`, `utils/training/*.cpp`
- `src/storage/phrase_index.cpp`, `src/storage/phrase_index.h`, `src/storage/phrase_large_table2.*`
- `src/lookup/pinyin_lookup2.cpp`, `src/lookup/phrase_lookup.cpp`, `src/lookup/phonetic_lookup.cpp`
- `src/zhuyin.*`, `src/storage/zhuyin_parser2.*`, `src/storage/zhuyin_table.h`
- `tests/storage/test_parser2.cpp`, `tests/storage/test_ngram.cpp`, `tests/lookup/test_pinyin_lookup.cpp`, `tests/lookup/test_phrase_lookup.cpp`

If you want, I will proceed now to:
- Option A: Create the Rust skeleton modules and tests described above, or
- Option B: Start implementing the `libpinyin` parser (trie + DP) and port a small set of parser tests from `tests/storage/test_parser2.cpp`.

Which do you prefer?
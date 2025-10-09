# Phase 2 — libpinyin source mapping and analysis template

Purpose
-------
This document captures the mapping between the original `libpinyin` C++ source and the planned Rust modules under the `libchinese-core` and language-specific crates (`libpinyin`, `libzhuyin`). It focuses on algorithmic behavior to be preserved (segmentation, fuzzy matching, n-gram scoring, user learning) rather than line-for-line translation. Use this template while auditing the C++ code and filling in the detailed notes per-file.

Scope
-----
- Identify core C++ files and map them to Rust modules.
- Summarize algorithmic intent and required behaviors.
- Note persistent formats and migration strategy (binary blobs -> bincode/fst/redb).
- Provide a prioritized migration checklist and test vectors to port.

How to use
----------
1. For each C++ file in the original repo, add a row to the mapping table with the current findings.
2. Under "Algorithmic notes", describe the core logic in neutral phrasing (pseudo-algorithm).
3. Under "Migration hints", list data-structure and API translation suggestions.
4. Track testcases and acceptance criteria under "Validation".

Mapping table (starter)
-----------------------
| C++ source (original) | Responsibility / short description | Rust module target (planned) | Notes / algorithmic highlights |
|---|---|---:|---|
| `pinyin_parser.cc` / `pinyin_parser.h` | Pinyin syllable segmentation, DP for best segmentation | `libpinyin::parser` → `core::pinyin::parser` (later) | Implements trie of valid syllables, dynamic programming to choose segmentation with fuzzy support. Port to a Trie-based Rust implementation; expose `segment(&str) -> Vec<PinyinSyllable>` and `segment_best(&str, cfg) -> Vec<SyllableCandidates>`. |
| `fuzzy_pinyin.cc` / `fuzzy_pinyin.h` | Fuzzy equivalence rules (zh↔z, ch↔c, sh↔s, l↔n, etc.) | `libpinyin::fuzzy` → `core::config` + `libpinyin::fuzzy` | Store equivalence as `HashMap<String, Vec<String>>` or specialized `enum` rules. Penalize fuzzy matches during scoring. Config-driven. |
| `ngram.cc` / `ngram.h` | N-gram model: training, smoothing, scoring (log-probs) | `core::ngram::NGramModel` | Translate probabilities to ln-space; implement linear interpolation with configurable lambda weights. Provide `score_sequence(tokens: &[String], cfg: &Config) -> f32`. Keep same smoothing behavior or implement Katz/Kneser-Ney if explicitly used. |
| `phrase_table.cc` / `phrase_table.h` | Phrase -> pinyin mapping, phrase frequency tables | `core::lexicon` (initial) then `libpinyin::tables` | Replace blob / custom format with `fst::Map` for lookup or bincode / fst combined. Provide reverse lookup: pinyin-key -> list of phrases. |
| `user_phrase.cc` / `user_phrase.h` | User learning and phrase frequency updates | `core::userdict` | Use `redb` for persistence; runtime API: `learn(phrase)`, `get_count(phrase)`, `merge`. Keep atomic increment semantics. |
| `config.cc` / `config.h` | Configuration parsing (fuzzy flags, weights) | `core::config` (TOML) | Replace GLib/INI with `serde` + TOML. Provide clear schema and defaults. |
| `model_io.cc``/`model_io.h` | Binary model load/save | `core::model::io` | Replace with `bincode`-serialized `Model { lexicon, ngram }` for now; later adopt `fst` + separate ngram file. Provide versioning header in model file. |
| `segmentation_tests.cc` / test data | Unit test expectations for segmentation | `libpinyin/tests` and `core/tests` | Port test vectors verbatim; use them to validate segmentation parity. |
| `candidate_generator.cc` | Combine segmentation + lexicon -> candidate list, apply scoring | `libpinyin::engine` using `core::Model` | Provide `Engine::input(&str) -> Vec<Candidate>` which uses parser → lexicon → ngram → userdict. |
| `lua_bindings.cc` / `glue` | GUI bindings, legacy integrations | Drop (out of scope) | Not porting. Replace with clean Rust API + optional C-ABI later. |
| `autotools` / build scripts | Build system | Cargo workspace | Replace autotools with `cargo` workspace and crate-level build scripts if needed. |

Algorithmic notes (per-topic)
-----------------------------

- Pinyin segmentation
  - Input: raw Latin string (possibly with spaces), e.g. `nihao`.
  - Core idea: build a trie of valid syllables, then run dynamic programming over the input to find valid partitions. Each partition receives a cost: either length-based, frequency-based, or fuzzy-penalty adjusted.
  - Desired API: `segment_candidates(input: &str) -> Vec<Vec<SyllableMatch>>` and `best_segment(input: &str, cfg: &Config) -> Vec<Syllable>`.
  - Fuzzy integration: whenever a syllable is matched via a fuzzy rule, mark with penalty and propagate to score.

- Fuzzy matching
  - Represent fuzzy mapping as list of equivalence classes (bi-directional). Example canonicalization: `zh` ↔ `z`, `ch` ↔ `c`, `sh` ↔ `s`, `l` ↔ `n`.
  - Scoring: apply a configurable penalty (e.g., subtract delta in ln-space or multiply probability) for fuzzy-substituted syllables. Keep fuzzy toggles in `Config`.

- N-gram scoring
  - Store ln probabilities for unigram/bigram/trigram.
  - Compute interpolation: lambda1*lnP1 + lambda2*lnP2 + lambda3*lnP3 per token and sum across sequence.
  - Missing n-grams: fallback to lower-order (bigram→unigram→floor). Choose floor like -20.0 (≈2e-9).
  - Training: builder CLI will compute counts and convert to probabilities with smoothing; preserve count->prob formula in migration.

- Candidate generation
  - For each segmentation, join the pinyin key and query lexicon to produce phrase lists.
  - Score each phrase by converting phrase → token sequence (language-specific tokenization), then scoring with n-gram and applying userdict boost.
  - Rank by final score. Return top-N.

Data formats and migration strategy
----------------------------------
- Replace legacy binary blobs:
  - Short term: `bincode`-serialized `Model` struct (contains lexicon map and n-gram maps).
  - Long term: separate compact formats:
    - Lexicon: `fst::Map` (pinyin_key -> offset/ID). Store phrases in a compressed blob or separate SST file indexed by ID.
    - N-gram: custom binary with header + compressed sections (or `bincode` initially).
    - Userdict: persisted in `redb` for durability and concurrency.
- Versioning:
  - Each binary model should have a small header with `magic`, `version`, `format_flags` to ensure forward/backward compatibility.
- Migration path:
  1. Implement reader/writer for current C++ model format (if needed) or rebuild model using the builder CLI against raw corpora.
  2. Provide tooling to convert C++ blobs → new bincode/fst format, if required for users.

Testing & validation
--------------------
- Port test vectors:
  - Pinyin → segmentation canonical outputs.
  - Pinyin input → top-5 candidate lists.
  - N-gram scoring: given token sequences, compare scores from original C++ implementation vs Rust (allow small epsilon).
  - User learning: increment + merge semantics parity.
- Unit test structure:
  - `core/tests/` for generic model scoring, userdict behavior, serialization round-trips.
  - `libpinyin/tests/` for segmentation, fuzzy-matching, candidate generation.
- Integration tests:
  - End-to-end: `cargo run -p libpinyin -- query` with test corpus and assert stable outputs.
- Benchmarks:
  - Use `criterion` in a `benches/` directory to compare segmentation and lookup performance.

Migration checklist (prioritized)
--------------------------------
1. Audit original source files and fill this mapping table with exact file paths and responsibilities.
2. Implement and test `parser` module (trie + DP segmentation) in `libpinyin` crate. Port segmentation unit tests.
3. Implement `fuzzy` module with clear mapping rules and tests.
4. Implement `core::NGramModel` scoring and port n-gram tests.
5. Implement `core::Lexicon` API and a `model_io` writer/reader for immediate compatibility with builder output.
6. Implement `core::UserDict` persistent backend using `redb` and ensure thread-safety.
7. Create `Engine` in `libpinyin` integrating parser+lexicon+ngram+userdict and implement `input(&str) -> Vec<Candidate>`.
8. Add builder CLI to construct `bincode`/`fst` model from raw corpora.
9. Port remaining tests and run parity checks vs the original c++ outputs.
10. Optimize lexicon storage with `fst::Map` and tune `redb` batching.

Acceptance criteria
-------------------
- Functional parity on core behaviors:
  - Identical segmentation results for official segmentation testcases.
  - Equivalent top-N candidates (or acceptable ranking within epsilon) for candidate generation testcases.
  - User learning persists and influences ranking as in original tests.
- Unit tests added for all ported behaviors.
- CI runs `cargo test` for `core` and `libpinyin` and includes model build+query integration test.

Open questions / TODOs to resolve during audit
---------------------------------------------
- Exact set of fuzzy rules used in production: are there more than the common zh/ch/sh/l rules?
- Original n-gram smoothing algorithm: simple add-k, Katz, or Kneser-Ney? The chosen smoothing must be reproduced for parity.
- Does the original lexicon store phrase-level probabilities or only counts? How are ties broken?
- Binary model format details (endianness, versioning): is it worth supporting direct reading of old models or require rebuild?
- Performance constraints: is memory usage or lookup latency the most critical constraint? This will drive `fst` vs in-memory HashMap choice.
- Are there any multithreaded access expectations for userdict and lexicon from UI clients?

Appendix: Suggested Rust module layout (initial)
-----------------------------------------------
- `core/`
  - `src/lib.rs` (current core API)
  - `src/ngram.rs` (`NGramModel`, training helpers)
  - `src/lexicon.rs` (`Lexicon` type + io helpers)
  - `src/userdict.rs` (`UserDict` trait + redb backend)
  - `src/config.rs` (`Config` + serde <-> TOML)
  - `src/model.rs` (`Model` container + serialization`)
  - `src/utils.rs`
- `libpinyin/`
  - `src/parser.rs` (trie, segmentation DP)
  - `src/fuzzy.rs` (fuzzy maps, penalties)
  - `src/engine.rs` (`Engine` wrapping `Model` and language glue)
  - `src/tables.rs` (language-specific table loaders)
  - `src/bin/` (CLI: build / query)
- `libzhuyin/` (same layout as `libpinyin` but zhuyin-specific parser/tables)

Fill-in checklist (for auditor)
-------------------------------
- [ ] Replace each row in the mapping table with the exact C++ file path from the upstream repo.
- [ ] Summarize the algorithm in plain English for each file and include small pseudocode snippets where helpful.
- [ ] Identify test files and copy test vectors into `core/tests/data` or `libpinyin/tests/data`.
- [ ] Note any license attribution or attribution required when porting logic.

End of template — use this file to record findings as you audit the original `libpinyin` repo and prepare for implementation phases 3–6.
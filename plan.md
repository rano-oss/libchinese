libpinyin is a large, legacy C++/GLib library designed for intelligent Chinese pinyin input.
You’re not just translating code — you’re modernizing the architecture around Rust idioms and memory safety, while preserving the core linguistic intelligence.

Decide on your scope:

✅ Keep: core algorithms — segmentation, fuzzy matching, n-gram scoring, user learning

❌ Drop: GLib, Lua, autotools, custom allocators, cloud services

⚙️ Replace: configuration, file formats, dictionary storage, and testing system

Your end goal is three pure Rust crates:

libchinese-core — shared foundation (model, lexicon, n-gram logic, userdict, config)

libpinyin and libzhuyin — language-specific frontends

🧩 Step 2. Analyze the current libpinyin structure

From the original source tree, identify and categorize files:

| Area                          | Files / Modules                             | What it does                            | Porting Decision                        |
| ----------------------------- | ------------------------------------------- | --------------------------------------- | --------------------------------------- |
| **Parsing / Syllabification** | `pinyin_parser.cc`, `pinyin_parser_table.h` | Converts raw text into pinyin syllables | Rewrite in Rust, keep data tables       |
| **Fuzzy matching**            | `fuzzy_pinyin.cc`, `pinyin_fuzzy_map.h`     | Maps near sounds (zh↔z, ch↔c)           | Port as static map in Rust              |
| **Lexicon and phrase tables** | `phrase_table.cc`, `phrase_index.cc`        | Maps pinyin to Hanzi candidates         | Replace with Rust `fst` map             |
| **N-gram scoring**            | `ngram.cc`, `context_model.cc`              | Statistical language model              | Re-implement logic using Rust `HashMap` |
| **User learning**             | `user_phrase.cc`                            | Stores user selections and frequencies  | Replace with embedded DB (`redb`)       |
| **Configuration**             | `config.cc`                                 | Stores settings, modes, fuzzy flags     | Replace with `toml`           |
| **Memory and GLib wrappers**  | `memory_chunk.cc`, GLib types               | Manual memory, refcounting              | Drop entirely                           |
| **Tools**                     | `gen_unigram`, `import_interpolation`, etc. | Preprocess data models                  | Replace with Rust CLI builder           |
| **Lua/cloud layers**          | `lua_extension.cc`, `cloud_service.cc`      | Script and online features              | Drop                |

Step3:
| Rust Module      | Replaces                        | Description                           |
| ---------------- | ------------------------------- | ------------------------------------- |
| `core::lexicon`  | `phrase_table.cc`               | Pinyin → Hanzi lookup (static + user) |
| `core::ngram`    | `ngram.cc`                      | Contextual scoring                    |
| `core::userdict` | `user_phrase.cc`                | User dictionary                       |
| `core::config`   | `config.cc`                     | Config handling                       |
| `pinyin::parser` | `pinyin_parser.cc`              | Syllable segmentation                 |
| `pinyin::fuzzy`  | `fuzzy_pinyin.cc`               | Fuzzy equivalence                     |
| `api::Engine`    | combination of lookup + context | High-level IME API                    |

Step4:
Step 4. Extract linguistic logic (not code)

You don’t want a mechanical translation of C++; you want to extract the algorithms:

Pinyin segmentation

Identify syllable boundaries via a prefix trie of valid syllables.

Implement dynamic programming to pick optimal splits.

Keep fuzzy substitution rules for tolerance.

Candidate generation

Load dictionary entries: pinyin sequence → phrases.

Use finite state transducers (fst crate) or hashmaps for lookup.

N-gram scoring

Reuse libpinyin’s probability model concept:
P(phrase) = λ1*P1 + λ2*P2 + λ3*P3

Implement backoff smoothing between uni/bigram/trigram.

Store frequency tables in binary format using serde + bincode.

User learning

On candidate commit, increase phrase frequency in userdict.redb.

Merge user frequencies into runtime lookups.

Use transactions in redb for atomic updates.

Ranking

Combine dictionary frequency, context score, and user boost.

Sort and prune top candidates.

Step5:
Step 5. Design new data storage formats

Replace libpinyin’s binary blobs and offset tables with structured Rust formats:
| Type         | Old                                 | New                                 |
| ------------ | ----------------------------------- | ----------------------------------- |
| Base lexicon | `phrase_table.bin`                  | Serialized with `bincode`           |
| N-gram model | `.unigram` / `.bigram` / `.trigram` | `serde` struct + optional `fst` map |
| User dict    | `user_phrase` file                  | `redb` database                     |
| Config       | `.conf`                             | `config.toml`                       |

Step6:
Implement base engine in Rust

🧮 Step 7. Port preprocessing tools

libpinyin includes CLI tools for building binary models (gen_binary_files, import_interpolation, etc.).
In Rust, merge them into a single builder subcommand:
cargo run -p libpinyin -- build \
    --input data/unigram.txt \
    --output models/unigram.bin

Implementation:

Read raw text corpus

Compute frequencies

Serialize as binary model (bincode::serialize_into())

Step 8. Add CLI interfaces for testing

🧠 Step 9. Verify correctness against libpinyin

To validate your Rust logic:

Extract test cases from libpinyin’s tests/ directory.

Compare candidate ranking output.

Use the same model data for fair comparison.

Create a small diff tool that prints mismatched probabilities.

⚙️ Step 10. Optimize and extend

Use fst for prefix dictionary queries (fast autocomplete).

Use redb transactions to handle multiple user profiles.

Cache candidate lists for repeated prefixes.

🧩 Step 11. Build libzhuyin

Once your core crate is solid:

Add a zhuyin::parser for Bopomofo tokenization.

Reuse the same core::lexicon and core::ngram.

Train Zhuyin data with same builder tools.

You now support both input systems via one shared engine.
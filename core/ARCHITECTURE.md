# Core Architecture & Type Hierarchy

## Overview
`libchinese-core` provides the shared foundation for Chinese IME engines (libpinyin, libzhuyin).

## Type Hierarchy

```
Model (top-level container)
├── Lexicon (syllable → phrase mapping)
│   ├── AHashMap<String, Vec<String>> (in-memory)
│   ├── fst::Map (on-disk FST index)
│   └── Vec<Vec<LexEntry>> (bincode payloads)
├── NGramModel (statistical scoring)
│   ├── HashMap<String, f64> (unigrams)
│   ├── HashMap<(String, String), f64> (bigrams)
│   └── HashMap<(String, String, String), f64> (trigrams)
├── UserDict (user learning)
│   └── redb::Database (persistent KV store)
├── Config (feature flags & weights)
└── Interpolator (adaptive lambda weights)
    ├── fst::Map (key → index)
    └── Vec<Lambdas> (per-key weights)

Engine<P: SyllableParser> (generic IME pipeline)
├── Model (scoring & lookup)
├── P: SyllableParser (language-specific)
├── FuzzyMap (phonetic corrections)
└── LruCache<String, Vec<Candidate>> (result cache)

Candidate (output)
├── text: String (phrase)
└── score: f32 (ranking)
```

## Module Responsibilities

### `lib.rs` (390 lines)
- **Exports**: Public API surface
- **Types**: `Candidate`, `Config`, `Model`, `Lexicon`, `LexEntry`
- **Role**: Configuration management, top-level composition
- **Can optimize**: Config has redundant SortOption enum (not used)

### `ngram.rs` (601 lines)
- **Types**: `NGramModel`, `Interpolator`, `Lambdas`
- **Role**: Statistical language modeling, probability scoring
- **Tests**: 3 unit tests (60 lines)
- **Can optimize**: HashMap → DashMap for concurrent access

### `fuzzy.rs` (378 lines)
- **Types**: `FuzzyMap`, `FuzzyRule`
- **Role**: Phonetic similarity, alternative generation
- **Tests**: 7 unit tests (117 lines)
- **Can optimize**: Use phf for compile-time fuzzy rules

### `trie.rs` (243 lines)
- **Types**: `TrieNode`
- **Role**: Prefix matching for syllable segmentation
- **Tests**: 5 unit tests (74 lines)
- **Can replace**: Use `trie-rs` or `qp-trie` crate

### `userdict.rs` (173 lines)
- **Types**: `UserDict`
- **Role**: Persistent user phrase learning
- **Tests**: 1 unit test (21 lines)
- **Dependencies**: redb (already optimal)

### `engine.rs` (358 lines)
- **Traits**: `SyllableParser`, `SyllableType`
- **Types**: `Engine<P>`
- **Role**: Generic IME pipeline (parse → lookup → score → rank)
- **Tests**: None (tested via integration tests)
- **Can optimize**: Consider rayon for parallel candidate generation

## Dependencies Analysis

### Keep (production-ready)
- `serde`, `bincode` - serialization (standard)
- `fst` - memory-efficient FST (optimal for lexicon)
- `redb` - embedded DB (better than sled/rocksdb)
- `lru` - LRU cache (standard)
- `unicode-normalization` - NFC normalization (essential)

### Consider replacing
- `ahash` → Already optimal (faster than std HashMap)
- `tracing` → Unused? Check if any trace! macros exist
- `anyhow` → Only used in ngram.rs, could use std::error::Error
- **TrieNode implementation → `trie-rs` crate** (230 lines saved)

### Candidates for external crates
1. **TrieNode → `trie-rs`** (most impactful)
   - Current: 243 lines custom implementation
   - Replacement: `trie-rs = "0.2"` (well-tested, optimized)
   - Savings: ~170 lines (keep 70 for tests/integration)

2. **FuzzyMap → Keep custom** (domain-specific logic)
   - Phonetic similarity rules are IME-specific
   - No generic fuzzy matching library fits

3. **NGramModel → Keep custom** (simple & clear)
   - Generic n-gram libraries are overkill
   - Our log-prob storage is optimal

## Test Organization

### Current state (272 test lines embedded)
```
fuzzy.rs:    7 tests (117 lines) ← Move to tests/fuzzy_tests.rs
ngram.rs:    3 tests ( 60 lines) ← Move to tests/ngram_tests.rs  
trie.rs:     5 tests ( 74 lines) ← Move to tests/trie_tests.rs
userdict.rs: 1 test  ( 21 lines) ← Keep (DB lifecycle test)
```

### Proposed structure
```
core/
├── src/           (1690 lines → 1418 lines after cleanup)
└── tests/
    ├── fuzzy_tests.rs
    ├── ngram_tests.rs
    └── trie_tests.rs
```

## Excessive Documentation to Prune

### lib.rs
- Lines 125-150: `SortOption` enum (unused, Config bool flags are used instead)
- Lines 60-95: Overly verbose Config field comments (reduce to 1 line each)

### engine.rs
- Lines 215-253: Excessive commit() doc example (reduce to 5 lines)
- Generic trait docs are good (keep as-is)

### ngram.rs
- Lines 1-20: Redundant module header (reduce to 5 lines)
- Keep: Serialization logic is complex, docs helpful

### fuzzy.rs
- Lines 1-18: Verbose module header (reduce to 5 lines)
- Keep: Rule parsing is tricky, docs helpful

### trie.rs
- Lines 1-15: Verbose module header (reduce to 5 lines)
- Lines 22-42: Example in struct doc (move to README)

### userdict.rs
- Already minimal (keep as-is)

## Cleanup Checklist

- [ ] Remove `SortOption` enum (unused, 30 lines)
- [ ] Reduce Config field doc comments (save 35 lines)
- [ ] Simplify module headers (save 40 lines)
- [ ] Move fuzzy tests → tests/ (save 117 lines from src)
- [ ] Move ngram tests → tests/ (save 60 lines from src)
- [ ] Move trie tests → tests/ (save 74 lines from src)
- [ ] Replace TrieNode with `trie-rs` crate (save ~170 lines)
- [ ] Remove unused `tracing` dependency (if no trace! macros)
- [ ] Reduce engine.rs doc examples (save 30 lines)

**Total savings: ~556 lines → 1690 → 1134 lines (33% reduction)**

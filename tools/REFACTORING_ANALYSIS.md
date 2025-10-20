# Tools Folder Refactoring Analysis

## Current Structure Issues

### 1. Confusing Mixed Architecture
The `tools/` folder has a confusing mix of:
- **Main crate** (`tools/Cargo.toml` → `convert_tables`)
  - Contains `src/main.rs`, `src/lib.rs`
  - Has bins in `src/bin/` directory
- **Standalone workspace member crates** with their own Cargo.toml:
  - `estimate_interpolation/`
  - `serialize_ngram/`
  - `gen_bigrams_from_tables/`
  - `inspect_redb/`
  - `list_fst_keys/`

### 2. Duplicate/Conflicting `estimate_interpolation`

**TWO different implementations:**

1. **`tools/estimate_interpolation/src/main.rs`** (170 lines)
   - Standalone crate with own Cargo.toml
   - Depends on `libchinese-core`
   - Reads deleted bigram text file
   - Computes per-left-token lambdas using fixed-point iteration
   - Outputs FST + redb

2. **`tools/src/bin/estimate_interpolation.rs`** (203 lines)  
   - Binary in the `convert_tables` crate
   - Uses `convert_tables::bigram_db::BigramDB`
   - Reads interpolation model dump (.text format)
   - Computes per-token backoff/interpolation lambdas
   - Outputs text file

**Different algorithms, different input/output formats!**

### 3. Quadruple `PhraseEntry` Definition

The exact same struct is defined in **4 places**:

```rust
struct PhraseEntry {
    text: String,
    freq: u64,
}
```

Locations:
1. `tools/src/convert_table.rs:10` (pub struct, Serialize + Deserialize)
2. `tools/src/convert_interpolation.rs:133` (local inline struct)
3. `tools/gen_bigrams_from_tables/src/main.rs:25` (Deserialize)
4. `core/src/lib.rs:192` (Deserialize, private)

### 4. Inconsistent Binary Organization

**Workspace members with own Cargo.toml:**
- ✅ `inspect_redb` - clear standalone tool
- ✅ `list_fst_keys` - clear standalone tool
- ❓ `serialize_ngram` - could be bin in main tools crate
- ❓ `gen_bigrams_from_tables` - could be bin in main tools crate
- ❌ `estimate_interpolation` - conflicts with src/bin/estimate_interpolation.rs

**Bins in src/bin/:**
- `build_bigram_db.rs` - uses `convert_tables::bigram_db`
- `estimate_interpolation.rs` - uses `convert_tables::bigram_db`

### 5. Unclear Entry Points

The main `tools/src/main.rs` has logic to detect input type:
- If `.text` file → calls `convert_interpolation::run()`
- Otherwise → calls `convert_table::run()`

This is confusing because users don't know whether to run:
- `cargo run -p convert_tables`
- `cargo run --bin estimate_interpolation` (which one?)
- `cargo run -p estimate_interpolation`

## Recommended Refactoring

### Option A: Consolidate Everything into `tools` Crate

```
tools/
├── Cargo.toml (main crate: "libchinese-tools")
├── src/
│   ├── lib.rs (shared code)
│   ├── common/
│   │   └── phrase_entry.rs (shared PhraseEntry)
│   ├── bigram_db.rs
│   └── bin/
│       ├── convert_tables.rs (from src/main.rs logic)
│       ├── convert_interpolation.rs (keep ONE implementation)
│       ├── estimate_lambda.rs (rename to avoid confusion)
│       ├── build_bigram_db.rs
│       ├── serialize_ngram.rs (move from subdir)
│       ├── gen_bigrams.rs (move from subdir)
│       ├── inspect_redb.rs (move from subdir)
│       └── list_fst_keys.rs (move from subdir)
```

**Pros:**
- Single crate to maintain
- Easy to share code via lib.rs
- Clear: all tools are bins in one place
- No naming conflicts

**Cons:**
- Large dependency tree shared by all bins
- Slower compile if you only want one tool

### Option B: Keep Standalone Tools, Clean Up Main

```
tools/
├── Cargo.toml (core tools crate: "libchinese-tools-core")
├── src/
│   ├── lib.rs (shared utilities)
│   ├── common.rs (PhraseEntry, etc.)
│   └── bigram_db.rs
├── convert_tables/ (main converter - rename from tools/)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── table_converter.rs
│       └── interpolation_converter.rs
├── estimate_lambda/ (RENAME, keep ONE implementation)
│   ├── Cargo.toml
│   └── src/main.rs
├── build_bigram_db/
│   ├── Cargo.toml
│   └── src/main.rs
├── serialize_ngram/ (keep)
├── gen_bigrams/ (rename from gen_bigrams_from_tables)
├── inspect_redb/ (keep)
└── list_fst_keys/ (keep)
```

**Pros:**
- Each tool can have minimal dependencies
- Tools are independently versioned
- Clear separation of concerns
- Fast incremental builds

**Cons:**
- More Cargo.tomls to maintain
- Need to carefully manage shared code in core crate

## Immediate Actions Required

### Critical
1. **Resolve `estimate_interpolation` conflict**
   - Decide which implementation to keep (or merge them)
   - Rename one to avoid confusion
   - Document which does what

2. **Consolidate `PhraseEntry`**
   - Move to `tools/src/common.rs` or `core/src/lexicon.rs`
   - Make it `pub` and add proper derives
   - Update all 4 locations to use the shared version

### High Priority
3. **Document tool purposes in README**
   - What each tool does
   - When to use which
   - Example commands

4. **Standardize naming**
   - Either all tools are workspace members with subdirs
   - Or all tools are bins in main tools crate
   - Don't mix both!

### Medium Priority
5. **Clean up convert_tables main.rs logic**
   - The auto-detection by file extension is clever but confusing
   - Consider separate entry points: `convert-tables` and `convert-interpolation`

6. **Review bigram_db.rs**
   - Currently only exported via lib.rs
   - Only used by src/bin/ binaries
   - Should other tools use it?

## Questions for Maintainers

1. Which `estimate_interpolation` is canonical? What's the difference?
2. Should all tools share dependencies via a main crate, or stay independent?
3. Are the subdirectory tools (inspect_redb, list_fst_keys) meant to be user-facing or internal dev tools?
4. Should `PhraseEntry` live in core (shared with library users) or tools (internal only)?


## TODOS before completion:
1. Fuzzy can probably be reused in libzhuyin as well as libpinyin? Clean up and move to core, make it only initialize with configuration.
2. Engine can probably be merged(same for pinyin and zhuyin) as well, should be fairly similar in behavior, just data and parser that is different. 
3. More code should be moved from redb to bincode(remove json metadata output, figure out what metadata is even needed)
4. Data scripts need to be rewritten so it handles creating all the binaries and fst files correctly, also separate into pinyin.simplified.fst/bin and pinyin.traditional.fst/bin.
Zhuyin also needs separation between traditional and simplified. Dealing with merged we will avoid as it is too much of a niche for now. 
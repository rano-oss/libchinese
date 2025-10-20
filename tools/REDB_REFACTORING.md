# ReDB Usage Analysis and Refactoring Proposal

## Current ReDB Usage

### 1. **Lexicon Dictionary (WRONG USE OF REDB)**
**Location:** `core/src/lib.rs` - `Lexicon` struct
**Current:** FST + ReDB for read-only phrase lookups
- FST maps `pinyin_key -> index`
- ReDB stores `index -> bincode(Vec<PhraseEntry>)`
- Data is **static** and **read-only** after generation

**Problem:**
- ReDB is a transactional database designed for writes and ACID guarantees
- The lexicon data never changes after generation
- We're paying for transaction overhead on every lookup
- ReDB file size is larger than raw bincode would be

**Should be:** FST + plain bincode file
- FST maps `pinyin_key -> (offset, length)` in the bincode file
- Single bincode file contains all serialized phrase lists
- Much faster reads (no transaction overhead)
- Smaller file size
- Simpler code

### 2. **UserDict (CORRECT USE OF REDB)**
**Location:** `core/src/userdict.rs`
**Current:** ReDB for user learning/frequency tracking
- Stores `phrase -> frequency_count`
- Supports transactions (learn, increment, update)
- Dynamic data that changes frequently

**This is correct!** UserDict needs:
- ✅ ACID transactions (multiple IME windows writing simultaneously)
- ✅ Concurrent read/write access
- ✅ Crash recovery (user data is precious)
- ✅ Incremental updates (no need to rewrite entire file)

### 3. **Interpolation Lambdas (QUESTIONABLE USE OF REDB)**
**Location:** `tools/estimate_interpolation/src/main.rs`
**Current:** Outputs FST + ReDB for lambda values
- FST maps `token -> index`
- ReDB stores `index -> lambda_value`

**Problem:**
- Lambda values are computed once during training
- Never modified during runtime
- Small data size (~few MB)
- No need for transactions

**Should be:** FST + bincode file OR just a single bincode HashMap

## Refactoring Plan

### Phase 1: Create Common PhraseEntry Type (Preparatory)

**Changes to `core/src/lib.rs`:**

```rust
/// Shared phrase entry structure for lexicon data.
/// Used by both build tools and runtime loading.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PhraseEntry {
    pub text: String,
    pub freq: u64,
}

/// Collection of phrases for a single key, stored compactly.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PhraseList {
    pub entries: Vec<PhraseEntry>,
}
```

**Benefits:**
- Single source of truth for phrase data structure
- Removes 4 duplicate definitions
- Can add methods like `sort_by_freq()` in one place

### Phase 2: Replace Lexicon FST+ReDB with FST+Bincode

**Changes to `core/src/lib.rs`:**

```rust
pub struct Lexicon {
    map: AHashMap<String, Vec<String>>,
    fst_map: Map<Vec<u8>>,
    // OLD: db: Option<Arc<Database>>,
    // NEW: phrase_data loaded into memory or mmap'd
    phrase_data: Vec<u8>,
    no_apos_map: AHashMap<String, u64>,
    metadata: LexiconMetadata,
}

impl Lexicon {
    pub fn load_from_fst_bincode<P: AsRef<std::path::Path>>(
        fst_path: P, 
        bincode_path: P
    ) -> Result<Self, String> {
        // Load FST
        let fst_bytes = std::fs::read(fst_path)?;
        let map = Map::new(fst_bytes)?;
        
        // Load entire bincode file into memory (or use mmap for large files)
        let phrase_data = std::fs::read(bincode_path)?;
        
        // Build no_apos_map...
        
        Ok(Self {
            map: AHashMap::new(),
            fst_map: map,
            phrase_data: phrase_data,
            no_apos_map: no_apos,
            metadata: LexiconMetadata::default(),
        })
    }
    
    pub fn lookup(&self, key: &str) -> Vec<String> {
        if let Some(v) = self.map.get(key) {
            return v.clone();
        }
        
        if let Some(map) = &self.fst_map {
            if let Some(idx) = map.get(key) {
                if let Some(data) = &self.phrase_data {
                    // NEW: Direct bincode deserialization
                    // FST value encodes offset into phrase_data
                    let offset = idx as usize;
                    if let Ok(list) = bincode::deserialize::<Vec<PhraseEntry>>(&data[offset..]) {
                        return list.into_iter().map(|pe| pe.text).collect();
                    }
                }
            }
        }
        
        Vec::new()
    }
}
```

**Changes to `tools/src/convert_table.rs`:**

```rust
pub fn run(inputs: &[PathBuf], out_fst: &PathBuf, out_bincode: &PathBuf) -> Result<()> {
    // ... existing code to build global map ...
    
    // Build FST and create offset map
    let mut keys: Vec<String> = global.keys().cloned().collect();
    keys.sort();
    
    // Write all phrase lists to a single bincode file and track offsets
    let mut bincode_writer = BufWriter::new(File::create(out_bincode)?);
    let mut offsets: Vec<u64> = Vec::new();
    
    for k in keys.iter() {
        let offset = bincode_writer.stream_position()?;
        offsets.push(offset);
        
        let list = &global[k];
        let serialized = bincode::serialize(list)?;
        bincode_writer.write_all(&serialized)?;
    }
    bincode_writer.flush()?;
    
    // Build FST with offsets as values
    let mut fst_builder = fst::MapBuilder::new(Vec::new())?;
    for (i, k) in keys.iter().enumerate() {
        fst_builder.insert(k, offsets[i])?;
    }
    let fst_bytes = fst_builder.into_inner()?;
    std::fs::write(out_fst, &fst_bytes)?;
    
    Ok(())
}
```

### Phase 3: Simplify Interpolation Storage

**Option A:** Single bincode HashMap (Simple, Good for small data)
```rust
// Output: interpolation.bincode
let lambda_map: HashMap<String, f32> = compute_lambdas();
let serialized = bincode::serialize(&lambda_map)?;
std::fs::write("interpolation.bincode", serialized)?;

// Load:
let bytes = std::fs::read("interpolation.bincode")?;
let lambdas: HashMap<String, f32> = bincode::deserialize(&bytes)?;
```

**Option B:** Keep FST + bincode (Better for prefix searches)
```rust
// If you need to query by prefix efficiently, keep FST
// but replace ReDB with bincode array indexed by FST values
```

## Benefits Summary

### Performance Improvements
- **Faster lexicon lookups:** No transaction overhead
- **Smaller memory footprint:** No ReDB transaction buffers
- **Better caching:** OS can page in/out bincode files naturally
- **Simpler code:** Fewer error paths, no transaction management

### File Size Comparison (Estimated)
```
BEFORE (FST + ReDB):
pinyin.fst:     ~2 MB
pinyin.redb:    ~30 MB  (includes ReDB metadata, B-tree overhead)
TOTAL:          ~32 MB

AFTER (FST + Bincode):
pinyin.fst:     ~2 MB
pinyin.bincode: ~20 MB  (raw serialized data, no overhead)
TOTAL:          ~22 MB

SAVINGS: ~30% smaller on disk
```

### Code Simplification
- Remove ReDB dependency from `core` (stays in `userdict` only)
- Eliminate transaction handling in read-only paths
- Simpler error types (no ReDB errors in lexicon)
- Easier to debug (bincode files are portable)

## Migration Path

### Step 1: Add bincode output to convert_tables
- remove ReDB output for now
- Generate `.bincode` files
- Test that it works

### Step 2: Update Lexicon to prefer bincode
- Add `load_from_fst_bincode()` method
- remove `load_from_fst_redb()` as fallback
- Update examples to use bincode

### Step 3: Remove ReDB from Lexicon
- Delete `load_from_fst_redb()` method
- Remove ReDB dependency from core/Cargo.toml
- Update all tools to only generate bincode

### Step 4: Clean up
- Remove old `.redb` files from data/
- Update documentation
- Update .gitignore

## What NOT to Change

### Keep ReDB for:
1. **UserDict** (`core/src/userdict.rs`)
   - User learning data is mutable
   - Needs ACID guarantees
   - Concurrent write access required
   
2. **Any future user data:**
   - Custom phrases
   - Input history
   - User preferences/settings

## Questions for Discussion

1. Should we use `mmap` for large bincode files or just load into memory? NO
   - Memory: Simpler, works well for <100MB files
   - Mmap: More complex, better for very large dictionaries

2. For interpolation lambdas, FST+bincode or just HashMap?
   - FST: Better if we need prefix queries
   - HashMap: Simpler if we only do exact key lookups

3. Should PhraseEntry include more metadata? No
   - Part-of-speech tags
   - Word segmentation info
   - Pronunciation variants

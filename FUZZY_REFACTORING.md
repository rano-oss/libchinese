# Fuzzy Matching Refactoring

## Summary

Successfully moved fuzzy matching system from `libpinyin/src/fuzzy.rs` to `core/src/fuzzy.rs` to make it reusable across both libpinyin and libzhuyin crates.

## Changes Made

### 1. Core Module (`core/src/fuzzy.rs`)

**Created:** New generic fuzzy matching module in core
- **Key Changes:**
  - Removed pinyin-specific `with_standard_rules()` method
  - Changed `from_config()` to `from_rules()` accepting `&[String]` instead of `Config`
  - Removed lowercase conversion (to support bopomofo characters)
  - Made module generic and language-agnostic

**API:**
```rust
pub struct FuzzyMap { ... }
pub struct FuzzyRule { ... }

impl FuzzyMap {
    pub fn new() -> Self
    pub fn from_rules(rules: &[String]) -> Self
    pub fn add_rule(&mut self, from: &str, to: &str, penalty: f32)
    pub fn alternatives(&self, syllable: &str) -> Vec<(String, f32)>
    // ... other methods
}
```

### 2. Core Exports (`core/src/lib.rs`)

Added fuzzy module exports:
```rust
pub mod fuzzy;
pub use fuzzy::{FuzzyMap, FuzzyRule};
```

### 3. libpinyin Configuration (`libpinyin/src/lib.rs`)

**Added:** `standard_fuzzy_rules()` function that returns pinyin-specific fuzzy rules
- Shengmu (initial) confusions: zh/z, ch/c, sh/s, n/l, f/h, r/l, k/g (penalty 1.0)
- Composed syllables: zi/zhi, fan/fang, ben/beng, etc. (penalty 1.0)
- Yunmu (final) confusions: an/ang, en/eng, in/ing, ian/iang (penalty 1.0)
- Corrections: ng/gn, iu/iou, ui/uei, un/uen, etc. (penalty 1.5)
- V/U corrections: ju/jv, qu/qv, etc. (penalty 2.0)

**Total:** ~150+ fuzzy rules covering all upstream libpinyin patterns

**Removed:** `pub mod fuzzy;` and `pub use fuzzy::FuzzyMap;` (now from core)

### 4. libpinyin Engine (`libpinyin/src/engine.rs`)

**Updated:**
```rust
// Old:
use crate::fuzzy::FuzzyMap;
let fuzzy = FuzzyMap::with_standard_rules();

// New:
use libchinese_core::FuzzyMap;
let rules = crate::standard_fuzzy_rules();
let fuzzy = FuzzyMap::from_rules(&rules);
```

### 5. libpinyin Parser (`libpinyin/src/parser.rs`)

**Updated:**
```rust
// Old:
use crate::fuzzy::FuzzyMap;
fuzzy: FuzzyMap::with_standard_rules()

// New:
use libchinese_core::FuzzyMap;
let rules = crate::standard_fuzzy_rules();
fuzzy: FuzzyMap::from_rules(&rules)
```

### 6. libzhuyin Configuration (`libzhuyin/src/lib.rs`)

**Added:** Configuration functions for zhuyin fuzzy rules
```rust
pub fn standard_fuzzy_rules() -> Vec<String>  // HSU/ETEN26 keyboard corrections
pub fn no_fuzzy_rules() -> Vec<String>        // Empty rules (strict input)
```

**Example rules:**
- HSU layout: ㄓ/ㄐ, ㄔ/ㄑ, ㄕ/ㄒ, ㄢ/ㄇ, ㄣ/ㄋ (penalty 1.5)
- ETEN26 layout: similar corrections (penalty 1.5)

### 7. libzhuyin Engine (`libzhuyin/src/engine.rs`)

**Added:** Fuzzy support with two constructors
```rust
use libchinese_core::FuzzyMap;

pub struct Engine {
    fuzzy: FuzzyMap,
    // ... other fields
}

impl Engine {
    pub fn new(model: Model, parser: ZhuyinParser) -> Self {
        let rules = crate::standard_fuzzy_rules();
        let fuzzy = FuzzyMap::from_rules(&rules);
        // ...
    }
    
    pub fn with_fuzzy_rules(model: Model, parser: ZhuyinParser, 
                           fuzzy_rules: Vec<String>) -> Self {
        let fuzzy = FuzzyMap::from_rules(&fuzzy_rules);
        // ...
    }
}
```

**Note:** Fuzzy field is currently unused (TODO: integrate with parser)

### 8. libzhuyin Parser (`libzhuyin/src/parser.rs`)

**Removed:** Custom `ZhuyinFuzzy` implementation (68 lines)

**Updated:** Now uses `libchinese_core::FuzzyMap`
```rust
use libchinese_core::{TrieNode, FuzzyMap};

pub struct ZhuyinParser {
    trie: TrieNode,
    fuzzy: FuzzyMap,  // Changed from ZhuyinFuzzy
}

impl ZhuyinParser {
    pub fn new() -> Self {
        let rules = crate::standard_fuzzy_rules();
        Self {
            trie: TrieNode::new(),
            fuzzy: FuzzyMap::from_rules(&rules),
        }
    }
    
    pub fn with_fuzzy_rules(fuzzy_rules: Vec<String>) -> Self {
        Self {
            trie: TrieNode::new(),
            fuzzy: FuzzyMap::from_rules(&fuzzy_rules),
        }
    }
}
```

**Updated:** Fuzzy alternatives usage in `segment_best()`
```rust
// Old:
let alts = self.fuzzy.alternatives(&substr);
for alt in alts.into_iter() {
    let seg_cost = 1.5; // hardcoded penalty
}

// New:
let alts = self.fuzzy.alternatives(&substr);
for (alt, penalty) in alts.into_iter() {
    let seg_cost = penalty; // use configured penalty from rule
}
```

### 9. File Cleanup

**Deleted:** 
- `libpinyin/src/fuzzy.rs` (moved to core)
- Custom `ZhuyinFuzzy` implementation in `libzhuyin/src/parser.rs` (replaced with core FuzzyMap)

## Architecture Benefits

1. **Code Reuse:** Single fuzzy matching implementation shared by all input methods
2. **Maintainability:** Fuzzy logic updates benefit all crates automatically
3. **Flexibility:** Each input method configures its own language-specific rules
4. **Separation:** Core provides generic infrastructure, language crates provide domain knowledge
5. **Testability:** Core fuzzy tests cover all edge cases once

## Configuration Pattern

Each input method crate provides a `standard_fuzzy_rules()` function:

```rust
// libpinyin
pub fn standard_fuzzy_rules() -> Vec<String> {
    vec!["zh=z:1.0", "an=ang:1.0", "fan=fang:1.0", ...]
}

// libzhuyin
pub fn standard_fuzzy_rules() -> Vec<String> {
    vec!["ㄓ=ㄐ:1.5", "ㄔ=ㄑ:1.5", ...]
}
```

Users can also provide custom rules:
```rust
let custom_rules = vec!["a=b:2.0".to_string()];
let fuzzy = FuzzyMap::from_rules(&custom_rules);
```

## Test Results

All tests pass:
- ✅ `core/src/fuzzy.rs`: 7 tests passed
- ✅ `libpinyin`: 6 lib tests passed
- ✅ `libzhuyin`: 5 lib tests passed (including updated fuzzy test)
- ✅ Interactive example works: "nihao" → "你好" (score=-40.0)

## Warnings Fixed

- Removed unused `Interpolator` import from libzhuyin engine
- `fuzzy` field in libzhuyin::Engine currently unused (noted for future integration)

## Future Work

1. **libzhuyin parser integration:** Add fuzzy matching to ZhuyinParser
2. **Dynamic configuration:** Allow runtime fuzzy rule updates
3. **Performance optimization:** Cache fuzzy alternatives for common syllables
4. **Extended rules:** Add more keyboard layout-specific corrections for zhuyin

## Upstream Compatibility

The refactoring maintains full compatibility with upstream libpinyin's fuzzy matching patterns:
- All shengmu/yunmu confusions supported
- All composed syllable rules included
- Penalty system matches upstream behavior
- HSU/ETEN26 keyboard corrections for zhuyin

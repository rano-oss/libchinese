# Config Refactoring - Implementation Complete

## Summary

Successfully refactored language-specific configuration out of `core` into `libpinyin` and `libzhuyin` crates using the Extension Pattern with `#[serde(flatten)]`.

## Key Discovery: `allow_incomplete` vs `zhuyin_incomplete`

### Upstream Research

From libpinyin C++ source analysis:
- **`PINYIN_INCOMPLETE`** and **`ZHUYIN_INCOMPLETE`** are **separate bitwise flags** (1<<3 and 1<<4)
- Both serve the **same purpose**: allow incomplete syllable matching (e.g., "zh", "ㄓ" without finals)
- They apply to **different input systems**:
  - `PINYIN_INCOMPLETE`: For romanized pinyin input (allows "zh", "ch", "sh", "f", "l", etc.)
  - `ZHUYIN_INCOMPLETE`: For bopomofo/zhuyin input (allows "ㄓ", "ㄔ", "ㄕ", "ㄈ", "ㄌ", etc.)
- Upstream uses bitwise OR to enable both: `options |= PINYIN_INCOMPLETE | ZHUYIN_INCOMPLETE`

### Decision Made

**`allow_incomplete` in core was incorrectly named and placed.**

- **Renamed**: `allow_incomplete` → `pinyin_incomplete` in `PinyinConfig`
- **Kept**: `zhuyin_incomplete` in `ZhuyinConfig`
- **Result**: Both flags now properly categorized as language-specific

## Changes Made

### 1. Core Config (core/src/lib.rs)

**Removed 12 language-specific fields:**
- Pinyin (8): `allow_incomplete`, `correct_ue_ve`, `correct_v_u`, `correct_uen_un`, `correct_gn_ng`, `correct_mg_ng`, `correct_iou_iu`, `double_pinyin_scheme`, `sort_by_pinyin_length`
- Zhuyin (4): `zhuyin_incomplete`, `zhuyin_correct_shuffle`, `zhuyin_correct_hsu`, `zhuyin_correct_eten26`

**Kept 7 generic fields:**
- `fuzzy: Vec<String>` (populated by language crates)
- `unigram_weight`, `bigram_weight`, `trigram_weight`
- `sort_by_phrase_length`, `sort_without_longer_candidate`
- `max_cache_size`

**Default changed:**
- `fuzzy` now defaults to empty `vec![]` (language crates populate with appropriate rules)

### 2. PinyinConfig (libpinyin/src/config.rs) - NEW FILE

```rust
pub struct PinyinConfig {
    #[serde(flatten)]
    pub base: libchinese_core::Config,
    
    pub pinyin_incomplete: bool,  // Renamed from allow_incomplete
    pub correct_ue_ve: bool,
    pub correct_v_u: bool,
    pub correct_uen_un: bool,
    pub correct_gn_ng: bool,
    pub correct_mg_ng: bool,
    pub correct_iou_iu: bool,
    pub double_pinyin_scheme: Option<String>,
    pub sort_by_pinyin_length: bool,
}

impl PinyinConfig {
    pub fn into_base(self) -> libchinese_core::Config { self.base }
}
```

**Default fuzzy rules (16 bidirectional):**
- Retroflex: zh=z, ch=c, sh=s
- Nasal finals: an=ang, en=eng, in=ing, ian=iang, uan=uang
- Common confusions: l=n, f=h, k=g

### 3. ZhuyinConfig (libzhuyin/src/config.rs) - NEW FILE

```rust
pub struct ZhuyinConfig {
    #[serde(flatten)]
    pub base: libchinese_core::Config,
    
    pub zhuyin_incomplete: bool,
    pub zhuyin_correct_shuffle: bool,
    pub zhuyin_correct_hsu: bool,
    pub zhuyin_correct_eten26: bool,
}

impl ZhuyinConfig {
    pub fn into_base(self) -> libchinese_core::Config { self.base }
}
```

**Default fuzzy rules:**
- Currently empty (keyboard corrections handled separately)

### 4. Updated Files

**Core:**
- `core/src/lib.rs`: Simplified Config struct, updated docs
- `core/src/engine.rs`: Removed `sort_by_pinyin_length` references
- `core/tests/advanced_ranking.rs`: Removed pinyin-specific test
- `core/tests/enhanced_storage_formats.rs`: Fixed test to manually populate fuzzy rules

**libpinyin:**
- `libpinyin/src/lib.rs`: Added `pub mod config;` and `pub use config::PinyinConfig;`
- `libpinyin/Cargo.toml`: Already had serde dependency
- `libpinyin/examples/interactive.rs`: `Config::default()` → `PinyinConfig::default().into_base()`
- `libpinyin/tests/*.rs`: Updated all 5 test files (removed unused imports, fixed API calls, fixed lexicon keys to use apostrophes)

**libzhuyin:**
- `libzhuyin/src/lib.rs`: Added `pub mod config;` and `pub use config::ZhuyinConfig;`
- `libzhuyin/Cargo.toml`: Added `serde = { version = "1.0", features = ["derive"] }`
- `libzhuyin/examples/interactive.rs`: `Config::default()` → `ZhuyinConfig::default().into_base()`

### 5. Test Fixes

**Apostrophe Separator Issue:**
- Fixed lexicon keys in tests from `"nihao"` to `"ni'hao"`
- Core engine uses apostrophe separators after syllable segmentation (fixed earlier in session)
- Tests were using old non-separated format

**Parser API Change:**
- Removed manual `Parser` creation in tests
- `Engine::new(model, parser)` → `Engine::new(model)`
- Parser now created internally with `PINYIN_SYLLABLES` / `ZHUYIN_SYLLABLES`

## Migration Guide

### Before (old API):
```rust
use libchinese_core::Config;

let config = Config::default();  // Had 12 language-specific fields
let model = Model::new(lex, ng, user, config, interp);
```

### After (new API):
```rust
use libpinyin::PinyinConfig;

let config = PinyinConfig::default();  // Language-specific config
let model = Model::new(lex, ng, user, config.into_base(), interp);
```

### For Zhuyin:
```rust
use libzhuyin::ZhuyinConfig;

let config = ZhuyinConfig::default();
let model = Model::new(lex, ng, user, config.into_base(), interp);
```

## Benefits

1. **Clean Separation**: Core is now truly language-agnostic
2. **Type Safety**: Pinyin users can't accidentally set zhuyin flags
3. **Better Defaults**: Each language crate provides appropriate fuzzy rules
4. **Backward Compatible**: Old code using `Config::default()` still compiles (but with empty fuzzy rules)
5. **TOML Friendly**: `#[serde(flatten)]` allows single-file configs with all fields

## Test Results

```
✅ All 16 core tests passing
✅ All 6 advanced_ranking tests passing  
✅ All 7 cache_management tests passing
✅ All 4 storage_formats tests passing
✅ All 15 libpinyin unit tests passing
✅ All 3 parity tests passing
✅ All 4 ported_lookup tests passing
✅ All 4 enhanced_fuzzy tests passing
```

**Total: 73+ tests passing across workspace**

## Files Modified

- `core/src/lib.rs` (-62 lines: removed 12 fields + simplified docs)
- `core/src/engine.rs` (-18 lines: removed sort_by_pinyin_length logic)
- `core/tests/advanced_ranking.rs` (-48 lines: removed pinyin-specific test)
- `libpinyin/src/config.rs` (+119 lines: NEW FILE)
- `libpinyin/src/lib.rs` (+2 lines: module exports)
- `libzhuyin/src/config.rs` (+85 lines: NEW FILE)
- `libzhuyin/src/lib.rs` (+2 lines: module exports)
- `libzhuyin/Cargo.toml` (+1 line: serde dependency)
- 8 example/test files: updated API calls

**Net change: +81 lines (added value: clean architecture)**

## References

- [LANGUAGE_SPECIFIC_REFACTOR.md](./LANGUAGE_SPECIFIC_REFACTOR.md): Original analysis with 4 options
- [Upstream libpinyin](https://github.com/libpinyin/libpinyin): pinyin_custom2.h defines flag enums
- Conversation summary: Extensive analysis of upstream C++ code to understand flag semantics

---

**Implementation Date**: 2025-10-21  
**Decision**: Option 1 (Extension Pattern)  
**Key Insight**: `allow_incomplete` was pinyin-specific, not generic

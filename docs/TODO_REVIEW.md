# TODO Review for libchinese Project

Date: October 16, 2025
Branch: feat/dex

## Summary

Found **7 TODO/FIXME references** across the codebase. Analysis below:

---

## 1. Parser TODOs (libpinyin/src/parser.rs)

### Location: Lines 5-16

```rust
// - Fuzzy map placeholder (TODO: full fuzzy handling)
//
// TODOs:
// - Port the exact DP recurrence and cost model from `pinyin_parser2.cpp` for parity.
// - Implement full fuzzy substitution handling (insertion/substitution of letters like `zh` <-> `z`).
// - Expose segmentation alternatives and scores (currently we return a single best segmentation).
// - Add comprehensive unit tests ported from `tests/storage/test_parser2.cpp`.
```

### Status: **MOSTLY RESOLVED ✅**

**Completed:**
- ✅ Full fuzzy handling is now implemented in `engine.rs` with comprehensive fuzzy rules
- ✅ DP recurrence implemented with beam search in `segment_top_k()`
- ✅ Segmentation alternatives are exposed via `segment_top_k()` returning `Vec<Vec<Syllable>>`
- ✅ Cost model includes penalties and distance tracking

**Still Relevant:**
- ⚠️ "Port exact DP recurrence" - current implementation is functional but may differ from upstream
- ⚠️ "Add comprehensive unit tests ported from upstream" - we have tests but not exhaustive upstream parity tests

### Recommendation: **UPDATE** the comment to reflect current state:

```rust
// libchinese/libpinyin/src/parser.rs
//
// Pinyin parser skeleton for libpinyin port.
// - DP-based segmentation with beam search
// - Fuzzy matching integrated via Engine
//
// References (upstream C++):
// - src/storage/pinyin_parser2.cpp
// - src/storage/pinyin_parser_table.h
// - src/pinyin.cpp
//
// Future enhancements:
// - Verify exact parity with upstream DP cost model
// - Add comprehensive test vectors from upstream test suite
```

---

## 2. Engine Fuzzy Module Comment (libpinyin/src/engine.rs)

### Location: Line 357

```rust
/// These implementations are intentionally minimal and documented with TODOs for
/// the next phases.
```

### Status: **OBSOLETE ❌**

The fuzzy module is no longer "minimal" - it has:
- Comprehensive shengmu/yunmu fuzzy rules (zh/z, ch/c, sh/s, an/ang, etc.)
- Composed alternatives generation
- Integration with segmentation
- Penalty system

### Recommendation: **REMOVE** - replace with accurate description:

```rust
/// Fuzzy and tables modules provide language-specific behavior for pinyin.
///
/// The fuzzy module implements comprehensive phonetic variation handling
/// for common pinyin confusions (zh/z, ch/c, an/ang, en/eng, etc.).
```

---

## 3. Build Command TODOs

### Location: 
- `libpinyin/src/main.rs` line 296
- `libzhuyin/src/main.rs` line 249

```rust
// TODO: Implement actual building logic
println!("⚠️  Model building not yet implemented - placeholder for Step 7");
```

### Status: **STILL RELEVANT ⚠️**

The `build` subcommand is a placeholder. Current workflow uses external tools:
- `convert_tables` for lexicon building
- `serialize_ngram` for n-gram model building
- `estimate_interpolation` for lambda estimation

### Options:

**Option A: Keep TODO** - Building is planned future work

**Option B: Remove stub** - Building is handled by tools, not needed in CLI

**Option C: Implement wrapper** - Call existing tools from CLI

### Recommendation: **KEEP** but clarify:

```rust
// Building is handled by external tools in the tools/ directory.
// See tools/README.md for model building workflow.
// TODO: Consider adding CLI wrappers for common build tasks
println!("ℹ️  Model building is handled by tools in tools/ directory");
println!("   See tools/README.md for building lexicons and n-gram models");
```

---

## 4. Convert Command TODOs

### Location:
- `libpinyin/src/main.rs` line 586
- `libzhuyin/src/main.rs` line 307

```rust
// TODO: Implement actual conversion logic
println!("⚠️  Format conversion not yet implemented - placeholder for Step 7");
```

### Status: **STILL RELEVANT ⚠️**

Format conversion is a placeholder. Current models are in fixed formats (FST+redb, bincode).

### Recommendation: **CLARIFY OR REMOVE**

If format conversion isn't a priority, remove the stub entirely:

```rust
// Remove the entire Convert subcommand from Cli struct
// Or implement basic conversions if needed
```

---

## Summary Table

| Location | Type | Status | Action |
|----------|------|--------|--------|
| `libpinyin/src/parser.rs:5-16` | Feature TODO | Mostly done | UPDATE comment |
| `libpinyin/src/engine.rs:357` | Outdated comment | Obsolete | REMOVE/UPDATE |
| `libpinyin/src/main.rs:296` | Unimplemented feature | Relevant | CLARIFY or DELEGATE |
| `libpinyin/src/main.rs:586` | Unimplemented feature | Relevant | CLARIFY or REMOVE |
| `libzhuyin/src/main.rs:249` | Unimplemented feature | Relevant | CLARIFY or DELEGATE |
| `libzhuyin/src/main.rs:307` | Unimplemented feature | Relevant | CLARIFY or REMOVE |

---

## Recommended Actions

### High Priority (Misleading Documentation)

1. **Update `libpinyin/src/parser.rs` header comments** - Reflect actual implementation
2. **Update `libpinyin/src/engine.rs:357` comment** - Fuzzy module is feature-complete

### Medium Priority (CLI Stubs)

3. **Clarify build/convert commands** - Either implement or document external workflow

### Low Priority

4. **Consider adding upstream test parity** - Port upstream test vectors if needed

---

## Proposed Changes Script

```rust
// 1. libpinyin/src/parser.rs - Update header
// 2. libpinyin/src/engine.rs - Update fuzzy module comment  
// 3. libpinyin/src/main.rs - Clarify build command
// 4. libpinyin/src/main.rs - Remove or implement convert command
// 5. libzhuyin/src/main.rs - Mirror libpinyin changes
```

Would you like me to implement these changes?

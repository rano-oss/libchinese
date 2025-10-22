# Wade-Giles Branch Migration

## Summary

Successfully moved Wade-Giles to Pinyin conversion support to a separate feature branch `feat/wade-giles`.

## Actions Taken

### 1. Created Feature Branch
```bash
git checkout -b feat/wade-giles
```

The `feat/wade-giles` branch contains:
- `libpinyin/src/wade_giles.rs` - Full Wade-Giles to Pinyin conversion module
- `libpinyin/examples/wade_giles_input.rs` - Interactive demonstration
- Module exports in `libpinyin/src/lib.rs`

### 2. Removed from Main Branch
```bash
git checkout main
git rm libpinyin/src/wade_giles.rs
git rm libpinyin/examples/wade_giles_input.rs
# Modified libpinyin/src/lib.rs to remove wade_giles module
git commit -m "refactor: move Wade-Giles support to separate branch"
```

### 3. Test Verification
```
✅ All workspace tests passing after removal
✅ No broken dependencies
✅ Clean compilation
```

## Branch Status

### main (current)
- **Commit**: `6463ebe` - "refactor: move Wade-Giles support to separate branch"
- **Content**: Core IME functionality without Wade-Giles
- **Tests**: All passing

### feat/wade-giles
- **Base**: `ac5d3a6` - Includes Config refactoring + Wade-Giles
- **Content**: Full Wade-Giles to Pinyin conversion support
- **Features**:
  - 70+ Wade-Giles to Pinyin mappings
  - Aspirated consonant handling (p'/k'/t'/ch')
  - Hyphen processing
  - Interactive example program

## Rationale

Wade-Giles romanization is:
1. **Legacy system**: Superseded by Hanyu Pinyin (1958)
2. **Niche use case**: Primarily for historical texts and academic contexts
3. **Not core IME**: Modern Chinese input uses Pinyin, Zhuyin, or Wubi
4. **Experimental**: Implementation may need refinement based on real-world usage

## Usage

### To use Wade-Giles support:
```bash
git checkout feat/wade-giles
cargo run --example wade_giles_input
```

### To merge back into main later (if needed):
```bash
git checkout main
git merge feat/wade-giles
```

## Files Removed from Main

1. **libpinyin/src/wade_giles.rs** (269 lines)
   - Conversion mappings
   - Syllable conversion logic
   - Input processing functions
   - Tests for conversions

2. **libpinyin/examples/wade_giles_input.rs** (91 lines)
   - Interactive demonstration
   - Example conversions (Beijing → Pei-ching)
   - Usage instructions

3. **libpinyin/src/lib.rs** (1 line)
   - Removed `pub mod wade_giles;`

**Total**: 361 lines removed from main branch

## Future Considerations

If Wade-Giles support is needed in production:

1. **Merge Strategy**: Cherry-pick or merge `feat/wade-giles` into main
2. **Feature Flag**: Consider making it an optional Cargo feature
3. **Documentation**: Add to README with clear "experimental" label
4. **Testing**: Expand test coverage for edge cases
5. **Performance**: Profile conversion overhead for IME integration

## Related Commits

- **Config Refactoring**: Already merged in both branches
  - PinyinConfig / ZhuyinConfig separation
  - Language-specific vs generic fields
  - All 73+ tests passing

---

**Branch Created**: 2025-10-22  
**Reason**: Separate experimental feature from core IME  
**Status**: Clean separation, all tests passing  
**Reversible**: Yes, via git merge

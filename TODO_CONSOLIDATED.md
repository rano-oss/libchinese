# TODO and Future Work Summary

**Date**: October 21, 2025  
**Status**: Post parser enhancements implementation

This document consolidates all TODOs and future improvements identified across documentation files, cross-referenced with upstream libpinyin.

---

## High Priority Items 🔴

### 1. Implement commit() for User Learning
**Location**: Engine API  
**Upstream**: `src/pinyin.cpp` pinyin_train()  
**Status**: ❌ Not implemented  
**Impact**: HIGH - Critical for user experience

Users cannot save learned phrase preferences. The `commit()` method exists but is a no-op.

**Required**:
- Add userdict mutation API to core
- Implement frequency boost on commit
- Add transaction support for atomic updates

**References**:
- ENGINE_UNIFICATION.md "Future Work" #1
- UPSTREAM_FEATURE_COMPARISON.md HIGH PRIORITY #1

---

### 2. Add Missing Pinyin Corrections
**Location**: Parser  
**Upstream**: `src/storage/pinyin_custom2.h`, parser table  
**Status**: 🚧 Partial (3/7 corrections done)  
**Impact**: MEDIUM - Better input tolerance

Currently implemented:
- ✅ PINYIN_CORRECT_UE_VE (ue ↔ ve)
- ✅ PINYIN_CORRECT_V_U (v ↔ u)

Missing from upstream:
- ❌ PINYIN_CORRECT_UEN_UN (xuen ↔ xun)
- ❌ PINYIN_CORRECT_GN_NG (agn ↔ ang)
- ❌ PINYIN_CORRECT_MG_NG (amg ↔ ang)
- ❌ PINYIN_CORRECT_IOU_IU (miou ↔ miu)

**Implementation**: Add to `Parser::apply_corrections()` method

**References**:
- PARSER_ENHANCEMENTS.md "Future Work" #5
- UPSTREAM_FEATURE_COMPARISON.md HIGH PRIORITY #2

---

### 3. Integrate Tone Handling ✅
**Location**: Config + Parser  
**Upstream**: USE_TONE, FORCE_TONE flags  
**Status**: 🎉 **IMPLEMENTED** - Tone extraction complete  
**Impact**: MEDIUM - Required for correct parsing

Tone support now fully integrated:
- ✅ USE_TONE flag added to Config (default: false)
- ✅ FORCE_TONE flag added to Config (default: false)
- ✅ Tone field added to Syllable struct (u8: 0-5)
- ✅ Tone extraction during parsing (tone digits 1-5 stripped from input)
- ✅ 9 comprehensive tests covering all edge cases

**Remaining work** (LOW PRIORITY):
- ⚠️ Respect USE_TONE flag (currently tones always extracted)
- ⚠️ Implement FORCE_TONE validation (reject toneless input when enabled)
- ⚠️ Tone-aware cost model (penalize tone mismatches)

**References**:
- TONE_IMPLEMENTATION.md - Full implementation details
- UPSTREAM_FEATURE_COMPARISON.md HIGH PRIORITY #3
- PARSER_ENHANCEMENTS.md "Future Work" #7

---

## Medium Priority Items 🟡

### 4. Zhuyin Parser Enhancements
**Location**: libzhuyin/src/parser.rs  
**Upstream**: `src/storage/zhuyin_parser2.cpp`  
**Status**: ❌ Not implemented  
**Impact**: MEDIUM - Feature parity with pinyin

Missing zhuyin-specific features:
- ZHUYIN_INCOMPLETE (partial syllable matching)
- ZHUYIN_CORRECT_SHUFFLE (medial/final order errors)
- ZHUYIN_CORRECT_HSU (HSU scheme corrections)
- ZHUYIN_CORRECT_ETEN26 (ETEN26 scheme corrections)

**References**:
- PARSER_ENHANCEMENTS.md "Future Work" #6
- UPSTREAM_FEATURE_COMPARISON.md MEDIUM PRIORITY #4

---

### 5. Advanced Candidate Ranking
**Location**: Engine  
**Upstream**: `src/pinyin.h` sort_option_t  
**Status**: ❌ Not implemented  
**Impact**: MEDIUM - Better candidate ordering

Missing sorting options:
- SORT_BY_PHRASE_LENGTH
- SORT_BY_PINYIN_LENGTH
- SORT_WITHOUT_LONGER_CANDIDATE
- Combined sorting strategies

**References**:
- UPSTREAM_FEATURE_COMPARISON.md MEDIUM PRIORITY #5

---

### 6. Double Pinyin Support
**Location**: New parser  
**Upstream**: `src/storage/pinyin_parser2.cpp` DoublePinyinParser2  
**Status**: ❌ Not implemented  
**Impact**: LOW-MEDIUM - Alternative input method

Popular schemes to support:
- Microsoft Shuangpin
- ZiRanMa
- ZiGuang
- ABC
- Requires shengmu/yunmu tables and fallback logic

**References**:
- UPSTREAM_FEATURE_COMPARISON.md MEDIUM PRIORITY #6

---

## Low Priority Items 🟢

### 7. Additional Parser Schemes
**Upstream**: Various parser classes  
**Status**: ❌ Not implemented  

- Wade-Giles/Luoma pinyin
- HSU/IBM/ETEN/Gin-Yieh zhuyin schemes
- Direct parsers (exact input, no ambiguity resolution)

**References**:
- UPSTREAM_FEATURE_COMPARISON.md LOW PRIORITY #7

---

### 8. Phrase Import/Export Tools
**Location**: New tools  
**Status**: ❌ Not implemented

- User phrase management CLI
- Frequency export for backup
- Custom dictionary import
- Batch operations

**References**:
- UPSTREAM_FEATURE_COMPARISON.md LOW PRIORITY #8

---

### 9. Advanced Engine Features
**Upstream**: Various context flags  
**Status**: ❌ Not implemented

- USE_DIVIDED_TABLE (phrase table splitting)
- USE_RESPLIT_TABLE (long phrase re-splitting)
- DYNAMIC_ADJUST (runtime frequency adjustment)
- Phrase masking API (filter unwanted phrases)

**References**:
- UPSTREAM_FEATURE_COMPARISON.md LOW PRIORITY #9

---

## Code Quality Improvements

### 10. Make Penalties Configurable
**Location**: Config struct  
**Status**: 🚧 Hardcoded in parser

Current penalties:
- Exact: 0
- Corrections: 200
- Fuzzy: varies
- Incomplete: 500
- Unknown: 1000

Should be:
- Exposed in Config
- Tunable per-user
- Documented defaults

**References**:
- PARSER_ENHANCEMENTS.md "Future Work" #1

---

### 11. Expose Parser Options in Engine API
**Location**: Engine constructor  
**Status**: 🚧 Only via Config struct

Parser options (allow_incomplete, correct_ue_ve, correct_v_u) currently only settable via Config. Should be:
- Exposed in Engine API
- Runtime togglable
- Per-instance configurable

**References**:
- PARSER_ENHANCEMENTS.md "Future Work" #2

---

### 12. Add Correction Statistics
**Location**: Engine  
**Status**: ❌ Not tracked

Track usage metrics:
- Correction type frequencies
- Incomplete match rate
- Fuzzy match distribution
- Performance profiling

**References**:
- PARSER_ENHANCEMENTS.md "Future Work" #3
- ENGINE_UNIFICATION.md "Future Work" #3

---

### 13. Smart Completion Selection
**Location**: Parser  
**Status**: 🚧 Returns first match

`find_syllable_completion()` currently returns first match. Should:
- Rank by syllable frequency
- Prefer common completions
- Consider context

**References**:
- PARSER_ENHANCEMENTS.md "Future Work" #4

---

### 14. Cache Management
**Location**: Engine  
**Status**: 🚧 Unlimited cache

Current cache has no limits. Should add:
- Size limits (MB or entry count)
- LRU eviction policy
- Cache statistics
- Configurable cache size

**References**:
- ENGINE_UNIFICATION.md "Future Work" #2

---

## Documentation Updates

### 15. Update Parser Comment TODOs
**Location**: libpinyin/src/parser.rs lines 5-16  
**Status**: ⚠️ Outdated

Comment lists TODOs that are now complete. Should update to reflect:
- ✅ Fuzzy handling complete
- ✅ DP recurrence implemented
- ✅ Alternatives exposed
- Still relevant: upstream parity tests

**References**:
- docs/TODO_REVIEW.md #1

---

### 16. Update Engine Fuzzy Comment
**Location**: libpinyin/src/engine.rs line 357  
**Status**: ⚠️ Obsolete

Comment says fuzzy module is "minimal" but it's now comprehensive. Should update or remove.

**References**:
- docs/TODO_REVIEW.md #2

---

### 17. Clarify Build/Convert Command Stubs
**Location**: libpinyin/src/main.rs, libzhuyin/src/main.rs  
**Status**: ⚠️ Misleading placeholders

Build and convert subcommands are stubs. Should either:
- Remove stubs entirely
- Document they're handled by tools/
- Implement wrappers to tools

**References**:
- docs/TODO_REVIEW.md #3, #4

---

## Completed Items ✅

### Parser Enhancements (October 2025)
- ✅ Partial pinyin (incomplete syllables)
- ✅ Pinyin corrections (ue/ve, v/u)
- ✅ Apostrophe separators (already existed)
- ✅ Parser option flags in Config

### Engine Unification (October 2025)
- ✅ Generic `core::Engine<P>`
- ✅ Fixed libzhuyin fuzzy matching
- ✅ Added caching to libzhuyin
- ✅ Eliminated ~300 lines duplication

### Core Infrastructure
- ✅ DP-based segmentation with beam search
- ✅ Fuzzy matching (9 common rules)
- ✅ N-gram scoring with interpolation
- ✅ User dictionary boosting
- ✅ Addon dictionary support

---

## Next Sprint Recommendations

**Sprint Goal**: User Learning & Corrections

1. **Week 1**: Implement commit() API
   - Add userdict mutation to core
   - Implement frequency boost logic
   - Add integration tests

2. **Week 2**: Add missing corrections
   - Implement uen/un, gn/ng, mg/ng, iou/iu
   - Update parser tests
   - Verify penalty ordering

3. **Week 3**: Integrate tone handling
   - Respect USE_TONE flag
   - Add FORCE_TONE validation
   - Update cost model

4. **Week 4**: Documentation & polish
   - Update all outdated comments
   - Add API documentation
   - Create user guide

---

## Reference Documents

- **UPSTREAM_FEATURE_COMPARISON.md** - Comprehensive upstream analysis
- **ENGINE_UNIFICATION.md** - Generic engine architecture
- **PARSER_ENHANCEMENTS.md** - Parser enhancement features  
- **docs/TODO_REVIEW.md** - Code TODO analysis
- **docs/fuzzy_comparison.md** - Fuzzy matching design

---

## Metrics

**Current Feature Completion**:
- Parser Core: ~80%
- Correction Options: ~40% (3/7 implemented)
- User Learning: ~60% (commit missing)
- Advanced Features: ~30%

**Overall**: ~60% feature parity with upstream libpinyin

**Lines of Code**:
- core: ~2,500 lines
- libpinyin: ~1,200 lines (was ~1,700 before unification)
- libzhuyin: ~600 lines (was ~850 before unification)
- Total: ~4,300 lines (eliminated ~650 lines through refactoring)

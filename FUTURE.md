## Additional Parser Schemes

### 🟢 Additional Zhuyin Input Schemes
**Priority**: Low (niche feature)  
**Effort**: Medium (~2-3 weeks)  
**Status**: Not started (Standard Bopomofo complete)

**Missing schemes** (from upstream `zhuyin_parser2.cpp`):
- HSU Zhuyin
- IBM Zhuyin
- Gin-Yieh Zhuyin
- ETEN Zhuyin
- ETEN26 Zhuyin
- Dachen CP26
- Standard Dvorak Zhuyin
- HSU Dvorak
- Zhuyin Direct

**Implementation notes**:
- Follow double pinyin pattern: scheme tables with key mappings
- Add tests per scheme (similar to double pinyin)
- Consider if these schemes are actually used in practice

**References**:
- `src/storage/zhuyin_parser2.cpp` lines 270-292, 461-482 (upstream)
- `libpinyin/src/double_pinyin.rs` (implementation pattern)

### 🟢 Phrase Masking API
**Priority**: Low  
**Effort**: Small (~1 week)  
**Status**: Not implemented

**Features**:
- Filter out unwanted/offensive phrases
- User-configurable blocklist
- Useful for content filtering applications

**Implementation**:
- Add `masked_phrases: HashSet<String>` to Config
- Filter candidates in Engine::get_candidates()
- API: `engine.mask_phrase("phrase")`, `engine.unmask_phrase("phrase")`

**References**:
- Upstream: `src/pinyin.cpp` phrase masking functions

---

### 🔵 Bigram Learning
**Priority**: Polish (incremental improvement)  
**Effort**: Medium (~2 weeks)  
**Status**: Not implemented (static bigram model only)

**Features**:
- Learn bigram relationships from user input
- Update bigram frequencies on commit()
- Improve context-aware predictions over time

**Implementation**:
- Extend UserDict to store bigrams
- Update commit() to record phrase pairs
- Merge user bigrams with static model during scoring

**References**:
- Upstream: `src/pinyin.cpp` lines 618-641

**Files**: `tools/import_phrases/main.rs`

## Code Quality & Configuration

### 🟡 Make Penalties Configurable
**Priority**: Medium (affects user experience)  
**Effort**: Small (~1 week)  
**Status**: Hardcoded in parser

**Current penalties** (hardcoded):
- Exact match: 0
- Corrections: 200
- Fuzzy: varies
- Incomplete: 500
- Unknown: 1000

**Implementation**:
- Add penalties struct to Config:
  ```rust
  pub struct PenaltyConfig {
      pub exact: i32,
      pub correction: i32,
      pub fuzzy: i32,
      pub incomplete: i32,
      pub unknown: i32,
  }
  ```
- Update parser to use Config penalties
- Document defaults and tuning guidelines

**Files**: `core/src/lib.rs` (Config), `libpinyin/src/parser.rs`

---

### 🟡 Expose Parser Options in Engine API
**Priority**: Medium  
**Effort**: Small (~1 week)  
**Status**: Only via Config struct

**Current**: Parser options (allow_incomplete, correct_ue_ve, correct_v_u) only settable via Config  
**Requested**: Runtime-togglable methods on Engine

**Implementation**:
```rust
impl Engine {
    pub fn set_allow_incomplete(&mut self, allow: bool) { ... }
    pub fn set_correction(&mut self, correction_type: CorrectionType, enabled: bool) { ... }
}
```

**Files**: `libpinyin/src/engine.rs`, `core/src/lib.rs`

---

### 🟢 Correction Statistics
**Priority**: Low (debugging/analytics)  
**Effort**: Medium (~2 weeks)  
**Status**: Not tracked

**Features**:
- Track how often each correction type is used
- Incomplete match rate
- Fuzzy match distribution
- Performance profiling (time per query)

**Implementation**:
- Add statistics struct to Engine:
  ```rust
  pub struct Statistics {
      pub exact_matches: u64,
      pub corrections: HashMap<String, u64>, // correction type → count
      pub fuzzy_matches: u64,
      pub incomplete_matches: u64,
      pub cache_hits: u64,
      pub cache_misses: u64,
      pub avg_query_time: Duration,
  }
  ```
- Increment counters in get_candidates()
- Add `engine.get_statistics()` method

**Files**: `libpinyin/src/engine.rs`

---

### 🟢 Smart Completion Selection
**Priority**: Low  
**Effort**: Medium (~2 weeks)  
**Status**: Returns first match

**Current behavior**: `find_syllable_completion()` returns first valid completion  
**Requested**: Rank by syllable frequency

**Implementation**:
- Build syllable frequency table from lexicon
- Sort completions by frequency
- Prefer common completions (e.g., "zh" → "zhi" not "zhe")

**Files**: `libpinyin/src/parser.rs`

---

### 🔵 FORCE_TONE Validation
**Priority**: Polish  
**Effort**: Small (~1 week)  
**Status**: Not implemented

**Feature**: When `FORCE_TONE` flag is set, reject input without tone markers

**Implementation**:
- Check for tone markers (1-5 or ā/á/ǎ/à) in parser
- Return error or empty results if tones missing
- Add Config flag and tests

**Files**: `libpinyin/src/parser.rs`, `core/src/lib.rs`

---

## Documentation & Polish

### 🔴 Update Outdated Code Comments
**Priority**: High (misleading documentation)  
**Effort**: Small (~1 day)  
**Status**: Several TODO comments are obsolete

**Files to update**:

1. **`libpinyin/src/parser.rs` lines 5-16**:
   - ❌ Current: "TODO: full fuzzy handling" (already done!)
   - ✅ Update: Reflect DP recurrence, fuzzy integration, alternatives

2. **`libpinyin/src/engine.rs` line 357**:
   - ❌ Current: "intentionally minimal and documented with TODOs"
   - ✅ Update: Fuzzy module is comprehensive

3. **`libpinyin/src/main.rs` line 296**:
   - ❌ Current: "TODO: Implement actual building logic"
   - ✅ Update: Clarify that tools/ handles building

4. **`libpinyin/src/main.rs` line 586**:
   - ❌ Current: "TODO: Implement actual conversion logic"
   - ✅ Consider: Remove stub or clarify purpose

5. **`libzhuyin/src/main.rs` lines 249, 307**:
   - ❌ Current: Similar build/convert TODOs
   - ✅ Update: Mirror libpinyin clarifications

**References**: `docs/TODO_REVIEW.md` (comprehensive analysis)

---

### 🟡 User Guide & Tutorials
**Priority**: Medium (for onboarding)  
**Effort**: Medium (~2 weeks)  
**Status**: Only API documentation exists

**Content needed**:
- Getting started guide
- Basic usage examples
- Integration tutorials (desktop environments, terminals)
- Configuration guide (penalties, fuzzy rules, etc.)
- Troubleshooting common issues

**Format**: Markdown in `docs/` directory

---

### 🟢 API Documentation
**Priority**: Low (code is self-documenting)  
**Effort**: Medium (~1-2 weeks)  
**Status**: Minimal rustdoc comments

**Tasks**:
- Add rustdoc comments to all public APIs
- Examples in doc comments
- Generate docs with `cargo doc --no-deps --open`
- Publish to docs.rs when crates are released

---

### 🔵 Architecture Documentation
**Priority**: Polish  
**Effort**: Small (~1 week)  
**Status**: Only copilot-instructions.md

**Content**:
- System architecture overview
- Data flow diagrams
- Module responsibilities
- Extension points for contributors

**File**: `docs/ARCHITECTURE.md`

---

## Testing & Validation

### 🟡 Upstream Test Parity
**Priority**: Medium (validation)  
**Effort**: Large (~4-6 weeks)  
**Status**: Functional tests exist, not exhaustive upstream parity

**Tasks**:
- Port test vectors from `tests/storage/test_parser2.cpp`
- Port test vectors from `tests/lookup/test_*.cpp`
- Compare outputs with upstream for identical inputs
- Document any intentional differences

**Benefits**: Confidence in behavioral parity

---

### 🟢 Fuzzy Matching Phase 5
**Priority**: Low  
**Effort**: Large (~3-4 weeks)  
**Status**: Basic fuzzy rules implemented

**Phase 5 features from `docs/fuzzy_comparison.md`**:
- Pre-computed fuzzy tables (faster lookups)
- Granular per-rule options (enable/disable specific rules)
- Distance metrics (edit distance for ranking)
- Component composition (proper syllable decomposition)
- Complete rule set from upstream (100% coverage)

**Implementation**:
- Build pre-computed fuzzy tables at build time
- Add per-rule Config flags
- Implement Levenshtein distance for fuzzy ranking
- Parser-level syllable decomposition

**References**: `docs/fuzzy_comparison.md`

---

### 🟢 Benchmark Suite
**Priority**: Low  
**Effort**: Medium (~2 weeks)  
**Status**: No formal benchmarks

**Features**:
- Criterion-based benchmarks
- Measure query latency (p50, p95, p99)
- Cache hit rate measurements
- Memory usage profiling
- Compare with upstream C++ library

**Files**: `benches/` directory

---

### 🔵 Fuzzing
**Priority**: Polish (robustness)  
**Effort**: Medium (~2 weeks)  
**Status**: No fuzzing

**Tools**: cargo-fuzz / AFL  
**Targets**:
- Parser with random pinyin input
- FST lookups with malformed keys
- Ngram scoring with edge cases
- Redb corruption handling

---

## Deployment & Distribution

### 🔴 Platform-Specific Packages
**Priority**: High (for adoption)  
**Effort**: Medium (~2-3 weeks)  
**Status**: Rust binaries only

**Packages**:
- **Windows**: MSI installer, winget package
- **Linux**: .deb (Debian/Ubuntu), .rpm (Fedora/RHEL), AUR (Arch)
- **macOS**: Homebrew formula, .dmg installer

**Integration**:
- IBus module (Linux)
- TSF module (Windows)
- Input source (macOS)

---

### 🟡 Desktop Environment Integration
**Priority**: Medium  
**Effort**: Large (~4-8 weeks)  
**Status**: CLI only

**Integrations**:
- **IBus** (GNOME, KDE on Linux)
- **Fcitx5** (Linux)
- **Windows TSF** (Text Services Framework)
- **macOS Input Sources**

**Requires**: FFI layer, platform-specific UI, system integration

---

### 🟡 Configuration GUI
**Priority**: Medium (usability)  
**Effort**: Large (~4-6 weeks)  
**Status**: Configuration via code only

**Features**:
- Fuzzy matching toggles
- Penalty adjustments
- Dictionary management (import/export)
- Scheme selection (double pinyin, zhuyin)
- Visual feedback for settings

**Tech**: egui (Rust GUI) or platform-native

---

### 🟢 Crate Publishing
**Priority**: Low (visibility)  
**Effort**: Small (~1 week)  
**Status**: Not published to crates.io

**Tasks**:
- Finalize crate metadata (description, keywords, license)
- Write README.md for each crate
- Publish to crates.io:
  - `libchinese-core`
  - `libpinyin`
  - `libzhuyin`
- Set up CI/CD for releases

---

### 🟢 Language Server Protocol (LSP)
**Priority**: Low (editor integration)  
**Effort**: Large (~6-8 weeks)  
**Status**: Not implemented

**Features**:
- Pinyin → Chinese completion in text editors
- Works in VSCode, Vim, Emacs, etc.
- Real-time candidate suggestions
- Configurable via LSP settings

**Benefit**: Write Chinese in any editor without OS-level IME

---

## Summary Table

| Category | High 🔴 | Medium 🟡 | Low 🟢 | Polish 🔵 | Total |
|----------|---------|-----------|--------|-----------|-------|
| **Parser Schemes** | - | - | 2 | - | 2 |
| **Engine Features** | - | - | 3 | 1 | 4 |
| **Import/Export Tools** | - | 1 | 4 | - | 5 |
| **Code Quality** | - | 2 | 2 | 1 | 5 |
| **Documentation** | 1 | 1 | 1 | 1 | 4 |
| **Testing** | - | 1 | 2 | 1 | 4 |
| **Deployment** | 1 | 2 | 2 | - | 5 |
| **Total** | **2** | **7** | **16** | **4** | **29** |

---

## Recommended Next Steps

Choose one of these paths based on your goals:

### Path A: Deployment Focus 🚀
**Goal**: Get libchinese into users' hands

1. ✅ **Update outdated comments** (1 day)
2. ✅ **Platform packages** (2-3 weeks)
3. ✅ **User guide & tutorials** (2 weeks)
4. ✅ **Desktop integration** (4-8 weeks)

**Outcome**: Production deployment with good UX

---

### Path B: Feature Completeness 🎯
**Goal**: 100% upstream parity

1. ✅ **Additional zhuyin schemes** (2-3 weeks)
2. ✅ **Advanced engine flags** (2-3 weeks)
3. ✅ **Upstream test parity** (4-6 weeks)
4. ✅ **Fuzzy matching Phase 5** (3-4 weeks)

**Outcome**: Feature-complete with upstream C++ library

---

### Path C: Polish & Tooling 💎
**Goal**: Best-in-class developer experience

1. ✅ **Update outdated comments** (1 day)
2. ✅ **Import/export enhancements** (3-4 weeks)
3. ✅ **Configuration improvements** (2-3 weeks)
4. ✅ **Benchmark suite** (2 weeks)
5. ✅ **API documentation** (1-2 weeks)

**Outcome**: Production-ready with excellent tooling

---

## Effort Estimates

- **Small**: 1 week or less
- **Medium**: 2-4 weeks
- **Large**: 4+ weeks

**Total estimated effort**: ~60-90 weeks (1.2-1.7 years) for all items

**Realistic scope**: Pick 5-10 items per quarter based on priorities

---

## Contributing

If you'd like to contribute to any of these items:

1. Check [FINAL_STATUS_REPORT.md](FINAL_STATUS_REPORT.md) for current status
2. Open an issue to discuss approach
3. Reference this document in your PR
4. Update this file when items are completed

---

**Project Status**: Production-ready with 138 tests passing and ~94% upstream parity  
**Maintenance Mode**: All high/medium priority features complete  
**Open for Contributions**: Yes! Pick any item and contribute  

**Last Updated**: October 21, 2025

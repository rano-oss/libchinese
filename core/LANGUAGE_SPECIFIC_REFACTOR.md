# Language-Specific Code in Core: Analysis & Refactoring Options

## Problem Statement
`core` currently contains language-specific (pinyin/zhuyin) configuration fields that should belong in their respective crates.

## Language-Specific Code Found

### 1. Config Fields in `core/src/lib.rs`

**Pinyin-specific:**
```rust
pub correct_ue_ve: bool,      // nue ↔ nve
pub correct_v_u: bool,         // nv ↔ nu  
pub correct_uen_un: bool,      // juen ↔ jun
pub correct_gn_ng: bool,       // bagn ↔ bang
pub correct_mg_ng: bool,       // bamg ↔ bang
pub correct_iou_iu: bool,      // liou ↔ liu
pub double_pinyin_scheme: Option<String>,  // Microsoft, ZiRanMa, etc.
pub sort_by_pinyin_length: bool,
```

**Zhuyin-specific:**
```rust
pub zhuyin_incomplete: bool,
pub zhuyin_correct_shuffle: bool,
pub zhuyin_correct_hsu: bool,
pub zhuyin_correct_eten26: bool,
```

**Shared (truly generic):**
```rust
pub fuzzy: Vec<String>,       // Generic fuzzy rules
pub unigram_weight: f32,
pub bigram_weight: f32,
pub trigram_weight: f32,
pub allow_incomplete: bool,    // Generic parser option
pub sort_by_phrase_length: bool,
pub sort_without_longer_candidate: bool,
pub max_cache_size: usize,
```

### 2. Default Fuzzy Rules in `Config::default()`

**Pinyin-specific rules:**
```rust
"zh=z", "ch=c", "sh=s",       // Pinyin initials
"an=ang", "en=eng", "in=ing", // Pinyin finals
"l=n", "f=h", "k=g",          // Pinyin confusion
```

### 3. Engine/Model Usage
- **Current:** Both libpinyin and libzhuyin use `Config::default()` blindly
- **Issue:** libpinyin gets zhuyin flags, libzhuyin gets pinyin flags (ignored but pollutes API)

---

## Refactoring Alternatives

### **Option 1: Config Extension Pattern** ⭐ RECOMMENDED

**Approach:** Keep base Config in core, extend in language crates

```rust
// core/src/lib.rs
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    // Generic fields only
    pub fuzzy: Vec<String>,
    pub unigram_weight: f32,
    pub bigram_weight: f32,
    pub trigram_weight: f32,
    pub allow_incomplete: bool,
    pub sort_by_phrase_length: bool,
    pub sort_without_longer_candidate: bool,
    pub max_cache_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            fuzzy: vec![],  // Empty, let language crates fill
            unigram_weight: 0.6,
            bigram_weight: 0.3,
            trigram_weight: 0.1,
            allow_incomplete: true,
            sort_by_phrase_length: false,
            sort_without_longer_candidate: false,
            max_cache_size: 1000,
        }
    }
}

// libpinyin/src/config.rs (NEW FILE)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PinyinConfig {
    #[serde(flatten)]
    pub base: libchinese_core::Config,
    
    // Pinyin-specific
    pub correct_ue_ve: bool,
    pub correct_v_u: bool,
    pub correct_uen_un: bool,
    pub correct_gn_ng: bool,
    pub correct_mg_ng: bool,
    pub correct_iou_iu: bool,
    pub double_pinyin_scheme: Option<String>,
    pub sort_by_pinyin_length: bool,
}

impl Default for PinyinConfig {
    fn default() -> Self {
        let mut base = libchinese_core::Config::default();
        base.fuzzy = pinyin_default_fuzzy_rules();
        
        Self {
            base,
            correct_ue_ve: true,
            correct_v_u: true,
            correct_uen_un: true,
            correct_gn_ng: true,
            correct_mg_ng: true,
            correct_iou_iu: true,
            double_pinyin_scheme: None,
            sort_by_pinyin_length: false,
        }
    }
}

impl PinyinConfig {
    pub fn into_base(self) -> libchinese_core::Config {
        self.base
    }
}

fn pinyin_default_fuzzy_rules() -> Vec<String> {
    vec![
        "zh=z".into(), "z=zh".into(),
        "ch=c".into(), "c=ch".into(),
        "sh=s".into(), "s=sh".into(),
        "an=ang".into(), "ang=an".into(),
        "en=eng".into(), "eng=en".into(),
        "in=ing".into(), "ing=in".into(),
        "l=n".into(), "n=l".into(),
        "f=h".into(), "h=f".into(),
        "k=g".into(), "g=k".into(),
    ]
}

// libzhuyin/src/config.rs (NEW FILE)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ZhuyinConfig {
    #[serde(flatten)]
    pub base: libchinese_core::Config,
    
    // Zhuyin-specific
    pub zhuyin_incomplete: bool,
    pub zhuyin_correct_shuffle: bool,
    pub zhuyin_correct_hsu: bool,
    pub zhuyin_correct_eten26: bool,
}

impl Default for ZhuyinConfig {
    fn default() -> Self {
        let mut base = libchinese_core::Config::default();
        base.fuzzy = zhuyin_default_fuzzy_rules();
        
        Self {
            base,
            zhuyin_incomplete: true,
            zhuyin_correct_shuffle: true,
            zhuyin_correct_hsu: true,
            zhuyin_correct_eten26: true,
        }
    }
}

impl ZhuyinConfig {
    pub fn into_base(self) -> libchinese_core::Config {
        self.base
    }
}

fn zhuyin_default_fuzzy_rules() -> Vec<String> {
    vec![
        // Zhuyin rules (empty for now, or HSU/ETEN26 keyboard corrections)
    ]
}
```

**Pros:**
- ✅ Clean separation of concerns
- ✅ Core remains generic
- ✅ Language crates own their config
- ✅ Backward compatible (Model still takes Config)
- ✅ TOML deserialization works with `#[serde(flatten)]`

**Cons:**
- ⚠️ Requires new files (config.rs in each crate)
- ⚠️ Users must call `.into_base()` when creating Model

**Migration:**
```rust
// Before
let config = libchinese_core::Config::default();
let model = Model::new(lex, ngram, userdict, config, interp);

// After
let config = libpinyin::PinyinConfig::default();
let model = Model::new(lex, ngram, userdict, config.into_base(), interp);
```

---

### **Option 2: Generic Config with TypeState Pattern**

**Approach:** Use PhantomData to parameterize Config by language

```rust
// core/src/lib.rs
pub struct PinyinLang;
pub struct ZhuyinLang;

#[derive(Debug, Clone)]
pub struct Config<L = ()> {
    // Generic fields
    pub fuzzy: Vec<String>,
    pub unigram_weight: f32,
    // ...
    _lang: PhantomData<L>,
}

impl Config<PinyinLang> {
    pub fn correct_ue_ve(&self) -> bool { /* ... */ }
    // Pinyin methods
}

impl Config<ZhuyinLang> {
    pub fn zhuyin_incomplete(&self) -> bool { /* ... */ }
    // Zhuyin methods
}
```

**Pros:**
- ✅ Type safety at compile time
- ✅ Single Config type

**Cons:**
- ❌ Complex API
- ❌ Requires Model to be generic over language type
- ❌ Overkill for this use case

---

### **Option 3: Keep Everything, Add Language Enum**

**Approach:** Add a `language: Language` field to Config

```rust
pub enum Language {
    Pinyin,
    Zhuyin,
}

pub struct Config {
    pub language: Language,
    // All fields (pinyin + zhuyin)
    pub correct_ue_ve: bool,
    pub zhuyin_incomplete: bool,
    // ...
}
```

**Pros:**
- ✅ Minimal changes
- ✅ Single config type

**Cons:**
- ❌ Doesn't solve the problem (still language-specific in core)
- ❌ Polluted API (pinyin users see zhuyin fields)
- ❌ Runtime checks needed

---

### **Option 4: Trait-Based Config**

**Approach:** Define a trait for language-specific extensions

```rust
// core/src/lib.rs
pub trait LanguageConfig: Clone {
    fn base(&self) -> &Config;
    fn base_mut(&mut self) -> &mut Config;
}

pub struct Config {
    // Generic fields only
}

// libpinyin/src/config.rs
pub struct PinyinExtension {
    pub correct_ue_ve: bool,
    // ...
}

impl LanguageConfig for (Config, PinyinExtension) {
    fn base(&self) -> &Config { &self.0 }
    fn base_mut(&mut self) -> &mut Config { &mut self.0 }
}
```

**Pros:**
- ✅ Flexible extension
- ✅ Core stays generic

**Cons:**
- ❌ Overly complex for simple config needs
- ❌ Tuple types are awkward

---

## Recommendation: **Option 1 - Config Extension Pattern**

### Implementation Plan

1. **Create base Config in core** (remove language-specific fields)
2. **Create PinyinConfig in libpinyin/src/config.rs**
3. **Create ZhuyinConfig in libzhuyin/src/config.rs**
4. **Update Engine constructors** to accept language-specific config
5. **Add helper functions** for default fuzzy rules in each crate

### Migration Path

**Phase 1:** Add new config types (non-breaking)
- Create PinyinConfig/ZhuyinConfig
- Keep old Config for compatibility

**Phase 2:** Update examples and docs
- Show new pattern in examples
- Update README

**Phase 3:** (Optional) Deprecate old fields
- Mark language-specific fields with `#[deprecated]`
- Remove in next major version

---

## Code to Move

### From `core/src/lib.rs` → `libpinyin/src/config.rs`:
- `correct_ue_ve`, `correct_v_u`, `correct_uen_un`
- `correct_gn_ng`, `correct_mg_ng`, `correct_iou_iu`
- `double_pinyin_scheme`, `sort_by_pinyin_length`
- Pinyin fuzzy rules (zh=z, ch=c, sh=s, etc.)

### From `core/src/lib.rs` → `libzhuyin/src/config.rs`:
- `zhuyin_incomplete`, `zhuyin_correct_shuffle`
- `zhuyin_correct_hsu`, `zhuyin_correct_eten26`
- Zhuyin fuzzy rules (keyboard layout corrections)

### Keep in `core/src/lib.rs`:
- `fuzzy: Vec<String>` (generic, filled by language crates)
- `unigram_weight`, `bigram_weight`, `trigram_weight`
- `allow_incomplete` (generic parser option)
- `sort_by_phrase_length`, `sort_without_longer_candidate`
- `max_cache_size`

---

## Decision Required

Which option do you prefer?
- **Option 1**: Extension pattern (my recommendation)
- **Option 2**: TypeState pattern  
- **Option 3**: Keep everything with Language enum
- **Option 4**: Trait-based config
- **Other**: Suggest alternative?

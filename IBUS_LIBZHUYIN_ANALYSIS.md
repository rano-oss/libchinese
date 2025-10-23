# ibus-libzhuyin Feature Analysis

**Date**: 2025-10-23  
**Source**: https://github.com/libzhuyin/ibus-libzhuyin  
**Goal**: Identify features to port to libzhuyin to make it a complete IME

## Executive Summary

ibus-libzhuyin is the **upstream reference implementation** for Zhuyin/Bopomofo input on IBus. Like ibus-libpinyin, it's a **C++/GObject** implementation that provides:

1. **Complete IME architecture** (editors, modes, properties)
2. **Multiple keyboard layout support** (Standard, HSU, IBM, ETEN, etc.)
3. **Pinyin input mode** (ibus-libzhuyin supports BOTH Zhuyin and Pinyin!)
4. **Symbol/Emoji input** (easy symbols, user symbols, builtin symbols)
5. **UI property management** (Chinese/English mode, Full/Half width, Trad/Simp)
6. **Editor state machine** (Init, Raw, Fallback modes)

**Key Insight**: ibus-libzhuyin is architecturally **nearly identical** to ibus-libpinyin! Same editor hierarchy, same configuration system, same property management. The main difference is:
- **Parser**: ZhuyinParser vs PinyinParser
- **Keyboard layouts**: Standard/HSU/IBM/ETEN vs Pinyin schemes
- **Dual-mode support**: Can switch between Zhuyin and Pinyin input!

**Current libzhuyin Status**: ⚠️ **VERY BASIC** - Only has parser, config stub, and thin engine wrapper. Needs complete IME architecture like we built for libpinyin.

## Architecture Comparison

### ibus-libzhuyin (Upstream)

```
ZhuyinEngine (main engine)
├── ZhuyinProperties (mode properties)
├── EditorPtr[MODE_LAST] (editor array)
│   ├── MODE_INIT → ZhuyinEditor or PinyinEditor
│   └── MODE_RAW  → RawEditor
├── FallbackEditor (handles non-phonetic input)
│
Editor Hierarchy:
├── Editor (base class)
│   ├── RawEditor (pass-through mode)
│   ├── FallbackEditor (quotes, punctuation)
│   ├── EnhancedEditor (enhanced text format support)
│   │   └── PhoneticEditor (phonetic candidates + symbols)
│   │       ├── ZhuyinEditor (bopomofo input)
│   │       └── PinyinEditor (pinyin input mode)
│
Symbol System:
├── SymbolSection (base)
│   ├── BuiltinSymbolSection (predefined symbols)
│   ├── PhoneticSection (phonetic input during symbols)
│   ├── BopomofoSymbolSection (symbol+bopomofo mix)
│   ├── UserSymbolListAllSection (list all user symbols)
│   └── UserSymbolShownSection (show user symbols)
```

### Our libzhuyin (Current)

```
libzhuyin (crate)
├── lib.rs (exports + standard_fuzzy_rules stub)
├── config.rs (ZhuyinConfig stub)
├── parser.rs (ZhuyinParser - COMPLETE)
└── engine.rs (Engine thin wrapper)

Missing:
- ❌ IME engine architecture
- ❌ Editor system
- ❌ Mode management
- ❌ Property system
- ❌ Symbol input
- ❌ UI integration helpers
```

## Feature Comparison Table

| Feature | ibus-libzhuyin | Our libzhuyin | Priority | Notes |
|---------|----------------|---------------|----------|-------|
| **Core Engine** |
| Zhuyin parser | ✅ Full | ✅ Complete | ✅ DONE | parser.rs functional |
| Pinyin mode | ✅ Dual-mode | ❌ Not impl | 🟡 LOW | Can use libpinyin separately |
| Lexicon loading | ✅ libzhuyin C | ✅ Via core | ✅ DONE | Reuses core::Lexicon |
| N-gram model | ✅ libzhuyin C | ✅ Via core | ✅ DONE | Reuses core::NGramModel |
| User dictionary | ✅ libzhuyin C | ✅ Via core | ✅ DONE | Reuses core::UserDict |
| **Keyboard Layouts** |
| Standard Zhuyin | ✅ Yes | ⚠️ No config | 🔴 HIGH | Need keyboard scheme config |
| HSU layout | ✅ Yes | ⚠️ No config | 🔴 HIGH | Popular in Taiwan |
| IBM layout | ✅ Yes | ⚠️ No config | 🟡 MEDIUM | Less common |
| ETEN layout | ✅ Yes | ⚠️ No config | 🟡 MEDIUM | Less common |
| ETEN26 layout | ✅ Yes | ⚠️ No config | 🟡 MEDIUM | Extended ETEN |
| Custom layouts | ✅ Yes | ❌ No | 🟡 LOW | Via fuzzy rules? |
| **Fuzzy Matching** |
| Zhuyin fuzzy rules | ✅ Full | ⚠️ Stub only | 🔴 HIGH | c-ch, s-sh, etc. |
| Layout-specific fuzzy | ✅ Per-layout | ❌ No | 🔴 HIGH | HSU vs ETEN corrections |
| Tone fuzzy | ✅ Optional | ❌ No | 🟡 MEDIUM | Match ignoring tones |
| **Editor System** |
| PhoneticEditor | ✅ Yes | ❌ No | 🔴 HIGH | Main input editor |
| EnhancedEditor | ✅ Yes | ❌ No | 🟡 MEDIUM | Enhanced text format |
| FallbackEditor | ✅ Yes | ❌ No | 🟢 MEDIUM | Punctuation handling |
| RawEditor | ✅ Yes | ❌ No | 🟡 LOW | Pass-through mode |
| Mode switching | ✅ Yes | ❌ No | 🔴 HIGH | Init/Raw/Phonetic |
| **Symbol Input** |
| Easy symbols | ✅ Yes | ❌ No | 🟢 MEDIUM | Quick symbol access |
| User symbols | ✅ Yes | ❌ No | 🟢 MEDIUM | Custom symbol dict |
| Builtin symbols | ✅ Yes | ❌ No | 🟢 MEDIUM | Predefined symbols |
| Bopomofo+symbol mix | ✅ Yes | ❌ No | 🟡 LOW | Advanced feature |
| Symbol browsing | ✅ Yes | ❌ No | 🟡 LOW | List all symbols |
| **UI Properties** |
| Chinese/English mode | ✅ Toggle | ❌ No | 🔴 HIGH | Essential UI |
| Full/Half width | ✅ Toggle | ❌ No | 🔴 HIGH | Essential UI |
| Trad/Simp Chinese | ✅ Toggle | ❌ No | 🟢 MEDIUM | For display |
| Setup dialog | ✅ Yes | ❌ No | 🟡 LOW | GUI-specific |
| Property icons | ✅ SVG | ❌ No | 🟡 LOW | GUI-specific |
| **Configuration** |
| GSettings integration | ✅ Yes | ❌ No | 🟡 LOW | GNOME-specific |
| Config persistence | ✅ Yes | ⚠️ Stub | 🔴 HIGH | Need actual config |
| Orientation | ✅ H/V | ❌ No | 🟡 LOW | Candidate list layout |
| Page size | ✅ Yes | ⚠️ In core | ✅ DONE | Already in core::Config |
| Candidate keys | ✅ Custom | ❌ No | 🟢 MEDIUM | "1234567890" etc. |
| Init defaults | ✅ Yes | ⚠️ Stub | 🔴 HIGH | Chinese/Full/Trad defaults |
| **Training** |
| User bigram learning | ✅ zhuyin_train | ✅ Via core | ✅ DONE | core::Engine.learn_bigram |
| Modified flag | ✅ Yes | ❌ No | 🟡 MEDIUM | Track if save needed |
| Auto-save timer | ✅ Yes | ❌ No | 🟡 MEDIUM | Periodic persistence |
| Clear user data | ✅ Yes | ❌ No | 🟡 LOW | Reset learning |
| **Special Features** |
| Number always input | ✅ Config | ❌ No | 🟡 MEDIUM | Type numbers directly |
| Space show candidates | ✅ Config | ❌ No | 🟡 MEDIUM | Space key behavior |
| Candidates after cursor | ✅ Config | ❌ No | 🟡 LOW | UI positioning |
| Content type awareness | ✅ IBus 1.5.4+ | ❌ No | 🟡 LOW | Password fields, etc. |

**Legend**:  
- ✅ = Fully implemented  
- ⚠️ = Partially implemented  
- ❌ = Not implemented  
- 🔴 HIGH = Critical for basic IME  
- 🟢 MEDIUM = Important for full-featured IME  
- 🟡 LOW = Nice-to-have or GUI-specific  

## Detailed Feature Analysis

### 1. Keyboard Layout System (HIGH PRIORITY)

**Upstream Implementation**:
```cpp
// ZYZConfig.cc - Keyboard layout mapping
static const struct {
    gint layout;
    ZhuyinScheme scheme;
} zhuyin_schemes [] = {
    {0, ZHUYIN_STANDARD},      // Standard layout
    {1, ZHUYIN_HSU},           // HSU (許氏) layout
    {2, ZHUYIN_IBM},           // IBM layout
    {3, ZHUYIN_GINYIEH},       // GinYieh layout
    {4, ZHUYIN_ETEN},          // ETEN (倚天) layout
    {5, ZHUYIN_ETEN26},        // ETEN26 layout
    {6, ZHUYIN_DACHEN_CP26},   // DaChen CP26 layout
    {7, ZHUYIN_STANDARD_DVORAK},// Dvorak variant
};

// Fuzzy corrections per layout
static const struct {
    const gchar *name;
    guint option;
} fuzzy_zhuyin_options [] = {
    { "fuzzy-zhuyin-c-ch",       ZHUYIN_AMB_C_CH      },
    { "fuzzy-zhuyin-z-zh",       ZHUYIN_AMB_Z_ZH      },
    { "fuzzy-zhuyin-s-sh",       ZHUYIN_AMB_S_SH      },
    { "fuzzy-zhuyin-l-n",        ZHUYIN_AMB_L_N       },
    { "fuzzy-zhuyin-f-h",        ZHUYIN_AMB_F_H       },
    { "fuzzy-zhuyin-l-r",        ZHUYIN_AMB_L_R       },
    { "fuzzy-zhuyin-g-k",        ZHUYIN_AMB_G_K       },
    { "fuzzy-zhuyin-an-ang",     ZHUYIN_AMB_AN_ANG    },
    { "fuzzy-zhuyin-en-eng",     ZHUYIN_AMB_EN_ENG    },
    { "fuzzy-zhuyin-in-ing",     ZHUYIN_AMB_IN_ING    },
};
```

**What We Need**:
1. **ZhuyinScheme enum** in config.rs:
   ```rust
   pub enum ZhuyinScheme {
       Standard,      // Default bopomofo layout
       Hsu,           // HSU (許氏) - most popular alternative
       Ibm,           // IBM layout
       Eten,          // ETEN (倚天) layout
       Eten26,        // Extended ETEN
       DaChenCp26,    // DaChen CP26
       // ... etc
   }
   ```

2. **Layout-to-keymap conversion**:
   - Map QWERTY keys → Bopomofo symbols based on scheme
   - E.g., HSU: 'd' → 'ㄉ', 't' → 'ㄊ', but different from Standard

3. **Per-layout fuzzy rules**:
   - HSU has specific key confusions (ㄓ/ㄐ on same key)
   - ETEN has different confusions
   - Need penalty weights per rule type

**Priority**: 🔴 **CRITICAL** - Without keyboard layouts, users can't actually type!

---

### 2. IME Editor Architecture (HIGH PRIORITY)

**Upstream Structure**:
```cpp
// ZYZZhuyinEngine.cc - Editor array
class ZhuyinEngine : public Engine {
    enum {
        MODE_INIT = 0,    // Main phonetic input mode
        MODE_RAW,         // Raw pass-through mode
        MODE_LAST,
    } m_input_mode;

    EditorPtr m_editors[MODE_LAST];      // Mode-specific editors
    EditorPtr m_fallback_editor;         // Handles non-phonetic keys
};

// Editor hierarchy
class PhoneticEditor : public EnhancedEditor {
    // Handles phonetic input, candidates, symbols
    bool processSpace();      // Commit or show candidates
    bool processEscape();     // Cancel input
    bool processEasySymbol(); // Quick symbol input
    bool processUserSymbol(); // User-defined symbols
    bool insert();            // Insert phonetic char
    void commit();            // Commit to app
    void reset();             // Clear state
    void updateCandidates();  // Refresh lookup table
};
```

**What We Need** (port from our libpinyin work!):

1. **ImeEngine struct** (main coordinator):
   ```rust
   pub struct ImeEngine {
       phonetic_editor: PhoneticEditor,
       punct_editor: PunctuationEditor,
       suggestion_editor: SuggestionEditor,
       session: ImeSession,
       context: ImeContext,
   }
   ```

2. **PhoneticEditor** (zhuyin input):
   ```rust
   pub struct PhoneticEditor {
       backend: Engine,
       input_buffer: String,    // Raw bopomofo input
       parsed: Vec<String>,     // Parsed syllables
       candidates: Vec<Candidate>,
       cursor: usize,
   }
   ```

3. **Mode management**:
   - **InputMode::Phonetic** - Main zhuyin input
   - **InputMode::Suggestion** - Post-commit predictions (reuse from libpinyin!)
   - **InputMode::Raw** - Direct character pass-through

4. **Key routing**:
   ```rust
   fn process_key(&mut self, key: KeyEvent) -> KeyResult {
       match self.session.mode() {
           InputMode::Phonetic => self.phonetic_editor.process_key(key),
           InputMode::Suggestion => self.suggestion_editor.process_key(key),
           InputMode::Raw => KeyResult::NotHandled,
       }
   }
   ```

**Priority**: 🔴 **CRITICAL** - This is the core of the IME!

**Good News**: 🎉 We already built most of this for libpinyin! Can reuse:
- ✅ ImeEngine architecture
- ✅ EditorResult pattern (Commit, CommitAndReset, etc.)
- ✅ SuggestionEditor (auto-prediction)
- ✅ KeyEvent/KeyResult enums
- ✅ Session/Context management

---

### 3. Property System (HIGH PRIORITY)

**Upstream Implementation**:
```cpp
// ZYZhuyinProperties.cc - UI properties
class ZhuyinProperties {
    bool m_mode_chinese;      // Chinese vs English
    bool m_mode_full_width;   // Full vs Half width
    bool m_mode_trad;         // Traditional vs Simplified

    Property m_prop_chinese;  // Toggle property
    Property m_prop_full_width;
    Property m_prop_trad;
    
    void toggleModeChinese();
    void toggleModeFullWidth();
    void toggleModeTrad();
};
```

**What We Need**:

1. **Property struct** (for GUI integration):
   ```rust
   pub struct Property {
       pub key: String,        // "mode.chinese"
       pub label: String,      // "中文"
       pub icon: Option<String>, // Path to icon
       pub state: bool,        // Current state
   }
   
   pub struct PropertiesPanel {
       pub chinese_mode: Property,
       pub full_width: Property,
       pub trad_chinese: Property,
   }
   ```

2. **Mode toggles**:
   ```rust
   impl ImeEngine {
       pub fn toggle_chinese_mode(&mut self) {
           self.properties.chinese_mode = !self.properties.chinese_mode;
           if !self.properties.chinese_mode {
               // Switch to English mode - pass keys through
               self.session.set_mode(InputMode::Raw);
           }
       }
   }
   ```

3. **Property activation** (for GUI callbacks):
   ```rust
   pub fn activate_property(&mut self, key: &str) -> bool {
       match key {
           "mode.chinese" => { self.toggle_chinese_mode(); true }
           "mode.full_width" => { self.toggle_full_width(); true }
           "mode.trad" => { self.toggle_trad_chinese(); true }
           _ => false
       }
   }
   ```

**Priority**: 🔴 **CRITICAL** - Users need to toggle Chinese/English!

---

### 4. Symbol Input System (MEDIUM PRIORITY)

**Upstream Implementation**:
```cpp
// ZYZPhoneticEditor.cc - Symbol handling
bool PhoneticEditor::processEasySymbolKey(guint keyval, ...) {
    if (!m_config.easySymbol())
        return false;
    
    // Quick access to common symbols
    // E.g., shift+number → symbol
    String symbol = lookupEasySymbol(keyval, modifiers);
    if (!symbol.empty()) {
        commit(symbol);
        return true;
    }
}

bool PhoneticEditor::processUserSymbolKey(guint keyval, ...) {
    // User-defined symbol mappings
    // Load from user symbol table
}
```

**What We Need**:

1. **Symbol tables**:
   ```rust
   pub struct SymbolTable {
       easy_symbols: HashMap<char, Vec<String>>,  // '1' → ["！", "⼀", ...]
       user_symbols: HashMap<String, Vec<String>>, // Custom mappings
   }
   ```

2. **Symbol mode**:
   ```rust
   pub enum InputMode {
       Phonetic,
       Suggestion,
       Symbol,   // ← New: browsing symbols
       Raw,
   }
   ```

3. **Symbol candidates**:
   ```rust
   fn build_symbol_candidates(&self, key: char) -> Vec<Candidate> {
       self.symbol_table.easy_symbols.get(&key)
           .map(|symbols| {
               symbols.iter().map(|s| Candidate {
                   text: s.clone(),
                   score: 1.0,
               }).collect()
           })
           .unwrap_or_default()
   }
   ```

**Priority**: 🟢 **MEDIUM** - Nice feature but not critical for basic input

**Note**: Could reuse punct.table infrastructure from pinyin!

---

### 5. Dual Pinyin/Zhuyin Mode (LOW PRIORITY)

**Upstream Feature**: ibus-libzhuyin can switch between Zhuyin and Pinyin input!

```cpp
// ZYZZhuyinEngine.cc - Mode switching
void ZhuyinEngine::focusIn() {
    Config *config = &ZhuyinConfig::instance();
    bool is_zhuyin = config->isZhuyin();
    
    if (is_zhuyin) {
        m_editors[MODE_INIT].reset(
            new ZhuyinEditor(m_props, ZhuyinConfig::instance()));
    } else {
        m_editors[MODE_INIT].reset(
            new PinyinEditor(m_props, ZhuyinConfig::instance()));
    }
}
```

**What We Need**:
- Conditional editor selection based on config
- Ability to hot-swap parsers

**Priority**: 🟡 **LOW** - Users can just use libpinyin separately. No need to bundle both in one binary.

**Recommendation**: **Don't implement**. Keep libzhuyin focused on Zhuyin only.

---

## Priority Roadmap

### Phase 1: Minimum Viable IME (Critical Features)

**Goal**: Make libzhuyin usable for basic Zhuyin input

1. **Keyboard Layout System** 🔴
   - Implement ZhuyinScheme enum (Standard, HSU at minimum)
   - Add keymap conversion (QWERTY → Bopomofo per scheme)
   - Port fuzzy rules from parser (already partially done!)
   - **Files**: `config.rs`, `lib.rs` (update standard_fuzzy_rules)
   - **Estimate**: 2-3 hours

2. **Port ImeEngine Architecture from libpinyin** 🔴
   - Copy ImeEngine, PhoneticEditor, EditorResult pattern
   - Replace PinyinParser with ZhuyinParser
   - Remove pinyin-specific fuzzy logic
   - **Files**: Create `ime_engine.rs`, `editor/phonetic.rs`, `editor/mod.rs`
   - **Estimate**: 4-5 hours (most code already exists!)

3. **Property System** 🔴
   - Add Property struct
   - Add PropertiesPanel
   - Implement Chinese/English toggle
   - Implement Full/Half width toggle
   - **Files**: Create `properties.rs`
   - **Estimate**: 2 hours

4. **Configuration Integration** 🔴
   - Flesh out ZhuyinConfig (currently just a stub)
   - Add keyboard_scheme, fuzzy_options, init_defaults
   - Load/save to file (or just in-memory for now)
   - **Files**: Update `config.rs`
   - **Estimate**: 2 hours

**Total Phase 1**: ~10-12 hours (1-2 days)

---

### Phase 2: Enhanced Features (Medium Priority)

**Goal**: Make libzhuyin feature-complete

5. **Symbol Input System** 🟢
   - Port punct.table loading from pinyin
   - Add easy_symbols.table support
   - Implement SymbolEditor
   - Add symbol browsing mode
   - **Files**: Create `editor/symbol.rs`, add data files
   - **Estimate**: 3-4 hours

6. **Auto-Suggestion Mode** 🟢
   - Port SuggestionEditor from libpinyin (already done!)
   - Just wire it into ImeEngine
   - **Files**: Copy from libpinyin, integrate
   - **Estimate**: 1 hour (already implemented!)

7. **Advanced Configuration** 🟢
   - Add orientation (H/V)
   - Add candidate keys customization
   - Add page size (already in core::Config)
   - Add Traditional/Simplified toggle
   - **Files**: Update `config.rs`
   - **Estimate**: 2 hours

8. **Training Enhancements** 🟢
   - Add modified flag tracking
   - Add auto-save timer (optional)
   - Add clear user data helper
   - **Files**: Update `engine.rs`
   - **Estimate**: 2 hours

**Total Phase 2**: ~8-9 hours (1 day)

---

### Phase 3: Polish & Optimization (Low Priority)

9. **Additional Keyboard Layouts** 🟡
   - Add IBM, ETEN, ETEN26, DaChen layouts
   - Per-layout fuzzy rules
   - **Estimate**: 3-4 hours

10. **Advanced Symbol Features** 🟡
    - User symbol tables
    - Symbol browsing UI
    - Bopomofo+symbol mixing
    - **Estimate**: 4-5 hours

11. **Content Type Awareness** 🟡
    - Detect password fields
    - Disable learning in sensitive contexts
    - **Estimate**: 1-2 hours

**Total Phase 3**: ~8-11 hours (1 day)

---

## Implementation Strategy

### Reuse from libpinyin (80% overlap!)

Since ibus-libzhuyin and ibus-libpinyin share **identical architecture**, we can reuse almost everything:

**Direct Ports** (copy & adapt):
- ✅ `ime_engine.rs` → Change parser type
- ✅ `editor/phonetic.rs` → Change parser type
- ✅ `editor/punctuation.rs` → Works as-is
- ✅ `editor/suggestion.rs` → Works as-is
- ✅ `session.rs` → Works as-is
- ✅ `context.rs` → Works as-is
- ✅ `key_event.rs` → Works as-is

**Zhuyin-Specific** (new code):
- 🆕 Keyboard layout system (keymaps)
- 🆕 Layout-specific fuzzy rules
- 🆕 Bopomofo display formatting

**Estimate**: 70-80% of code can be **copied directly** from libpinyin!

---

## File Structure Plan

```
libzhuyin/
├── src/
│   ├── lib.rs                    # Public API + re-exports
│   ├── config.rs                 # ZhuyinConfig with keyboard schemes
│   ├── parser.rs                 # ✅ DONE - ZhuyinParser
│   ├── engine.rs                 # ✅ DONE - Engine wrapper
│   ├── ime_engine.rs             # 🆕 Main IME coordinator (port from libpinyin)
│   ├── properties.rs             # 🆕 UI property system
│   ├── keyboard.rs               # 🆕 Keyboard layout mappings
│   ├── session.rs                # 🆕 Session state (copy from libpinyin)
│   ├── context.rs                # 🆕 UI context (copy from libpinyin)
│   ├── key_event.rs              # 🆕 Key events (copy from libpinyin)
│   └── editor/
│       ├── mod.rs                # Editor exports
│       ├── phonetic.rs           # 🆕 PhoneticEditor (port from libpinyin)
│       ├── punctuation.rs        # 🆕 PunctuationEditor (copy from libpinyin)
│       ├── suggestion.rs         # 🆕 SuggestionEditor (copy from libpinyin)
│       └── symbol.rs             # 🆕 SymbolEditor (optional, later)
├── examples/
│   ├── demo.rs                   # Basic usage demo
│   └── ime_demo.rs               # 🆕 Full IME demo
└── tests/
    ├── integration_tests.rs
    └── keyboard_layout_tests.rs  # 🆕 Test keyboard mappings
```

---

## Testing Strategy

### Unit Tests

1. **Keyboard Layout Tests**:
   ```rust
   #[test]
   fn test_hsu_layout_mapping() {
       let keymap = HsuKeymap::new();
       assert_eq!(keymap.map('d'), Some("ㄉ"));
       assert_eq!(keymap.map('t'), Some("ㄊ"));
       assert_eq!(keymap.map('j'), Some("ㄓ"));
   }
   ```

2. **Fuzzy Rule Tests**:
   ```rust
   #[test]
   fn test_c_ch_fuzzy_matching() {
       let engine = Engine::new_with_fuzzy(vec!["ㄘ=ㄔ:1.5"]);
       let candidates = engine.lookup("ㄘ");
       // Should include both ㄘ and ㄔ candidates
   }
   ```

3. **Editor State Tests**:
   ```rust
   #[test]
   fn test_phonetic_editor_workflow() {
       let mut editor = PhoneticEditor::new(engine);
       editor.process_key(KeyEvent::char('d'));  // ㄉ
       editor.process_key(KeyEvent::char('a'));  // ㄚ
       assert_eq!(editor.input_buffer(), "ㄉㄚ");
       let candidates = editor.candidates();
       assert!(candidates.iter().any(|c| c.text == "大"));
   }
   ```

### Integration Tests

1. **End-to-End IME Flow**:
   ```rust
   #[test]
   fn test_ime_end_to_end() {
       let mut ime = ImeEngine::new(engine);
       
       // Type "ㄋㄧˇ ㄏㄠˇ" (ni hao)
       ime.process_key(KeyEvent::char('s'));  // ㄋ (HSU layout)
       ime.process_key(KeyEvent::char('u'));  // ㄧ
       ime.process_key(KeyEvent::char('3'));  // Tone 3
       ime.process_key(KeyEvent::Space);       // Commit
       
       assert_eq!(ime.context.commit_text, "你");
       
       // Auto-suggestion should trigger
       assert_eq!(ime.session.mode(), InputMode::Suggestion);
       let candidates = ime.get_candidates();
       assert!(candidates.iter().any(|c| c.text == "好"));
   }
   ```

2. **Mode Switching Test**:
   ```rust
   #[test]
   fn test_chinese_english_toggle() {
       let mut ime = ImeEngine::new(engine);
       
       // Start in Chinese mode
       assert!(ime.properties.chinese_mode);
       
       // Type English
       ime.toggle_chinese_mode();
       assert!(!ime.properties.chinese_mode);
       
       let result = ime.process_key(KeyEvent::char('a'));
       assert_eq!(result, KeyResult::NotHandled);  // Pass-through
   }
   ```

---

## Recommendations

### What to Implement (Priority Order)

1. **Phase 1 - Critical** (Do first):
   - ✅ Keyboard layouts (Standard + HSU minimum)
   - ✅ IME engine architecture (port from libpinyin)
   - ✅ Property system (Chinese/English, Full/Half)
   - ✅ Configuration (keyboard scheme, fuzzy options)

2. **Phase 2 - Important** (Do next):
   - ✅ Symbol input (reuse punct.table)
   - ✅ Auto-suggestion (port from libpinyin - already done!)
   - ✅ Traditional/Simplified toggle
   - ✅ Training enhancements

3. **Phase 3 - Optional** (Do later):
   - Additional keyboard layouts (IBM, ETEN, etc.)
   - Advanced symbol features
   - Content type awareness

### What NOT to Implement

1. **Dual Pinyin/Zhuyin Mode** ❌
   - **Reason**: Users can use libpinyin separately
   - **Complexity**: Not worth maintaining two parsers in one binary

2. **GSettings Integration** ❌
   - **Reason**: GNOME-specific, not portable
   - **Alternative**: Simple file-based config or in-memory

3. **Setup GUI Dialog** ❌
   - **Reason**: GUI framework-specific
   - **Alternative**: Let GUI framework handle config UI

4. **IBus-Specific Features** ❌
   - **Reason**: We're building a framework-agnostic IME
   - **Alternative**: Provide clean API for GUI integration

---

## Success Criteria

After implementation, libzhuyin should be able to:

1. ✅ Accept QWERTY keyboard input and map to Bopomofo (HSU layout minimum)
2. ✅ Parse Bopomofo into syllables
3. ✅ Look up candidates from lexicon
4. ✅ Display candidates with scores
5. ✅ Commit selected candidate
6. ✅ Learn user preferences (bigrams)
7. ✅ Auto-suggest next word after commit
8. ✅ Toggle Chinese/English mode
9. ✅ Toggle Full/Half width mode
10. ✅ Handle punctuation marks
11. ✅ Support symbol input (basic)

**Definition of "Complete IME"**: Can be integrated into any GUI framework (GTK, Qt, Windows, macOS) with minimal glue code. Just call `ime_engine.process_key()` and render the returned `ImeContext`.

---

## Next Steps

1. **Review this analysis** ✅
2. **Choose Phase 1 priorities** ← YOU ARE HERE
3. **Start with keyboard layouts** (easiest first!)
4. **Port ImeEngine from libpinyin** (bulk of work)
5. **Add property system** (UI integration)
6. **Test end-to-end** (HSU layout input → commit)
7. **Move to Phase 2** (symbols, auto-suggest)

---

## Conclusion

**Key Insights**:
- ibus-libzhuyin is **architecturally identical** to ibus-libpinyin
- 70-80% of libpinyin code can be **directly reused**
- Main differences: keyboard layouts, parser type, bopomofo display
- **Estimated total effort**: 20-30 hours (2-4 days) for complete IME

**Biggest Win**: We already implemented auto-suggestion, user learning, and full IME architecture for libpinyin! Just need to:
1. Add keyboard layout system
2. Port ImeEngine (change parser type)
3. Add property system
4. Done! ✨

**Recommendation**: Start with Phase 1 (keyboard + IME engine port). This gets us to a **functional Zhuyin IME** quickly. Then iterate on polish.

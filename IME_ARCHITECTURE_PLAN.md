# NEXT-libpinyin: Standalone IME Architecture Plan

## Executive Summary

Building a complete, framework-agnostic Chinese IME based on analysis of ibus-libpinyin. This will be a new crate that extracts the core IME logic from ibus-libpinyin and implements it in Rust, making it injectable into any GUI/IME framework (IBus, Fcitx, Windows TSF, macOS, Wayland, etc.).

## Key Insights from ibus-libpinyin Analysis

### Core Architecture Components

1. **Engine Layer** - Top-level state machine managing input modes
2. **Editor Layer** - Handles key events, manages input buffer, generates candidates
3. **Session State** - Tracks cursor, input buffer, selected text, mode switches
4. **Candidate Management** - Lookup table, paging, selection, enhanced candidates
5. **Feature Modules** - Punctuation, English mode, extensions, suggestions

### Critical Separation of Concerns

```
┌─────────────────────────────────────────────────────────────┐
│                    GUI/Framework Layer                       │
│         (IBus, Fcitx, TSF, macOS, Wayland, etc.)           │
└──────────────────────┬──────────────────────────────────────┘
                       │ Simple trait-based API
┌──────────────────────▼──────────────────────────────────────┐
│                  Standalone IME Core                         │
│  ┌────────────┐  ┌─────────────┐  ┌──────────────────┐     │
│  │   Engine   │  │   Session   │  │  Candidate Mgr   │     │
│  │  (Modes)   │→ │   State     │→ │  (Paging/Select) │     │
│  └────────────┘  └─────────────┘  └──────────────────┘     │
│         │                │                    │              │
│         └────────────────┼────────────────────┘              │
│                          ▼                                   │
│              ┌───────────────────────┐                       │
│              │  libchinese Backend   │                       │
│              │  (Model, Parser, etc) │                       │
│              └───────────────────────┘                       │
└─────────────────────────────────────────────────────────────┘
```

## Proposed Crate Structure

**Goal: Merge NEXT-libpinyin features into existing libpinyin crate**

```
libpinyin/  (enhanced, not separate crate)
├── Cargo.toml
├── src/
│   ├── lib.rs                  # Public API exports
│   ├── engine.rs               # Enhanced IME engine (existing + session)
│   ├── session.rs              # NEW: Input session state
│   ├── editor/                 # NEW: Editor implementations
│   │   ├── mod.rs
│   │   ├── phonetic.rs         # Wraps existing Engine logic
│   │   ├── punct.rs            # Punctuation editor
│   │   └── suggestion.rs       # Post-commit suggestions
│   ├── candidates/             # NEW: Candidate management
│   │   ├── mod.rs
│   │   ├── lookup_table.rs     # Paging, cursor, selection
│   │   └── types.rs            # Candidate types
│   ├── input_buffer.rs         # NEW: Buffer management
│   ├── composition.rs          # NEW: Preedit composition
│   ├── context.rs              # NEW: Input context (focus, content type)
│   ├── parser.rs               # EXISTING: Parser
│   ├── fuzzy.rs                # EXISTING: Fuzzy matching
│   └── config.rs               # EXISTING: Configuration
├── examples/
│   ├── cli_ime.rs              # NEW: Full IME demo for Wayland
│   └── interactive.rs          # EXISTING: Simple interactive demo
└── tests/
    ├── engine_tests.rs         # EXISTING
    ├── session_tests.rs        # NEW
    └── ime_integration.rs      # NEW: Full IME flow tests
```

## Core Components Design

### 1. Engine - Top-Level State Machine

**Responsibilities:**
- Manage input modes (Init, Phonetic, Punctuation, English, Suggestion)
- Route key events to appropriate editor
- Handle mode transitions
- Coordinate with GUI framework

**API Design:**
```rust
pub struct ImeEngine {
    mode: InputMode,
    session: Session,
    editors: EditorRegistry,
    config: EngineConfig,
}

pub enum InputMode {
    Init,           // No active input
    Phonetic,       // Pinyin/Zhuyin input
    Punctuation,    // Full-width punct selection
    English,        // English completion (optional)
    Suggestion,     // Post-commit suggestions
}

impl ImeEngine {
    pub fn new(backend: Backend, config: EngineConfig) -> Self;
    
    // Key event processing
    pub fn process_key(&mut self, event: KeyEvent) -> KeyResult;
    
    // State queries
    pub fn preedit(&self) -> &Composition;
    pub fn candidates(&self) -> &CandidateList;
    pub fn auxiliary_text(&self) -> Option<&str>;
    
    // Lifecycle
    pub fn focus_in(&mut self);
    pub fn focus_out(&mut self);
    pub fn reset(&mut self);
    
    // Candidate interaction
    pub fn page_up(&mut self);
    pub fn page_down(&mut self);
    pub fn select_candidate(&mut self, index: usize) -> Option<CommitText>;
}

pub enum KeyResult {
    Consumed,                      // Key handled, no commit
    Commit(String),                // Text to commit
    PassThrough,                   // Let app handle key
    ModeChange(InputMode),         // Mode switch occurred
}
```

### 2. Session - Input State Management

**Responsibilities:**
- Track input buffer and cursor position
- Maintain preedit composition (pinyin + converted text)
- Store candidate list state
- Manage mode-specific state

**API Design:**
```rust
pub struct Session {
    buffer: InputBuffer,
    composition: Composition,
    candidates: CandidateList,
    mode_state: ModeState,
}

pub struct InputBuffer {
    text: String,           // Raw input (e.g., "nihao")
    cursor: usize,          // Cursor position in buffer
}

pub struct Composition {
    preedit: String,        // Display text (e.g., "你好")
    segments: Vec<Segment>, // Segment boundaries
    cursor: usize,          // Visual cursor position
}

pub struct Segment {
    range: Range<usize>,    // Byte range in preedit
    source: Range<usize>,   // Byte range in input buffer
    confirmed: bool,        // User has moved past this
}

impl Session {
    pub fn new() -> Self;
    pub fn clear(&mut self);
    pub fn update_from_backend(&mut self, results: BackendResults);
}
```

### 3. Editor - Input Processing Logic

**Trait Design:**
```rust
pub trait Editor {
    /// Process a key event in this editor's context
    fn process_key(&mut self, key: KeyEvent, session: &mut Session) -> EditorResult;
    
    /// Update candidates based on current input
    fn update_candidates(&mut self, session: &mut Session);
    
    /// Reset editor state
    fn reset(&mut self);
    
    /// Select a candidate by index
    fn select_candidate(&mut self, index: usize, session: &mut Session) -> Option<String>;
}

pub enum EditorResult {
    Handled,                        // Key consumed, session updated
    Commit(String),                 // Text to commit, stay in mode
    CommitAndReset(String),         // Text to commit, exit mode
    ModeSwitch(InputMode),          // Switch to different mode
    PassThrough,                    // Let parent handle
}

pub struct PhoneticEditor {
    backend: Arc<libpinyin::Engine>, // Or libzhuyin::Engine
    fuzzy_config: FuzzyConfig,
}

pub struct PunctuationEditor {
    punct_map: HashMap<char, Vec<String>>,
    active_key: Option<char>,
}

pub struct SuggestionEditor {
    backend: Arc<libpinyin::Engine>,
    previous_text: String,
}
```

### 4. Candidate Management

**Design:**
```rust
pub struct CandidateList {
    items: Vec<Candidate>,
    page_size: usize,
    cursor: usize,
}

pub struct Candidate {
    text: String,
    label: Option<String>,    // "1", "2", etc.
    comment: Option<String>,  // Auxiliary info
    candidate_type: CandidateType,
}

pub enum CandidateType {
    Normal,
    Predicted,      // From suggestion mode
    Punctuation,
    English,
    LuaExtension,   // Future
}

impl CandidateList {
    pub fn page_count(&self) -> usize;
    pub fn current_page(&self) -> &[Candidate];
    pub fn page_up(&mut self) -> bool;
    pub fn page_down(&mut self) -> bool;
    pub fn cursor_up(&mut self) -> bool;
    pub fn cursor_down(&mut self) -> bool;
    pub fn select(&mut self, index: usize) -> Option<&Candidate>;
}
```

### 5. IME Context (Simple Struct)

**Goal:** Simple struct with public fields that platform code fills in

```rust
/// IME context for displaying preedit, candidates, and committing text
/// 
/// Platform-specific code (Wayland, IBus, CLI) updates these fields,
/// then ImeSession reads them to interact with the platform.
pub struct ImeContext {
    // Output from IME to platform
    pub preedit_text: String,
    pub preedit_cursor: usize,
    pub commit_text: String,
    pub candidates: Vec<String>,
    pub auxiliary_text: String,
    
    // Input from platform to IME
    pub input_purpose: InputPurpose,
    pub is_focused: bool,
}

impl ImeContext {
    pub fn new() -> Self {
        Self {
            preedit_text: String::new(),
            preedit_cursor: 0,
            commit_text: String::new(),
            candidates: Vec::new(),
            auxiliary_text: String::new(),
            input_purpose: InputPurpose::FreeForm,
            is_focused: false,
        }
    }
    
    pub fn clear(&mut self) {
        self.preedit_text.clear();
        self.preedit_cursor = 0;
        self.commit_text.clear();
        self.candidates.clear();
    }
    
    /// Take the commit text (leaving it empty)
    pub fn take_commit(&mut self) -> String {
        std::mem::take(&mut self.commit_text)
    }
}

pub enum InputPurpose {
    FreeForm,
    Email,
    Password,
    Url,
    Number,
}

/// IME session combining engine + context
pub struct ImeSession {
    engine: ImeEngine,
    pub context: ImeContext,
}

impl ImeSession {
    pub fn new(backend: Backend) -> Self {
        Self {
            engine: ImeEngine::new(backend, EngineConfig::default()),
            context: ImeContext::new(),
        }
    }
    
    pub fn process_key(&mut self, event: KeyEvent) {
        // Clear previous updates
        self.context.commit_text.clear();
        
        match self.engine.process_key(event) {
            KeyResult::Consumed => {
                // Update context with latest state
                let preedit = self.engine.preedit();
                self.context.preedit_text = preedit.preedit.clone();
                self.context.preedit_cursor = preedit.cursor;
                
                let candidates = self.engine.candidates();
                self.context.candidates = candidates.current_page()
                    .iter()
                    .map(|c| c.text.clone())
                    .collect();
            }
            KeyResult::Commit(text) => {
                self.context.commit_text = text;
                self.context.clear();
                self.engine.reset();
            }
            KeyResult::PassThrough => {}
            KeyResult::ModeChange(_) => {
                // Update auxiliary text if mode changed
            }
        }
    }
    
    pub fn focus_in(&mut self) {
        self.context.is_focused = true;
        self.engine.focus_in();
    }
    
    pub fn focus_out(&mut self) {
        self.context.is_focused = false;
        self.engine.focus_out();
    }
    
    pub fn reset(&mut self) {
        self.engine.reset();
        self.context.clear();
    }
    
    /// Get access to the engine
    pub fn engine(&self) -> &ImeEngine {
        &self.engine
    }
    
    /// Get mutable access to the engine
    pub fn engine_mut(&mut self) -> &mut ImeEngine {
        &mut self.engine
    }
}
```

## Feature Modules from ibus-libpinyin

### Essential Features (Phase 1)

**What we currently have in libpinyin:**
- ✅ **Phonetic Input** - Parser with fuzzy matching
- ✅ **N-gram Scoring** - NGramModel with interpolation
- ✅ **User Dictionary** - UserDict with learn/frequency
- ✅ **Fuzzy Matching** - Comprehensive syllable-level rules
- ✅ **Candidate Generation** - Model::candidates_for_key()
- ✅ **Basic Engine** - Engine::input() with segmentation

**What's MISSING for full IME (Phase 1 additions):**
- ❌ **Session State Management** - No input buffer, cursor tracking, or composition
- ❌ **Candidate Paging** - No page_up/down, cursor navigation in candidate list
- ❌ **Mode Management** - No Init/Phonetic/Punct/Suggestion mode switching
- ❌ **Preedit Display** - No formatted composition with segments
- ❌ **Key Event Routing** - No structured key event handling
- ❌ **Commit Logic** - No proper commit/reset flow

**Phase 1 Goal:** Add session management layer on top of existing Engine

### Advanced Features (Phase 2)

**What's MISSING in libpinyin for Phase 2:**
- ❌ **Punctuation Mode** - Full-width punctuation selection (need punct map + editor)
- ❌ **Suggestion Mode** - Post-commit predictions (need suggestion editor)
- ❌ **Auxiliary Text** - Mode indicators, help text (need display logic)
- ❌ **Multi-Segment Editing** - Cursor movement within composition, segment re-conversion
- ❌ **Smart Commit** - Auto-commit on certain keys (period, numbers, etc.)
- ❌ **Emoji Support** - Emoji candidates from keywords

**Phase 2 Goal:** Add specialized editors and enhanced candidate types

### Data-Driven Features (Phase 3)

7. **Traditional Chinese** - ✅ Already supported through lexicon data
   - Load Traditional Chinese lexicon instead of/alongside Simplified
   - NGramModel handles both character sets
   - No conversion logic needed - pure data-driven

8. **Emoji Support** - Load emoji lookup table as lexicon entries
   - Map keywords (like ":smile:") to emoji in lexicon
   - Use same candidate generation pipeline

**Phase 3 Goal:** Leverage existing data infrastructure for new input types

## Integration Examples

### Wayland Standalone IME (Primary Use Case)

```rust
// In wayland-libpinyin binary crate
use libpinyin::{ImeSession, Backend, KeyEvent};
use wayland_client::protocol::zwp_input_method_v2::ZwpInputMethodV2;

fn main() {
    let backend = Backend::from_data_dir("./data").unwrap();
    let mut session = ImeSession::new(backend);
    
    // Wayland input method handle
    let input_method: ZwpInputMethodV2 = connect_input_method_v2();
    
    // Main event loop
    loop {
        match wayland_event.read() {
            WaylandEvent::Key(key_event) => {
                // Process the key
                session.process_key(KeyEvent::from_wayland(key_event));
                
                // Read updated context and send to Wayland
                if !session.context.commit_text.is_empty() {
                    let text = session.context.take_commit();
                    input_method.commit_string(text);
                    input_method.commit(/* serial */);
                }
                
                if !session.context.preedit_text.is_empty() {
                    input_method.set_preedit_string(
                        session.context.preedit_text.clone(),
                        session.context.preedit_cursor as i32,
                        session.context.preedit_cursor as i32,
                    );
                } else {
                    input_method.set_preedit_string(String::new(), 0, 0);
                }
                
                // Update candidate popup
                if !session.context.candidates.is_empty() {
                    show_candidate_popup(&session.context.candidates);
                } else {
                    hide_candidate_popup();
                }
            }
            WaylandEvent::FocusIn => {
                session.focus_in();
            }
            WaylandEvent::FocusOut => {
                session.focus_out();
            }
            WaylandEvent::Reset => {
                session.reset();
            }
            WaylandEvent::ContentType(purpose, _hints) => {
                session.context.input_purpose = convert_wayland_purpose(purpose);
            }
        }
    }
}
```

### CLI Demo (Testing/Development)

```rust
// In examples/cli_ime.rs
use libpinyin::{ImeSession, Backend, KeyEvent};

fn main() {
    let backend = Backend::from_data_dir("./data").unwrap();
    let mut session = ImeSession::new(backend);
    
    println!("CLI IME Demo - Type to test!");
    
    loop {
        // Read key from terminal
        let key = read_key();
        
        // Process key
        session.process_key(key);
        
        // Display updates
        if !session.context.commit_text.is_empty() {
            let text = session.context.take_commit();
            print!("{}", text);
            std::io::stdout().flush().unwrap();
        }
        
        // Show preedit
        if !session.context.preedit_text.is_empty() {
            print!("\r\x1b[K{}", session.context.preedit_text);
            
            // Show candidates
            if !session.context.candidates.is_empty() {
                print!(" | ");
                for (i, cand) in session.context.candidates.iter().enumerate() {
                    print!("{}:{} ", i + 1, cand);
                }
            }
            std::io::stdout().flush().unwrap();
        } else {
            // Clear line
            print!("\r\x1b[K");
            std::io::stdout().flush().unwrap();
        }
    }
}
```

### Alternative: IBus Integration Example

```rust
// Shows how easy it is to adapt to other frameworks
use libpinyin::{ImeSession, Backend, KeyEvent};
use ibus::Engine as IBusEngine;

fn process_ibus_key(session: &mut ImeSession, ibus_engine: &IBusEngine, key_event: KeyEvent) {
    session.process_key(key_event);
    
    // Read context and update IBus
    if !session.context.commit_text.is_empty() {
        let text = session.context.take_commit();
        ibus_engine.commit_text(&ibus::Text::new(&text));
    }
    
    let preedit = ibus::Text::new(&session.context.preedit_text);
    ibus_engine.update_preedit_text(&preedit, session.context.preedit_cursor as u32, true);
    
    // Update lookup table with candidates
    let lookup_table = create_ibus_lookup_table(&session.context.candidates);
    ibus_engine.update_lookup_table(&lookup_table, true);
}
```

## State Transition Examples

### Scenario 1: Basic Pinyin Input

```
User types: n i h a o [space]

States:
1. Init mode, empty buffer
2. Key 'n' → Phonetic mode, buffer="n", preedit="n", candidates=[你,呢,...]
3. Key 'i' → buffer="ni", preedit="你", candidates=[你,尼,逆,...]
4. Key 'h' → buffer="nih", preedit="你好", candidates=[...]
5. Key 'a' → buffer="niha", preedit="你哈", candidates=[...]
6. Key 'o' → buffer="nihao", preedit="你好", candidates=[你好,尼好,...]
7. Key [space] → commit "你好", return to Init mode
```

### Scenario 2: Punctuation Mode

```
User types: [comma]

States:
1. Phonetic mode with text → Detect comma → Switch to Punct mode
2. Show candidates: [，,,,、,…] (full-width variants)
3. User selects or types again → Commit punctuation, return to previous mode
```

### Scenario 3: Suggestion Mode

```
User commits "你好"

States:
1. Post-commit → Check config for suggestion_enabled
2. If enabled → Switch to Suggestion mode
3. Load predicted next words: [吗,！,啊,...]
4. User selects → Commit, stay in Suggestion
5. User types new char → Exit Suggestion, enter Phonetic
```

## Migration Path - Enhancing Existing libpinyin

### Current State (What We Have)
```rust
// libpinyin/src/engine.rs
pub struct Engine {
    model: Model,  // Owns the backend
}

impl Engine {
    pub fn input(&self, text: &str) -> Vec<Candidate> {
        // 1. Parse input with fuzzy matching
        // 2. Segment into syllables
        // 3. Generate candidates
        // 4. Return top candidates
    }
}

// libpinyin/src/lib.rs
pub use engine::Engine;
pub use parser::{Parser, Segmentation};
pub use fuzzy::FuzzyMap;
```

**Limitations:**
- No state persistence (every call to `input()` is independent)
- No cursor tracking or editing
- No paging through candidates
- No mode switching
- No preedit composition
- Returns candidates but no display formatting

### Phase 1: Add Session Management Layer

**Goal:** Keep existing API, add new session-based API alongside

```rust
// NEW: libpinyin/src/session.rs
pub struct ImeEngine {
    backend: Engine,  // Wraps existing Engine
    session: Session,
    mode: InputMode,
}

impl ImeEngine {
    // New session-based API
    pub fn process_key(&mut self, event: KeyEvent) -> KeyResult { ... }
    pub fn preedit(&self) -> &Composition { ... }
    pub fn candidates(&self) -> &CandidateList { ... }
    
    // Existing Engine methods still available via Deref
    pub fn segment(&self, input: &str) -> Vec<Segmentation> {
        self.backend.segment(input)
    }
}

// OLD API still works
let engine = Engine::from_data_dir("./data").unwrap();
let candidates = engine.input("nihao");  // ✅ Still works

// NEW API for full IME
let mut ime = ImeEngine::new(engine);
ime.process_key(KeyEvent::Char('n'));
ime.process_key(KeyEvent::Char('i'));
println!("Preedit: {}", ime.preedit().preedit);  // "你"
```

**Changes Required:**
1. Add `session.rs` - Session state management
2. Add `input_buffer.rs` - Raw input tracking
3. Add `composition.rs` - Preedit display formatting
4. Add `candidates/lookup_table.rs` - Paging, cursor navigation
5. Enhance `engine.rs` - Wrap existing Engine, add mode management
6. No changes to `parser.rs`, `fuzzy.rs`, `config.rs` ✅

### Phase 2: Add Specialized Editors

```rust
// NEW: libpinyin/src/editor/mod.rs
pub trait Editor {
    fn process_key(&mut self, key: KeyEvent, session: &mut Session) -> EditorResult;
    fn update_candidates(&mut self, session: &mut Session);
}

// libpinyin/src/editor/phonetic.rs - Wraps existing Engine
pub struct PhoneticEditor {
    backend: Engine,  // Uses existing Engine::segment + Model::candidates_for_key
}

// libpinyin/src/editor/punct.rs - New functionality
pub struct PunctuationEditor {
    punct_map: HashMap<char, Vec<String>>,
}

// libpinyin/src/editor/suggestion.rs - New functionality
pub struct SuggestionEditor {
    backend: Engine,  // Uses existing n-gram for predictions
}
```

**Changes Required:**
1. Add `editor/` module with trait + implementations
2. Add `editor/punct.rs` with punctuation map
3. Add `editor/suggestion.rs` using existing n-gram backend
4. Update `ImeEngine` to route to appropriate editor based on mode

## Testing Strategy

### Unit Tests
- Each editor in isolation
- Candidate list paging/selection
- Input buffer manipulation
- Composition building

### Integration Tests
- Complete input scenarios (as shown above)
- Mode transitions
- Multi-character input sequences
- Error recovery

### Framework Tests
- Mock ImeContext for framework-independent testing
- Verify correct API calls
- Test commit/preedit/candidate updates

## Dependencies

```toml
[dependencies]
libchinese-core = { path = "../core" }
libpinyin = { path = "../libpinyin" }
libzhuyin = { path = "../libzhuyin" }

# For punctuation maps, emoji data
phf = "0.11"
unicode-segmentation = "1.10"

# For serialization of session state (optional)
serde = { version = "1.0", features = ["derive"], optional = true }

[dev-dependencies]
criterion = "0.5"    # Benchmarking
mockall = "0.12"     # Mocking for tests
```

## Roadmap

### Phase 1: Session Management Layer (1-2 weeks)
**Goal:** Add IME session capabilities to existing libpinyin without breaking changes

**Current State Analysis:**
- ✅ Have: Parser, FuzzyMap, Engine::input(), Model, NGramModel, UserDict
- ❌ Missing: Input buffer, cursor, preedit composition, candidate paging, mode switching

**Tasks:**
- [ ] Design `Session` struct to track input state
- [ ] Implement `InputBuffer` with cursor management
- [ ] Implement `Composition` for preedit display formatting
- [ ] Implement `CandidateList` with paging/cursor navigation
- [ ] Create `ImeEngine` wrapping existing `Engine`
- [ ] Add `InputMode` enum and state machine
- [ ] Implement `KeyEvent` processing in phonetic mode
- [ ] CLI demo (`examples/cli_ime.rs`) working end-to-end
- [ ] Integration tests for session state

**Success Criteria:**
- Old `Engine::input()` API still works unchanged ✅
- New `ImeEngine` can handle character-by-character input
- Preedit updates as user types
- Can page through candidates with page up/down
- Can commit text and reset session

### Phase 2: Specialized Editors (1-2 weeks)
**Goal:** Add punctuation and suggestion modes

**Missing Components:**
- ❌ Punctuation editor with full-width punct map
- ❌ Suggestion editor for post-commit predictions
- ❌ Editor trait for pluggable input modes
- ❌ Auxiliary text display for mode indicators

**Tasks:**
- [ ] Define `Editor` trait
- [ ] Refactor phonetic input into `PhoneticEditor`
- [ ] Implement `PunctuationEditor` with punct map
- [ ] Implement `SuggestionEditor` using n-gram predictions
- [ ] Add mode switching logic (comma → punct, post-commit → suggestion)
- [ ] Add auxiliary text support
- [ ] Update CLI demo with all modes
- [ ] Tests for each editor

**Success Criteria:**
- Typing comma in pinyin mode switches to punctuation selection
- After committing text, suggestions appear automatically (if enabled)
- Can switch between modes seamlessly
- Mode indicator shown in auxiliary text

### Phase 3: Data-Driven Extensions (1 week)
**Goal:** Leverage data infrastructure for Traditional Chinese and Emoji

**Approach:**
- ✅ Traditional Chinese: Already works via lexicon data (no code changes)
- ✅ Emoji: Add emoji lexicon mapping keywords to emoji

**Tasks:**
- [ ] Document how to load Traditional Chinese lexicons
- [ ] Create emoji lookup table format
- [ ] Add emoji data loading example
- [ ] Update CLI demo to showcase both
- [ ] Performance testing with larger lexicons

**Success Criteria:**
- Can switch between Simplified/Traditional by loading different lexicon
- Can type emoji by keywords (e.g., "smile" → 😊)
- No performance degradation with emoji data loaded

## Success Criteria

1. **Backward Compatibility**: Existing `Engine::input()` API continues to work unchanged
2. **Completeness**: Can replicate 90% of ibus-libpinyin core IME functionality
3. **Performance**: <5ms key event processing, <50ms candidate generation
4. **Usability**: Simple, direct struct-based API for Wayland integration
5. **Testability**: >80% code coverage, comprehensive integration tests
6. **Data-Driven**: Traditional Chinese and Emoji via lexicon data, not code

## Design Decision: Simple Struct with Public Fields

**Question:** Why use a simple struct with public fields instead of callbacks or traits?

**Answer:** **Maximum Simplicity - Just Data**

The `ImeContext` is a **data transfer object** - it's just a container for state that flows between the IME engine and the platform.

**Design:**
```rust
pub struct ImeContext {
    // IME writes here, platform reads
    pub preedit_text: String,
    pub commit_text: String,
    pub candidates: Vec<String>,
    
    // Platform writes here, IME reads
    pub input_purpose: InputPurpose,
}
```

**Advantages:**
1. **Dead simple** - It's just a struct with fields
2. **Zero abstraction** - No functions, no callbacks, no traits
3. **Crystal clear** - You can see exactly what data flows where
4. **Easy to inspect** - Just look at the fields in debugger
5. **Trivial to test** - Create struct, check fields, done
6. **Zero overhead** - No function calls, just direct field access
7. **Obvious usage** - Call `process_key()`, read the fields

**Usage Pattern:**
```rust
// 1. Create session
let mut session = ImeSession::new(backend);

// 2. Process input
session.process_key(KeyEvent::Char('n'));

// 3. Read the fields - that's it!
if !session.context.commit_text.is_empty() {
    platform.commit(&session.context.commit_text);
}
if !session.context.preedit_text.is_empty() {
    platform.show_preedit(&session.context.preedit_text);
}
```

**Why not callbacks/traits?**
- ❌ Callbacks add indirection and complexity
- ❌ Traits add generics and trait bounds
- ❌ Both require more code for the same result
- ✅ Direct field access is simpler and clearer

**Platform Integration:**
Each platform (Wayland, IBus, CLI) just:
1. Calls `session.process_key(event)`
2. Reads `session.context.*` fields
3. Updates its own display/protocol

No callbacks, no traits, no complexity. Just data in, data out.

**Result:** The simplest possible API - a struct with fields that you read and write. Can't get simpler than that!

## Conclusion

This architecture provides:
- **Enhanced libpinyin**: Session management built on top of existing crate
- **Backward compatible**: Old API continues to work unchanged
- **Clear separation**: Session layer doesn't change core Model/Parser/NGram
- **Wayland-focused**: Direct struct-based API, no trait abstraction overhead
- **Data-driven features**: Traditional Chinese & Emoji via lexicons
- **Comprehensive parity**: Replicates ibus-libpinyin core functionality
- **Excellent testability**: Framework-independent session logic

The result will be a production-ready Wayland IME that enhances libpinyin without breaking existing functionality, built incrementally over 3-4 weeks.

## Next Steps

1. **Review this plan** - Confirm approach and priorities
2. **Phase 1 Kickoff** - Start with Session/InputBuffer/Composition structs
3. **CLI Demo First** - Build working demo before Wayland integration
4. **Incremental Testing** - Test each component as it's built
5. **Wayland Integration** - Once CLI demo works, add Wayland protocol layer

# Phase 4: Keyboard Shortcuts & Cloud Input

## Overview

Phase 4 adds essential keyboard shortcuts and cloud input integration to complete the core IME functionality.

## Features

### 1. Keyboard Shortcuts ✅

**Mode Switching:**
- `Ctrl+period`: Toggle punctuation mode (commit current composition first)
- `Shift_lock`: Enable/disable passthrough mode (new)

**Navigation:**
- `Arrow keys` (Left/Right): Move cursor in preedit *(already supported)*
- `Home/End`: Jump to preedit start/end *(already supported)*
- `Page Up/Down`: Navigate candidate pages *(already supported)*
- `Tab`: Next candidate page *(already supported)*

**Selection:**
- `1-9`: Select candidate by number (numrow/numpad same) *(already supported)*
- `Space`: Commit first candidate *(already supported)*
- `Enter`: Commit first candidate *(already supported)*

**Editing:**
- `Backspace`: Delete previous character in preedit *(already supported)*
- `Delete`: Delete next character *(already supported)*
- `Escape`: Cancel composition and clear buffer *(already supported)*

### 2. Passthrough Mode ✅

**What it is:**
- New `InputMode::Passthrough` state
- When enabled, IME does not process any keys (passes through to application)
- Useful for temporary typing without IME conversion
- Toggle with `Shift_lock` keyboard shortcut

**Implementation:**
- Add `Passthrough` variant to `InputMode` enum
- Track toggle state in `ImeEngine`
- Return `KeyResult::NotHandled` for all keys except `Shift_lock` toggle

### 3. Cloud Input ✅

**What it is:**
- Online prediction service for rare phrases
- Queries cloud API (Baidu/Google) when local dictionary has no good matches
- Returns as additional candidate source mixed with local results

**Architecture:**
- New `libpinyin/src/cloud.rs` module
- Uses `reqwest` for HTTP requests (async)
- Provides `CloudCandidates` struct with methods:
  - `query(pinyin: &str) -> Result<Vec<CloudCandidate>>`
  - `is_enabled()` → bool (config toggle)
  - `set_provider(provider: CloudProvider)` → Self

**Integration:**
- `PhoneticEditor` calls `CloudCandidates::query()` after local lookup
- Mix cloud results into candidate list (with indicator)
- Config option: `enable_cloud_input` (default: false)
- Fallback on network errors (silent failure)

**API Providers:**
- Baidu Input API (primary)
- Google Input Tools API (fallback)
- Custom endpoint support

### 4. User Phrase Management API ✅

**What it is:**
- Programmatic interface for GUI to view/edit user dictionary
- No text file import/export (database-only operations)
- CRUD operations on learned phrases

**New Methods in `UserDict`:**

```rust
impl UserDict {
    /// List all phrases in user dictionary
    pub fn list_all(&self) -> Result<Vec<(String, u64)>>;
    
    /// Add a phrase manually (for GUI)
    pub fn add_phrase(&mut self, phrase: &str, frequency: u64) -> Result<()>;
    
    /// Delete a phrase from dictionary
    pub fn delete_phrase(&mut self, phrase: &str) -> Result<()>;
    
    /// Update phrase frequency
    pub fn update_frequency(&mut self, phrase: &str, new_freq: u64) -> Result<()>;
    
    /// Search phrases by prefix (for GUI filtering)
    pub fn search_by_prefix(&self, prefix: &str) -> Result<Vec<(String, u64)>>;
}
```

**GUI Use Cases:**
- List view: Show all learned phrases with frequencies
- Add dialog: Manually add custom phrase with initial frequency
- Delete button: Remove unwanted learned phrase
- Edit dialog: Adjust frequency of existing phrase
- Search box: Filter phrases by text prefix

## Implementation Steps

### Step 1: Add KeyEvent Variants

Update `libpinyin/src/ime_engine.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyEvent {
    // Existing variants...
    Char(char),
    Backspace,
    Delete,
    // ... etc ...
    
    // NEW: Modifier + key combos
    /// Ctrl + character (e.g., Ctrl+period = Ctrl('.'))
    Ctrl(char),
    /// Shift lock toggle (for passthrough mode)
    ShiftLock,
}
```

### Step 2: Add Passthrough Mode

Update `libpinyin/src/session.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Init,
    Phonetic,
    Punctuation,
    Suggestion,
    Passthrough,  // NEW
}
```

Update `libpinyin/src/ime_engine.rs`:

```rust
impl ImeEngine {
    pub fn process_key(&mut self, key: KeyEvent) -> KeyResult {
        // Handle global shortcuts first
        match key {
            KeyEvent::ShiftLock => {
                // Toggle passthrough mode
                if self.session.mode() == InputMode::Passthrough {
                    self.session.set_mode(InputMode::Init);
                    self.reset();
                } else {
                    self.session.set_mode(InputMode::Passthrough);
                }
                return KeyResult::Handled;
            }
            KeyEvent::Ctrl('.') => {
                // Toggle punctuation mode (commit first if in phonetic)
                if self.session.mode() == InputMode::Phonetic {
                    // Commit current composition
                    if let Some(text) = self.get_preedit() {
                        self.context.commit_text = text;
                    }
                    self.reset();
                }
                // Toggle: if in punctuation, go to init; else go to punctuation
                if self.session.mode() == InputMode::Punctuation {
                    self.reset();
                } else {
                    self.session.set_mode(InputMode::Punctuation);
                }
                return KeyResult::Handled;
            }
            _ => {}
        }
        
        // Passthrough mode: ignore all other keys
        if self.session.mode() == InputMode::Passthrough {
            return KeyResult::NotHandled;
        }
        
        // Existing mode routing...
        // ...
    }
}
```

### Step 3: Cloud Input Module

Create `libpinyin/src/cloud.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudProvider {
    Baidu,
    Google,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudCandidate {
    pub text: String,
    pub confidence: f32,
}

pub struct CloudInput {
    provider: CloudProvider,
    enabled: bool,
    client: reqwest::Client,
    timeout_ms: u64,
}

impl CloudInput {
    pub fn new(provider: CloudProvider) -> Self {
        Self {
            provider,
            enabled: false,
            client: reqwest::Client::new(),
            timeout_ms: 3000,
        }
    }
    
    pub fn enable(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    pub async fn query(&self, pinyin: &str) -> Result<Vec<CloudCandidate>, Box<dyn std::error::Error>> {
        if !self.enabled {
            return Ok(vec![]);
        }
        
        match self.provider {
            CloudProvider::Baidu => self.query_baidu(pinyin).await,
            CloudProvider::Google => self.query_google(pinyin).await,
            CloudProvider::Custom(ref url) => self.query_custom(url, pinyin).await,
        }
    }
    
    async fn query_baidu(&self, pinyin: &str) -> Result<Vec<CloudCandidate>, Box<dyn std::error::Error>> {
        // Baidu Input API endpoint
        let url = format!("https://olime.baidu.com/py?input={}&inputtype=py&bg=0&ed=20&result=hanzi&resultcoding=utf-8&ch_en=0&clientinfo=web&version=1", pinyin);
        
        let response = self.client
            .get(&url)
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .send()
            .await?;
            
        let text = response.text().await?;
        
        // Parse Baidu response (JSON array of arrays)
        // Example: [["你好","ni'hao"],["拟好","ni'hao"]]
        let candidates: Vec<Vec<String>> = serde_json::from_str(&text)?;
        
        Ok(candidates.into_iter()
            .filter_map(|c| c.get(0).cloned())
            .map(|text| CloudCandidate { text, confidence: 0.8 })
            .collect())
    }
    
    async fn query_google(&self, pinyin: &str) -> Result<Vec<CloudCandidate>, Box<dyn std::error::Error>> {
        // Google Input Tools API
        let url = "https://inputtools.google.com/request?text={}&itc=zh-t-i0-pinyin&num=13&cp=0&cs=1&ie=utf-8&oe=utf-8";
        
        // TODO: Implement Google API parsing
        Ok(vec![])
    }
    
    async fn query_custom(&self, url: &str, pinyin: &str) -> Result<Vec<CloudCandidate>, Box<dyn std::error::Error>> {
        // Custom endpoint expecting JSON: {"query": "pinyin"}
        // Returns: [{"text": "你好", "confidence": 0.95}]
        let response = self.client
            .post(url)
            .json(&serde_json::json!({"query": pinyin}))
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .send()
            .await?;
            
        Ok(response.json().await?)
    }
}
```

### Step 4: User Phrase Management

Update `core/src/userdict.rs`:

```rust
impl UserDict {
    /// List all phrases in user dictionary with their frequencies.
    pub fn list_all(&self) -> Result<Vec<(String, u64)>, Box<dyn std::error::Error>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(USERDICT_TABLE)?;
        
        let mut phrases = Vec::new();
        for item in table.iter()? {
            let (key, value) = item?;
            let phrase = String::from_utf8(key.value().to_vec())?;
            let freq = value.value();
            phrases.push((phrase, freq));
        }
        
        Ok(phrases)
    }
    
    /// Add a phrase manually with specified frequency.
    pub fn add_phrase(&mut self, phrase: &str, frequency: u64) -> Result<(), Box<dyn std::error::Error>> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(USERDICT_TABLE)?;
            table.insert(phrase.as_bytes(), frequency)?;
        }
        txn.commit()?;
        
        self.cache.put(phrase.to_string(), frequency);
        Ok(())
    }
    
    /// Delete a phrase from the user dictionary.
    pub fn delete_phrase(&mut self, phrase: &str) -> Result<(), Box<dyn std::error::Error>> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(USERDICT_TABLE)?;
            table.remove(phrase.as_bytes())?;
        }
        txn.commit()?;
        
        self.cache.pop(phrase);
        Ok(())
    }
    
    /// Update the frequency of an existing phrase.
    pub fn update_frequency(&mut self, phrase: &str, new_freq: u64) -> Result<(), Box<dyn std::error::Error>> {
        // Just overwrite with new frequency
        self.add_phrase(phrase, new_freq)
    }
    
    /// Search phrases by prefix (for GUI filtering).
    pub fn search_by_prefix(&self, prefix: &str) -> Result<Vec<(String, u64)>, Box<dyn std::error::Error>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(USERDICT_TABLE)?;
        
        let mut phrases = Vec::new();
        for item in table.iter()? {
            let (key, value) = item?;
            let phrase = String::from_utf8(key.value().to_vec())?;
            if phrase.starts_with(prefix) {
                let freq = value.value();
                phrases.push((phrase, freq));
            }
        }
        
        Ok(phrases)
    }
}
```

## Dependencies

Add to `libpinyin/Cargo.toml`:

```toml
[dependencies]
# Existing dependencies...
libchinese-core = { path = "../core" }
# ...

# NEW: Cloud input HTTP client
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
serde_json = "1.0"
```

## Testing

### Keyboard Shortcuts Tests

```rust
#[test]
fn test_shift_lock_toggles_passthrough() {
    let mut ime = ImeEngine::new(Engine::new(...));
    
    assert_eq!(ime.session().mode(), InputMode::Init);
    
    // Enable passthrough
    ime.process_key(KeyEvent::ShiftLock);
    assert_eq!(ime.session().mode(), InputMode::Passthrough);
    
    // Keys should pass through
    assert_eq!(ime.process_key(KeyEvent::Char('a')), KeyResult::NotHandled);
    
    // Disable passthrough
    ime.process_key(KeyEvent::ShiftLock);
    assert_eq!(ime.session().mode(), InputMode::Init);
}

#[test]
fn test_ctrl_period_toggles_punctuation() {
    let mut ime = ImeEngine::new(Engine::new(...));
    
    // Toggle to punctuation mode
    ime.process_key(KeyEvent::Ctrl('.'));
    assert_eq!(ime.session().mode(), InputMode::Punctuation);
    
    // Toggle back to init
    ime.process_key(KeyEvent::Ctrl('.'));
    assert_eq!(ime.session().mode(), InputMode::Init);
}
```

### Cloud Input Tests

```rust
#[tokio::test]
async fn test_cloud_input_baidu() {
    let mut cloud = CloudInput::new(CloudProvider::Baidu);
    cloud.enable(true);
    
    let results = cloud.query("nihao").await.unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].text, "你好");
}

#[test]
fn test_cloud_input_disabled_returns_empty() {
    let cloud = CloudInput::new(CloudProvider::Baidu);
    assert!(!cloud.is_enabled());
    
    // When disabled, should return empty immediately (no network call)
    // ...
}
```

### User Phrase Management Tests

```rust
#[test]
fn test_add_and_list_phrases() {
    let temp_path = std::env::temp_dir().join("test_userdict.redb");
    let mut dict = UserDict::new(&temp_path).unwrap();
    
    dict.add_phrase("测试", 100).unwrap();
    dict.add_phrase("示例", 50).unwrap();
    
    let all = dict.list_all().unwrap();
    assert_eq!(all.len(), 2);
    assert!(all.contains(&("测试".to_string(), 100)));
}

#[test]
fn test_delete_phrase() {
    let temp_path = std::env::temp_dir().join("test_userdict2.redb");
    let mut dict = UserDict::new(&temp_path).unwrap();
    
    dict.add_phrase("删除我", 10).unwrap();
    dict.delete_phrase("删除我").unwrap();
    
    assert_eq!(dict.frequency("删除我"), 0);
}

#[test]
fn test_search_by_prefix() {
    let temp_path = std::env::temp_dir().join("test_userdict3.redb");
    let mut dict = UserDict::new(&temp_path).unwrap();
    
    dict.add_phrase("你好", 100).unwrap();
    dict.add_phrase("你好吗", 50).unwrap();
    dict.add_phrase("我好", 30).unwrap();
    
    let results = dict.search_by_prefix("你").unwrap();
    assert_eq!(results.len(), 2);
}
```

## Documentation

- Update `README.md` with keyboard shortcuts table
- Add `CLOUD_INPUT.md` explaining API integration
- Update `USER_DICT_API.md` with GUI integration examples

## Success Criteria

- ✅ All keyboard shortcuts working correctly
- ✅ Passthrough mode toggles properly
- ✅ Cloud input returns results (with network fallback)
- ✅ User phrase management CRUD operations work
- ✅ All tests passing (95+ total tests)
- ✅ Documentation updated

## Notes

**Clarifications from user:**
- ❌ English mode: NOT NEEDED (users switch keyboard/IME)
- ❌ Table mode: NOT NEEDED (UserDict handles custom phrases)
- ❌ Lua extensions: NOT NEEDED
- ✅ Cloud input: YES - Using `reqwest` 
- ✅ User phrase management: YES - GUI-only (no text file import/export)
- ✅ Numpad keys: Treat same as number row (no distinction)
- ❌ Auxiliary select keys (F1-F9): NOT NEEDED
- ❌ Ctrl+Shift+F (Simp/Trad toggle): NOT NEEDED (handled by data files)

**Table Mode Clarification:**
Table mode functionality is essentially what `UserDict` provides - the ability to store and retrieve custom phrase mappings with frequencies. The only difference is that table mode in ibus-libpinyin allows loading external `.table` files, which we're not implementing. Our GUI will provide CRUD operations directly on the user dictionary database.

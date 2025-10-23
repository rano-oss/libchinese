# Emoji Support

## Overview

**libpinyin supports emoji input** through its data-driven lexicon architecture. Emojis are treated as regular characters and mapped to keywords (both English and Pinyin) in the lexicon.

## How It Works

Emoji support leverages the same phonetic lookup system used for Chinese characters:

```
User types: "smile"
   ↓
Engine looks up "smile" in lexicon
   ↓
Returns: 😊 (emoji as a candidate)
   ↓
User selects and commits emoji
```

The architecture is **completely character-agnostic** - emojis are just Unicode characters mapped to phonetic keywords.

## Data File Format

### Emoji Lexicon (emoji.table)

```
<keyword> <emoji> <id> <frequency>
```

**Example:**
```
smile   😊   100001  10000
heart   ❤️   100002  12000
xiao    😊   100003  8000
xin     ❤️   100004  9000
```

### Features

1. **Multiple keywords per emoji**
   - English: `smile` → 😊
   - Pinyin: `xiao` → 😊
   
2. **Multiple emojis per keyword**
   - `heart` → ❤️, 💕, 💖, 💗
   
3. **Frequency-based ranking**
   - Higher frequency = appears first in candidates
   
4. **Mixed with Chinese**
   - Emoji candidates appear alongside Chinese characters

## Usage

### Method 1: Load Emoji Alongside Pinyin

```rust
use libpinyin::Engine;
use libchinese_core::{Model, Lexicon, NGramModel, UserDict, Config};

// Load main Chinese lexicon
let chinese_lex = Lexicon::from_file("data/gb_char.table")?;

// Load emoji lexicon
let emoji_lex = Lexicon::from_file("data/emoji.table")?;

// Merge lexicons
let mut combined = chinese_lex;
combined.merge(emoji_lex);

// Create model with combined lexicon
let ngram = NGramModel::load("data/interpolation2.text")?;
let userdict = UserDict::new("data/user.redb")?;
let model = Model::new(combined, ngram, userdict, Config::default());
let engine = Engine::new(model);

// Now emoji candidates appear in results
let candidates = engine.input("xiao");
// Returns: 笑, 小, 😊, 销, ...
```

### Method 2: Emoji-Only Mode

```rust
// Load only emoji lexicon
let emoji_lex = Lexicon::from_file("data/emoji.table")?;
let model = Model::new(emoji_lex, NGramModel::new(), UserDict::new("data/user.redb")?, Config::default());
let engine = Engine::new(model);

// Only emoji candidates
let candidates = engine.input("smile");
// Returns: 😊, 😄, 😁, ...
```

### Method 3: Toggle Mode in IME

```rust
pub struct EmojiToggle {
    chinese_engine: Engine,
    emoji_engine: Engine,
    combined_engine: Engine,
    mode: EmojiMode,
}

enum EmojiMode {
    Chinese,      // No emoji
    Emoji,        // Only emoji
    Mixed,        // Both Chinese and emoji
}

impl EmojiToggle {
    pub fn process_input(&self, text: &str) -> Vec<Candidate> {
        match self.mode {
            EmojiMode::Chinese => self.chinese_engine.input(text),
            EmojiMode::Emoji => self.emoji_engine.input(text),
            EmojiMode::Mixed => self.combined_engine.input(text),
        }
    }
    
    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            EmojiMode::Chinese => EmojiMode::Mixed,
            EmojiMode::Mixed => EmojiMode::Emoji,
            EmojiMode::Emoji => EmojiMode::Chinese,
        };
    }
}
```

## Keyword Design

### English Keywords

Use common emoji names from Unicode CLDR:
- `smile`, `laugh`, `cry`, `heart`, `love`
- `thumbsup`, `ok`, `fire`, `star`
- `dog`, `cat`, `pizza`, `coffee`

### Pinyin Keywords

Map to Chinese emotion/concept words:
- `xiao` (笑 laugh) → 😊
- `ku` (哭 cry) → 😭
- `xin` (心 heart) → ❤️
- `hao` (好 good) → 👍
- `zan` (赞 praise) → 👍

### Internet Slang

Support common expressions:
- `haha` → 😄
- `666` → 👍 (Chinese gaming slang for "awesome")
- `niubi` → 👍 (Chinese slang for "amazing")

## Creating Custom Emoji Tables

### 1. Choose Keywords

Pick keywords that are:
- **Memorable** - Easy to recall
- **Unambiguous** - Don't conflict with common Chinese words
- **Short** - Typically 2-6 characters

### 2. Assign Frequencies

Higher frequency = appears first in candidates:
- Very common: 10000+ (heart, smile, thumbsup)
- Common: 5000-9999 (most emojis)
- Rare: 1000-4999 (obscure emojis)

### 3. Use Unique IDs

Start emoji IDs high to avoid conflicts:
- Chinese characters: 0-99999
- Emojis: 100000+

### 4. Format

```
<keyword> <emoji> <id> <frequency>
```

**Tips:**
- Use tabs for separation
- One entry per line
- UTF-8 encoding required
- Can have multiple keywords for same emoji

### Example

```
# Smileys
smile   😊   100001  10000
happy   😊   100002  8000
xiao    😊   100003  9000

# Hearts
heart   ❤️   100010  12000
love    ❤️   100011  11000
xin     ❤️   100012  10000
ai      ❤️   100013  9000
redheart    ❤️   100014  8000
```

## Integration with IME

### Emoji Trigger Pattern

Many IMEs use special prefixes to trigger emoji mode:
- `:smile:` (Slack/Discord style)
- `/emoji smile` (command style)
- `emoji:smile` (namespace style)

**Implementation:**
```rust
impl ImeEngine {
    pub fn process_key(&mut self, key: KeyEvent) -> KeyResult {
        let input = self.session.input_buffer().text();
        
        // Check for emoji trigger
        if input.starts_with(':') && input.ends_with(':') {
            // Strip colons and search emoji table
            let keyword = &input[1..input.len()-1];
            return self.search_emoji(keyword);
        }
        
        // Normal pinyin processing
        // ...
    }
}
```

### Candidate Filtering

Show emoji candidates separately or mixed:

**Mixed (recommended):**
```
Input: "xiao"
Candidates:
  1. 笑 (laugh - Chinese)
  2. 小 (small - Chinese)
  3. 😊 (smile - emoji)
  4. 销 (sell - Chinese)
```

**Separated:**
```
Input: "xiao"
Chinese:
  1. 笑  2. 小  3. 销
  
Emoji:
  1. 😊  2. 😄
```

## Performance

### Memory Impact

Emoji table size:
- ~100 emojis: <1KB
- ~1000 emojis: ~10KB  
- ~5000 emojis: ~50KB

**Negligible** compared to Chinese lexicon (50MB+)

### Lookup Speed

No measurable impact:
- FST lookup is O(1)
- Emoji entries are small
- Frequency sorting is fast

### Recommendations

- ✅ Load emoji by default (tiny overhead)
- ✅ Include both English and Pinyin keywords
- ✅ Mix emoji with Chinese candidates
- ⚠️ Don't overwhelm users - keep popular emojis first

## Emoji Categories

### Core Set (~100 emojis)

Include the most commonly used:
- Smileys: 😊😂😭❤️😘😎👍
- Gestures: 👍👎✌️👏🙏
- Basic objects: ☕🍕📱💻
- Nature: ☀️🌙⭐🔥

### Extended Set (~1000 emojis)

Add category-specific:
- Food & Drink: 🍜🍚🍺🍵
- Animals: 🐕🐱🐼🐟
- Activities: ⚽🏀🎮🎵
- Travel: 🚗✈️🏨🗺️

### Full Unicode Emoji

All ~3000+ emojis if desired (not recommended - overwhelming)

## Example Workflows

### Workflow 1: Quick Emoji During Chinese Input

```
Type: "wo3ai4ni3"
Candidates: 我爱你
Select: 我爱你
Commit: 我爱你

Type: "xin"
Candidates: 心, 新, 信, ❤️, 💕
Select: ❤️
Commit: 我爱你❤️
```

### Workflow 2: English Emoji Keywords

```
Type: ":smile"
Engine detects ":" prefix
Shows: 😊 😄 😁 😃
Select: 😊
Commit: 😊
```

### Workflow 3: Internet Slang

```
Type: "666"
Candidates: 六六六, 👍, 👍👍👍
Select: 👍
Commit: 👍
```

## Provided Emoji Table

The included `data/emoji.table` contains:
- ✅ ~100 popular emojis
- ✅ English keywords (smile, heart, thumbsup)
- ✅ Pinyin keywords (xiao, xin, zan)
- ✅ Internet slang (haha, 666, niubi)
- ✅ Frequency-ranked

**Categories covered:**
- Smileys & Emotion (25)
- Hand gestures (10)
- Nature (10)
- Food (10)
- Animals (10)
- Common expressions (10)
- Celebrations (5)
- Weather (5)
- Transport (5)
- Objects (10)

## Extending the Emoji Table

### Adding New Emojis

1. Find emoji Unicode: https://unicode.org/emoji/charts/
2. Choose memorable keyword
3. Assign unique ID (100000+)
4. Estimate frequency (based on usage)
5. Add to `emoji.table`

### Example: Adding New Emoji

```
# Adding "robot" emoji
robot   🤖   100300  5000
jiqiren  🤖   100301  4500
ai      🤖   100302  4000
```

### Regenerating After Changes

```bash
# Rebuild the engine data
cargo run --bin convert_tables -- --emoji-table data/emoji.table

# Test in CLI
cargo run --example cli_ime
> smile
  😊 (should appear in candidates)
```

## Testing

### Unit Test

```rust
#[test]
fn test_emoji_support() {
    let data_dir = PathBuf::from("data");
    
    // Load engine with emoji
    let chinese = Lexicon::from_file(data_dir.join("gb_char.table")).unwrap();
    let emoji = Lexicon::from_file(data_dir.join("emoji.table")).unwrap();
    let mut combined = chinese;
    combined.merge(emoji);
    
    let model = Model::new(combined, NGramModel::new(), UserDict::new("test.redb").unwrap(), Config::default());
    let engine = Engine::new(model);
    
    // Search with English keyword
    let candidates = engine.input("smile");
    assert!(candidates.iter().any(|c| c.text.contains("😊")));
    
    // Search with Pinyin keyword
    let candidates = engine.input("xiao");
    assert!(candidates.iter().any(|c| c.text.contains("😊")));
}
```

### CLI Test

```bash
cargo run --example cli_ime

> smile
  Candidates: 😊 😄 ...

> xiao
  Candidates: 笑 小 😊 ...

> heart
  Candidates: ❤️ 💕 ...
```

## Summary

✅ **Emoji support works out-of-the-box** - just add emoji.table  
✅ **Data-driven** - no code changes needed  
✅ **Multiple keywords** - English + Pinyin for flexibility  
✅ **Tiny overhead** - <50KB for 1000 emojis  
✅ **Mixed candidates** - emojis appear alongside Chinese  
✅ **Frequency-ranked** - popular emojis first  
✅ **Extensible** - easy to add more emojis  

The architecture treats emojis as first-class citizens - they're just Unicode characters with phonetic keywords, processed by the same engine that handles Chinese input.

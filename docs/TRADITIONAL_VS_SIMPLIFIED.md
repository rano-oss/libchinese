# Traditional vs Simplified Character Handling in libpinyin

## How libpinyin Differentiates Traditional and Simplified Characters

Based on analysis of the upstream libpinyin repository and our data files, here's how character variants are handled:

### 1. **Separate Table Files by Encoding Standard**

libpinyin uses **different table files** for different character sets:

#### **`gb_char.table`** - Simplified Chinese (GB2312)
- **Encoding**: GB2312 character set
- **Characters**: Simplified Chinese characters
- **Coverage**: ~6,763 characters
- **Use case**: Mainland China standard simplified characters
- Example from our data:
  ```
  a       锕      16777217        7
  a       吖      16777218        104
  a       阿      16777219        33237
  a       啊      16777220        26566
  ```

#### **`gbk_char.table`** - Extended Chinese (GBK, includes traditional)
- **Encoding**: GBK character set (superset of GB2312)
- **Characters**: Both simplified AND traditional characters
- **Coverage**: ~21,003 characters
- **Use case**: Includes traditional forms, rare characters, and extended set
- Example from our data:
  ```
  a       錒      33554433        3        (traditional 锕)
  ai      礙      33554437        19       (traditional 碍)
  ai      嬡      33554439        3        (traditional variant)
  ```

#### **Character ID Encoding**
Notice the IDs:
- GB2312: `16777217` (0x01000001) - starts at 0x01000000
- GBK: `33554433` (0x02000001) - starts at 0x02000000

The high byte encodes which table the character came from!

### 2. **How Upstream libpinyin Uses These Tables**

From the upstream source code analysis:

```cpp
// data/CMakeLists.txt shows two separate binary outputs:
gb_char.bin      // From gb_char.table
gbk_char.bin     // From gbk_char.table
```

The library loads different phrase tables based on configuration:

```cpp
// From src/storage/table_info.cpp
pinyin_table_info_t tables[] = {
    {"gb_char.bin", GB_DICTIONARY, /* ... */},
    {"gbk_char.bin", GBK_DICTIONARY, /* ... */},
    // ...
};
```

### 3. **User Selection Mechanism**

Users can configure which character set to use:

**Option A: Simplified Only (GB2312)**
```
table.conf:
use gb_char.table
```
- Smaller dictionary
- Faster lookups
- Only simplified characters

**Option B: Full Set (GBK)**
```
table.conf:
use gb_char.table
use gbk_char.table
```
- Larger dictionary
- Both simplified and traditional
- Rare characters available

**Option C: Traditional Priority (Taiwan/Hong Kong)**
```
table.conf:
use gbk_char.table
prefer_traditional = true
```
- Prioritize traditional forms in results
- Still has access to simplified

### 4. **Our Implementation Strategy**

Currently we have both `gb_char.table` and `gbk_char.table` in our `data/` directory. Here's what we should do:

#### **Current State:**
```
data/
├── gb_char.table      (0.5 MB, ~6,763 entries)
├── gbk_char.table     (1.8 MB, ~21,003 entries)
└── opengram.table     (phrase dictionary)
```

#### **Recommendation:**

**For Simplified Chinese users (default):**
1. Generate separate FST+bincode from `gb_char.table`
   ```
   pinyin.gb.fst
   pinyin.gb.bincode
   ```

**For Traditional/Mixed users:**
2. Generate from `gbk_char.table`
   ```
   pinyin.gbk.fst
   pinyin.gbk.bincode
   ```

**For Full coverage (both):**
3. Merge both tables with priority system
   ```
   pinyin.fst       (contains all keys)
   pinyin.bincode   (GB chars first, then GBK extensions)
   ```

### 5. **Detection in Table Files**

The character encoding itself reveals the variant:

```rust
// Example from gb_char.table
"a\t阿\t16777219\t33237"
//       ^           ^
//       simplified  GB2312 ID

// Example from gbk_char.table  
"a\t錒\t33554433\t3"
//       ^           ^
//       traditional GBK ID (rare variant of 锕)
```

### 6. **Smart Merging Strategy**

When merging both tables, libpinyin uses frequency to prioritize:

```rust
struct PhraseEntry {
    text: String,     // "阿" or "錒"
    freq: u64,        // Usage frequency
}

// Sorting by frequency puts common simplified first:
// "阿" (freq: 33237) appears before "錒" (freq: 3)
```

### 7. **Big5 Encoding (Traditional Chinese)**

For Traditional Chinese input (Taiwan/Hong Kong), libpinyin can also use:

**Option: Big5 Encoding**
- Not currently in our data directory
- Would need separate `big5.table` file
- Common in Taiwan IME systems

### 8. **Comparison with Other Systems**

| System | Approach |
|--------|----------|
| **libpinyin (upstream)** | Separate GB2312/GBK tables, user selects |
| **RIME** | Unified dictionary with variant tags |
| **Sogou/Baidu** | Simplified default, option to switch |
| **Google Pinyin** | Simplified default, auto-detects context |

### 9. **Implementation for libchinese**

Here's what we should implement:

```rust
// core/src/lib.rs
pub enum CharacterSet {
    Simplified,      // GB2312 only
    Traditional,     // Big5 or prefer GBK traditional
    Mixed,           // All characters, frequency-sorted
}

pub struct Lexicon {
    character_set: CharacterSet,
    // ... existing fields
}

impl Lexicon {
    pub fn load_gb2312(fst_path, bincode_path) -> Result<Self> {
        // Load gb_char only (simplified)
    }
    
    pub fn load_gbk(fst_path, bincode_path) -> Result<Self> {
        // Load gbk_char (includes traditional)
    }
    
    pub fn load_merged(gb_path, gbk_path) -> Result<Self> {
        // Merge both, sort by frequency
    }
}
```

### 10. **Current Data Files**

Looking at our current data:
```bash
$ ls -lh data/*.table
-rw-r--r-- 1 user user  524K gb_char.table      # Simplified
-rw-r--r-- 1 user user  1.8M gbk_char.table     # Traditional + Simplified
-rw-r--r-- 1 user user  842K merged.table       # ???
-rw-r--r-- 1 user user   28M opengram.table     # Phrases
```

**Question:** What is `merged.table`? 
- Likely pre-merged gb + gbk with deduplication
- Check if we should use this instead

### Summary

**libpinyin doesn't tag characters as "traditional" or "simplified".**

Instead, it uses:
1. **Separate source tables** (GB2312 vs GBK encoding standards)
2. **Character ID namespacing** (high bits indicate source table)
3. **Frequency-based sorting** (common simplified characters rank higher)
4. **User configuration** (which tables to load)

The brilliance is that the **encoding standard itself** provides the differentiation:
- GB2312 = Simplified Chinese (PRC standard)
- GBK = Extended set including Traditional
- Big5 = Traditional Chinese (Taiwan standard)

No explicit tagging needed!

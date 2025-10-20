# Apostrophe Handling in libpinyin

## Summary of Upstream Approach

After analyzing the libpinyin source code, here's how they handle apostrophes for pinyin input:

### Key Design Pattern: **Zero ChewingKey**

1. **Apostrophe as Zero Key**:
   - Apostrophes (`'`) are represented as **zero `ChewingKey`** objects in the `PhoneticKeyMatrix`
   - From `phonetic_key_matrix.cpp:33`: 
     ```cpp
     /* zero ChewingKey for "'" symbol and last key in fill_matrix function. */
     ```

2. **Parser Behavior**:
   - The `FullPinyinParser2::parse()` function in `pinyin_parser2.cpp` handles apostrophes:
     - Apostrophes are **NOT included** in `parse_one_key()` - line 169:
       ```cpp
       /* "'" are not accepted in parse_one_key. */
       gchar * input = g_strndup(pinyin, len);
       assert(NULL == strchr(input, '\''));
       ```
     - During full string parsing, when encountering `'`, it advances the step without creating a syllable
     - Line 224: `if (input[i] == '\'') { ... continue; }`

3. **Matrix Filling** (`phonetic_key_matrix.cpp:69-82`):
   ```cpp
   /* fill zero keys for "'". */
   ChewingKeyRest * next_key_rest = NULL;
   for (i = 0; i < key_rests->len - 1; ++i) {
       key_rest = &g_array_index(key_rests, ChewingKeyRest, i);
       next_key_rest = &g_array_index(key_rests, ChewingKeyRest, i + 1);

       for (size_t fill = key_rest->m_raw_end;
            fill < next_key_rest->m_raw_begin; ++fill) {
           zero_key_rest.m_raw_begin = fill;
           zero_key_rest.m_raw_end = fill + 1;
           matrix->append(fill, zero_key, zero_key_rest);
       }
   }
   ```

4. **Lookup Handling** (`pinyin_lookup2.cpp`, `phonetic_key_matrix.cpp:378-397`):
   - When searching the matrix, zero keys are **skipped**:
     ```cpp
     const ChewingKey zero_key;
     if (zero_key == key) {
         /* assume only one key here for "'" or the last key. */
         assert(1 == size);
         return search_matrix_recur(cached_keys, table, matrix,
                                   newstart, end, ranges, longest);
     }
     ```

5. **Candidate Generation** (`pinyin.cpp:2260`):
   - Skips consecutive zero ChewingKeys when building phrases:
     ```cpp
     /* skip the consecutive zero ChewingKey "'",
        to avoid duplicates of candidates. */
     ```

### How User Input Works

1. **User types**: `nihao` (no apostrophe)
2. **Parser generates**: Array of `ChewingKey` objects for valid syllables:
   - Tries to parse `"ni"` → valid
   - Tries to parse `"hao"` → valid  
   - Tries to parse `"nig"`, `"niha"`, etc. → invalid
3. **Matrix stores**: Both `["ni", "hao"]` AND other possible segmentations
4. **Lookup**: Searches for phrase matches using the key combinations
5. **Keys in dictionary**: Stored WITH apostrophes (like `"ni'hao"`) but...
6. **Matching**: The apostrophes serve as **syllable boundaries** in storage, but lookups work with syllable arrays

### Critical Insight for Our Implementation

**The dictionary doesn't actually use apostrophes for lookup!**

The apostrophes in stored keys like `"ni'hao"` are:
- **Storage format markers** indicating syllable boundaries
- **NOT used during actual lookups** - lookups use arrays of `ChewingKey` objects
- Filled as "zero keys" in the matrix to maintain position tracking

### Recommended Approach for libchinese

We have several options:

#### Option 1: **Follow Upstream Exactly**
- Store keys with apostrophes (like `"ni'hao"`) in FST
- During lookup, parse user input into syllables
- For each syllable array, construct the apostrophe-separated key for FST lookup
- Example: `["ni", "hao"]` → look up `"ni'hao"`

#### Option 2: **Simplified No-Apostrophe Storage** (EASIER)
- Store BOTH versions in FST:
  - `"ni'hao"` → index 123
  - `"nihao"` → index 123 (same payload)
- User types `"nihao"` → direct FST match
- Minimal code changes needed

#### Option 3: **Parser-Based Approach** (MOST CORRECT)
- Implement a proper pinyin parser (like `FullPinyinParser2`)
- Parse user input `"nihao"` into syllable array `["ni", "hao"]`
- Construct lookup key `"ni'hao"` from syllables
- Use existing FST with apostrophe keys

### Implementation Recommendation

For **libchinese**, I recommend **Option 3** (parser-based) because:

1. ✅ Matches upstream libpinyin design
2. ✅ Handles ambiguous input correctly:
   - `"xian"` could be `"xi'an"` (西安) or `"xian"` (先)
   - Parser tries all valid syllable segmentations
3. ✅ Works with existing converted data (already has apostrophes)
4. ✅ Enables fuzzy matching and other advanced features

### Example Parser Implementation

```rust
pub struct PinyinParser {
    valid_syllables: HashSet<String>,
}

impl PinyinParser {
    // Parse "nihao" into possible syllable arrays
    pub fn parse(&self, input: &str) -> Vec<Vec<String>> {
        // Dynamic programming to find all valid segmentations
        // Returns: [["ni", "hao"], ...]
    }
    
    // Convert syllable array to FST lookup key
    pub fn to_key(syllables: &[String]) -> String {
        syllables.join("'")
    }
}

// Usage in Engine:
pub fn lookup(&self, input: &str) -> Vec<Candidate> {
    let segmentations = self.parser.parse(input);
    let mut results = Vec::new();
    
    for syllables in segmentations {
        let key = PinyinParser::to_key(&syllables);
        let phrases = self.lexicon.lookup(&key);
        results.extend(phrases);
    }
    
    results
}
```

### Next Steps

1. Add a `valid_syllables.txt` file with all valid pinyin syllables
2. Implement `PinyinParser::parse()` using dynamic programming
3. Update `Engine::input()` to use parser before lexicon lookup
4. Keep existing FST data with apostrophes (no regeneration needed!)


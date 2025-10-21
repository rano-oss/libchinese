# User Dictionary Import/Export Tools

Tools for managing user dictionaries in libchinese - backup, restore, and import custom phrases.

## Tools Overview

### 1. export_userdict - Export User Dictionary

Export your learned phrases and frequencies to JSON or CSV format for backup or analysis.

**Usage:**
```pwsh
# Export to JSON (default)
cargo run -p export_userdict -- --db data/userdict.redb

# Export to CSV file
cargo run -p export_userdict -- --db data/userdict.redb --format csv --output backup.csv

# Export sorted by frequency
cargo run -p export_userdict -- --db data/userdict.redb --sort-by-freq --output top_phrases.json
```

**Output Formats:**
- **JSON**: `[["phrase", frequency], ...]` - Easy to parse programmatically
- **CSV**: `phrase,frequency` with header - Easy to view in spreadsheets

**Example Output (JSON):**
```json
[
  ["你好", 150],
  ["世界", 87],
  ["测试", 42]
]
```

**Example Output (CSV):**
```csv
phrase,frequency
你好,150
世界,87
测试,42
```

---

### 2. import_phrases - Import Custom Phrases

Import phrases from JSON, CSV, or plain text files into your user dictionary.

**Usage:**
```pwsh
# Import from JSON
cargo run -p import_phrases -- --db data/userdict.redb --input my_phrases.json

# Import from CSV
cargo run -p import_phrases -- --db data/userdict.redb --input phrases.csv --format csv

# Import from text file (one phrase per line, frequency defaults to 1)
cargo run -p import_phrases -- --db data/userdict.redb --input wordlist.txt --format txt

# Dry run to preview what would be imported
cargo run -p import_phrases -- --db data/userdict.redb --input phrases.json --dry-run
```

**Input Formats:**

**JSON** (`--format json`):
```json
[
  ["专业术语", 10],
  ["技术词汇", 5]
]
```

**CSV** (`--format csv`):
```csv
phrase,frequency
专业术语,10
技术词汇,5
```

**TXT** (`--format txt`):
```
专业术语
技术词汇
常用短语
```
*Note: TXT format sets frequency to 1 for all phrases*

**Import Modes:**
- `--mode add` (default): Adds to existing frequencies
- `--mode replace`: Would replace frequencies (not yet implemented)

---

## Common Workflows

### Backup User Dictionary
```pwsh
# Create a timestamped backup
$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
cargo run -p export_userdict -- --db data/userdict.redb --format json --output "backup_$timestamp.json"
```

### Restore from Backup
```pwsh
# Import phrases from backup
cargo run -p import_phrases -- --db data/userdict.redb --input backup_20251021_143022.json
```

### Import Custom Domain Vocabulary
```pwsh
# Create a text file with domain-specific terms
echo "机器学习`n深度学习`n神经网络" | Out-File -Encoding UTF8 ml_terms.txt

# Import with default frequency
cargo run -p import_phrases -- --db data/userdict.redb --input ml_terms.txt --format txt
```

### Analyze Usage Patterns
```pwsh
# Export sorted by frequency to see most-used phrases
cargo run -p export_userdict -- --db data/userdict.redb --format csv --sort-by-freq --output usage_stats.csv

# Open in Excel/LibreOffice to analyze
```

### Share Phrases Between Users
```pwsh
# User A exports their dictionary
cargo run -p export_userdict -- --db userA/userdict.redb --output phrases_to_share.json

# User B imports selected phrases
cargo run -p import_phrases -- --db userB/userdict.redb --input phrases_to_share.json --dry-run
cargo run -p import_phrases -- --db userB/userdict.redb --input phrases_to_share.json
```

---

## Integration with IME

These tools work directly with the user dictionary database files used by `libpinyin` and `libzhuyin` engines.

**Default database locations:**
- Pinyin: `data/userdict.redb` (or custom path via Config)
- Zhuyin: `data/zhuyin/userdict.redb`

**Safe to use while IME is running:** The tools use read/write transactions through `redb`, which handles concurrent access safely.

---

## Technical Notes

### File Formats
- **JSON**: Uses `serde_json` for robust parsing
- **CSV**: Simple parser handles quoted fields for phrases with commas
- **TXT**: One phrase per line, skips empty lines and `#` comments

### Frequency Semantics
- Frequencies are **cumulative**: importing adds to existing values
- Higher frequencies = higher ranking in candidate lists
- The IME engine multiplies learned frequencies by user-specific weights

### Database Format
- Uses `redb` embedded database (ACID-compliant)
- Table: `user_dict` with `(String, u64)` key-value pairs
- Portable across platforms (same database file works on Windows/Linux/macOS)

---

## Error Handling

All tools provide helpful error messages:

```pwsh
# Database not found
Error: Failed to open user dict: unable to open database file

# Invalid JSON format
Error: Failed to parse JSON: expected value at line 1 column 1

# Empty file
Parsed 0 phrases from input.txt
✓ Import complete! (nothing to import)
```

---

## Future Enhancements

- [ ] `--mode replace` for import_phrases (set exact frequencies)
- [ ] Merge tool to combine multiple dictionaries
- [ ] Statistics command to show dictionary size, top phrases, coverage
- [ ] Export filters (e.g., only phrases above certain frequency)
- [ ] Batch operations (import from multiple files)

---

## See Also

- `tools/inspect_redb/` - Low-level database inspection
- Core UserDict API: `core/src/userdict.rs`
- Engine commit() method for runtime learning

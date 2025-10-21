# convert_table

Single-step tool to generate all data artifacts for Chinese IME datasets from source `.table` files.

## What it does

Converts source `.table` files into optimized FST + bincode data structures:

1. **Lexicon** (key → phrase mappings)
   - `lexicon.fst`: FST map from pinyin/zhuyin keys to index
   - `lexicon.bincode`: Bincode-serialized Vec<Vec<LexEntry>> with phrase text, token IDs, and frequencies

2. **N-gram Model** (statistical language model)
   - `ngram.bincode`: Bincode-serialized NGramModel with Modified Kneser-Ney smoothing
   - Contains unigram, bigram, and trigram log-probabilities

3. **Interpolation Weights** (per-prefix lambda weights)
   - `lambdas.fst`: FST map from character prefix to index
   - `lambdas.bincode`: Bincode-serialized Vec<Lambdas> with 3-way weights [λ₁, λ₂, λ₃]
   - Computed using deleted interpolation for optimal n-gram mixing

## Generated Datasets

### Simplified (简体拼音)
- **Source**: `gb_char.table`, `merged.table`, `opengram.table`, `punct.table`
- **Keys**: Pinyin syllables (e.g., `zhong'guo`, `han'zi`)
- **Characters**: Simplified Chinese
- **N-gram tokenization**: Pinyin syllable-level
- **Output**: `data/converted/simplified/`

### Traditional (繁體拼音)
- **Source**: `tsi.table` (converted from Zhuyin)
- **Keys**: Toneless pinyin (e.g., `zhong'guo`, `han'zi`)
- **Characters**: Traditional Chinese
- **N-gram tokenization**: Pinyin syllable-level
- **Output**: `data/converted/traditional/`

### Zhuyin Traditional (注音/ㄅㄆㄇㄈ)
- **Source**: `tsi.table` (original Zhuyin keys preserved)
- **Keys**: Bopomofo/Zhuyin symbols (e.g., `ㄓㄨㄥ'ㄍㄨㄛ`, `ㄏㄢ'ㄗ`)
- **Characters**: Traditional Chinese
- **N-gram tokenization**: Character-level
- **Output**: `data/converted/zhuyin_traditional/`

## Usage

```bash
# Run from repository root
cargo run --release -p convert_table
```

**Note**: Hardcoded paths expect:
- Source tables in `data/` and `data/zhuyin/`
- Output written to `data/converted/{simplified,traditional,zhuyin_traditional}/`

## Implementation Details

### Key Conversion
- **Traditional dataset**: Zhuyin keys from `tsi.table` are converted to toneless pinyin using a bopomofo→pinyin mapping table
- **Zhuyin dataset**: Original Zhuyin keys are preserved (tones stripped for normalization)

### N-gram Model
- Uses **Modified Kneser-Ney smoothing** with continuation count-based discounting
- Computes:
  - Unigram probabilities from continuation counts
  - Bigram/trigram probabilities with interpolated backoff
- Tokenization mode:
  - **Pinyin datasets**: syllable-level (split on apostrophes in keys)
  - **Zhuyin dataset**: character-level (from phrase text)

### Interpolation Weights
- Computed using **3-way deleted interpolation**
- For each character prefix:
  - Evaluates which n-gram level (unigram/bigram/trigram) predicts best using leave-one-out counts
  - Accumulates evidence and normalizes to [λ₁, λ₂, λ₃] weights
- Minimum weight threshold (0.01) ensures all levels contribute

## Output Format

Each dataset directory contains exactly 5 files:

```
data/converted/{dataset}/
├── lexicon.fst        (~640-770 KB) - FST key→index map
├── lexicon.bincode    (~3.7-4.3 MB) - Phrase entries
├── ngram.bincode      (~4.3-6.5 MB) - N-gram model
├── lambdas.fst        (~20-25 KB)   - Lambda prefix→index map
└── lambdas.bincode    (~64-67 KB)   - Per-prefix weights
```

## Performance

- **Debug build**: ~30-60 seconds per dataset
- **Release build**: ~10-20 seconds per dataset
- All three datasets generated in one run

## Dependencies

- `fst`: FST construction and serialization
- `bincode`: Fast binary serialization
- `libchinese-core`: NGramModel and Lambdas types
- `serde`: Serialization traits
- `anyhow`: Error handling

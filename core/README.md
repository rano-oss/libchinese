# libchinese-core

Core building blocks for Chinese input method engines: memory-mapped lexicons (FST + dat), word bigram models, user dictionary (redb), configuration, and shared IME session logic.

Used by the [`libpinyin`](https://github.com/rano-oss/libchinese/tree/main/libpinyin) and [`libzhuyin`](https://github.com/rano-oss/libchinese/tree/main/libzhuyin) crates.

## Data files

This crate does **not** ship language data. At runtime, point engines at a directory containing converted artifacts (for example `lexicon.fst`, `lexicon.dat`, `word_bigram.bincode`). Build or obtain data from the [libchinese](https://github.com/rano-oss/libchinese) repository (`data/` and `tools/convert_table`).

## License

GPL-3.0-or-later — see `LICENSE` in this crate (same as the workspace root).

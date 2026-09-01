# libzhuyin

Zhuyin (Bopomofo) input method engine with HSU, ETEN, and standard keyboard layouts, built on [`libchinese-core`](https://github.com/rano-oss/libchinese/tree/main/core).

## Data files

Language data is **not** bundled. Provide a converted zhuyin data directory (`lexicon.fst`, `lexicon.dat`, `word_bigram.bincode`, etc.) when creating an engine. See the [libchinese](https://github.com/rano-oss/libchinese) repository.

## License

GPL-3.0-or-later — see `LICENSE` in this crate (same as the workspace root).

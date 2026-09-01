# libpinyin

Rust Pinyin input method engine: segmentation, candidate ranking, double pinyin schemes, English/emoji mixed input, and IME session APIs compatible with desktop integration (e.g. pinyinwl).

Built on [`libchinese-core`](https://github.com/rano-oss/libchinese/tree/main/core).

## Data files

Language data is **not** included in the crate. Pass a data directory to `Engine::from_data_dir` or `create_ime_engine` (typically `lexicon.fst`, `lexicon.dat`, `word_bigram.bincode`, plus optional aux tables). See the [libchinese](https://github.com/rano-oss/libchinese) repo for `data/converted/` layout and conversion tools.

## License

GPL-3.0-or-later — see `LICENSE` in this crate (same as the workspace root).

## Future Work

1. **libzhuyin parser integration:** Add fuzzy matching to ZhuyinParser
2. **Dynamic configuration:** Allow runtime fuzzy rule updates
3. **Performance optimization:** Cache fuzzy alternatives for common syllables
4. **Extended rules:** Add more keyboard layout-specific corrections for zhuyin

### Future Enhancements:
1. Add tone support (ni3hao3 → ni'hao with tones)
2. Implement partial pinyin (e.g., "nh" → "ni'hao")
3. Add pinyin correction for common typos
4. Optimize parser for very long inputs

### Next Steps

1. Add a `valid_syllables.txt` file with all valid pinyin syllables
2. Implement `PinyinParser::parse()` using dynamic programming
3. Update `Engine::input()` to use parser before lexicon lookup
4. Keep existing FST data with apostrophes (no regeneration needed!)
3. **FuzzyMap caching:** Cache `alternative_strings()` results for common syllables (engine-level caching is done)
4. **Extended rules:** Add more keyboard layout-specific corrections for zhuyin (Dachen, IBM, etc.)

### Future Enhancements:
2. **Implement partial pinyin** (e.g., "n" → "ni", "nh" → "nihao") - upstream has `PINYIN_INCOMPLETE` option
3. **Add more pinyin corrections** beyond fuzzy alternates - upstream has `PINYIN_CORRECT_*` flags (ue/ve, v/u, etc.)
4. **Optimize parser for very long inputs** - add apostrophe separator support like upstream (reduces DP search space)

### Next Steps
1. Add a `valid_syllables.txt` file with all valid pinyin syllables
2. Implement `PinyinParser::parse()` using dynamic programming
3. Update `Engine::input()` to use parser before lexicon lookup
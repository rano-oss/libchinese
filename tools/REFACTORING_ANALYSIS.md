## TODOS before completion:
1. Fuzzy can probably be reused in libzhuyin as well as libpinyin? Clean up and move to core, make it only initialize with configuration.
2. Engine can probably be merged(same for pinyin and zhuyin) as well, should be fairly similar in behavior, just data and parser that is different. 
3. More code should be moved from redb to bincode(remove json metadata output, figure out what metadata is even needed)
4. Data scripts need to be rewritten so it handles creating all the binaries and fst files correctly, also separate into pinyin.simplified.fst/bin and pinyin.traditional.fst/bin.
Zhuyin also needs separation between traditional and simplified. Dealing with merged we will avoid as it is too much of a niche for now. 
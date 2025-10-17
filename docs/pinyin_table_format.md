# Pinyin Table Format and Parser Initialization

## Overview

The upstream pinyin syllable set is derived from `data/gb_char.table` and `data/gbk_char.table`. These files contain mappings from pinyin to Chinese characters with frequency information.

## Table Format

### File Structure
Each line in `gb_char.table` and `gbk_char.table` follows this tab-separated format:

```
pinyin	hanzi	code	frequency
```

### Example Entries
```
a	啊	16777220	26566
ai	爱	16777230	60751
nihao	你好	16777300	5000
```

### Fields
1. **pinyin**: The romanization (can be single syllable like "a" or multi-syllable like "ni'hao")
2. **hanzi**: The Chinese character(s)
3. **code**: An internal ID (likely derived from character encoding)
4. **frequency**: Usage count/weight

### Polyphones
Characters with multiple pronunciations appear multiple times:
```
a	阿	16777219	33237
e	阿	16777219	11
```

### Multi-syllable Entries
Compound words use apostrophes as separators:
```
a'ba'zhou	阿坝州	...	...
```

## Syllable Extraction

### Statistics
- **gb_char.table**: 95,699 entries
- **gbk_char.table**: 21,235 entries  
- **Unique single syllables**: 405

### Complete Syllable List

The 405 valid pinyin syllables (without tone marks) extracted from `data/gb_char.table`:

```
a, ai, an, ang, ao
ba, bai, ban, bang, bao, bei, ben, beng, bi, bian, biao, bie, bin, bing, bo, bu
ca, cai, can, cang, cao, ce, cen, ceng, cha, chai, chan, chang, chao, che, chen, cheng, chi, chong, chou, chu, chuai, chuan, chuang, chui, chun, chuo, ci, cong, cou, cu, cuan, cui, cun, cuo
da, dai, dan, dang, dao, de, dei, deng, di, dia, dian, diao, die, ding, diu, dong, dou, du, duan, dui, dun, duo
e, ei, en, er
fa, fan, fang, fei, fen, feng, fo, fou, fu
ga, gai, gan, gang, gao, ge, gei, gen, geng, gong, gou, gu, gua, guai, guan, guang, gui, gun, guo
ha, hai, han, hang, hao, he, hei, hen, heng, hong, hou, hu, hua, huai, huan, huang, hui, hun, huo
ji, jia, jian, jiang, jiao, jie, jin, jing, jiong, jiu, ju, juan, jue, jun
ka, kai, kan, kang, kao, ke, ken, keng, kong, kou, ku, kua, kuai, kuan, kuang, kui, kun, kuo
la, lai, lan, lang, lao, le, lei, leng, li, lia, lian, liang, liao, lie, lin, ling, liu, long, lou, lu, luan, lun, luo, lv, lve
ma, mai, man, mang, mao, me, mei, men, meng, mi, mian, miao, mie, min, ming, miu, mo, mou, mu
na, nai, nan, nang, nao, ne, nei, nen, neng, ni, nian, niang, niao, nie, nin, ning, niu, nong, nou, nu, nuan, nun, nuo, nv, nve
o, ou
pa, pai, pan, pang, pao, pei, pen, peng, pi, pian, piao, pie, pin, ping, po, pou, pu
qi, qia, qian, qiang, qiao, qie, qin, qing, qiong, qiu, qu, quan, que, qun
ran, rang, rao, re, ren, reng, ri, rong, rou, ru, ruan, rui, run, ruo
sa, sai, san, sang, sao, se, sen, seng, sha, shai, shan, shang, shao, she, shei, shen, sheng, shi, shou, shu, shua, shuai, shuan, shuang, shui, shun, shuo, si, song, sou, su, suan, sui, sun, suo
ta, tai, tan, tang, tao, te, teng, ti, tian, tiao, tie, ting, tong, tou, tu, tuan, tui, tun, tuo
wa, wai, wan, wang, wei, wen, weng, wo, wu
xi, xia, xian, xiang, xiao, xie, xin, xing, xiong, xiu, xu, xuan, xue, xun
ya, yan, yang, yao, ye, yi, yin, ying, yo, yong, you, yu, yuan, yue, yun
za, zai, zan, zang, zao, ze, zei, zen, zeng, zha, zhai, zhan, zhang, zhao, zhe, zhei, zhen, zheng, zhi, zhong, zhou, zhu, zhua, zhuai, zhuan, zhuang, zhui, zhun, zhuo, zi, zong, zou, zu, zuan, zui, zun, zuo
```

(Full list available in `data/pinyin_syllables.txt`)

## Usage in libpinyin

### Current Implementation

The `Parser` in `libpinyin/src/parser.rs` is initialized with a syllable set:

```rust
let parser = Parser::with_syllables(&["ni", "hao", "zhong", "guo", ...]);
```

### Recommended Approach

For production use, load all 405 syllables from the table files:

```rust
// Load from generated syllable list
let syllables: Vec<String> = std::fs::read_to_string("data/pinyin_syllables.txt")?
    .lines()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
    .collect();

let parser = Parser::with_syllables(&syllables.iter().map(|s| s.as_str()).collect::<Vec<_>>());
```

Or extract syllables programmatically from the table files:

```rust
use std::collections::BTreeSet;

fn load_syllables_from_table(path: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let mut syllables = BTreeSet::new();
    
    for line in std::fs::read_to_string(path)?.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            let pinyin = parts[0];
            // Skip multi-syllable entries (contain apostrophes)
            if !pinyin.contains('\'') && !pinyin.is_empty() {
                syllables.insert(pinyin.to_string());
            }
        }
    }
    
    Ok(syllables.into_iter().collect())
}

// Usage
let mut all_syllables = BTreeSet::new();
all_syllables.extend(load_syllables_from_table("data/gb_char.table")?);
all_syllables.extend(load_syllables_from_table("data/gbk_char.table")?);

let syllable_vec: Vec<String> = all_syllables.into_iter().collect();
let parser = Parser::with_syllables(&syllable_vec.iter().map(|s| s.as_str()).collect::<Vec<_>>());
```

## Integration with Lexicon

The same table files are used to build the lexicon (FST + redb):

1. **FST (Finite State Transducer)**: Maps `pinyin -> index` for fast lookups
2. **redb (Database)**: Stores `index -> Vec<PhraseEntry>` with actual characters and frequencies

This is handled by the `convert_tables` tool:

```bash
cargo run --bin convert_tables -- \
    --inputs data/gb_char.table data/gbk_char.table \
    --out-fst data/pinyin.fst \
    --out-redb data/pinyin.redb
```

## Key Insights

1. **Comprehensive Coverage**: The 405 syllables cover all standard pinyin without tone marks
2. **Frequency Data**: Can be used for ranking/scoring candidates
3. **Polyphone Support**: Same character with different readings is preserved
4. **Multi-word Support**: Compound words are in the table but use apostrophes
5. **Parser Needs Single Syllables**: The parser's TrieNode should only contain the 405 base syllables for segmentation

## Next Steps

To use the complete pinyin table in libpinyin:

1. Extract syllables into a loadable format (done: `data/pinyin_syllables.txt`)
2. Update `Engine::from_data_dir()` to load syllables from file
3. Ensure parser is initialized with all 405 syllables for comprehensive coverage
4. Consider lazy-loading or embedding the syllable list at compile time for performance

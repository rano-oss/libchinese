//! Pinyin input method engine
//!
//! Provides a high-level Engine that combines parser, model, and fuzzy matching
//! into a simple `input(text) -> Vec<Candidate>` API with caching optimization.
//!
//! This is now a thin wrapper around the generic core::Engine<Parser>.

use std::error::Error;
use std::sync::Arc;

use crate::parser::Parser;
use libchinese_core::{Candidate, Lexicon, Model, UserDict};

/// Public engine for libpinyin.
///
/// This wraps the generic core::Engine<Parser> with pinyin-specific loading logic.
/// All actual IME logic (segmentation, fuzzy matching, caching, scoring) is in core.
///
/// The inner engine is wrapped in Arc to allow cheap cloning for sharing across editors.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<libchinese_core::Engine<Parser>>,
}

/// All standard pinyin syllables (without tone markers).
/// This list includes all valid pinyin syllables in Mandarin Chinese.
pub const PINYIN_SYLLABLES: &[&str] = &[
    "a", "ai", "an", "ang", "ao", "ba", "bai", "ban", "bang", "bao", "bei", "ben", "beng", "bi",
    "bian", "biao", "bie", "bin", "bing", "bo", "bu", "ca", "cai", "can", "cang", "cao", "ce",
    "cen", "ceng", "cha", "chai", "chan", "chang", "chao", "che", "chen", "cheng", "chi", "chong",
    "chou", "chu", "chuai", "chuan", "chuang", "chui", "chun", "chuo", "ci", "cong", "cou", "cu",
    "cuan", "cui", "cun", "cuo", "da", "dai", "dan", "dang", "dao", "de", "dei", "deng", "di",
    "dia", "dian", "diao", "die", "ding", "diu", "dong", "dou", "du", "duan", "dui", "dun", "duo",
    "e", "ei", "en", "er", "fa", "fan", "fang", "fei", "fen", "feng", "fo", "fou", "fu", "ga",
    "gai", "gan", "gang", "gao", "ge", "gei", "gen", "geng", "gong", "gou", "gu", "gua", "guai",
    "guan", "guang", "gui", "gun", "guo", "ha", "hai", "han", "hang", "hao", "he", "hei", "hen",
    "heng", "hong", "hou", "hu", "hua", "huai", "huan", "huang", "hui", "hun", "huo", "ji", "jia",
    "jian", "jiang", "jiao", "jie", "jin", "jing", "jiong", "jiu", "ju", "juan", "jue", "jun",
    "ka", "kai", "kan", "kang", "kao", "ke", "ken", "keng", "kong", "kou", "ku", "kua", "kuai",
    "kuan", "kuang", "kui", "kun", "kuo", "la", "lai", "lan", "lang", "lao", "le", "lei", "leng",
    "li", "lia", "lian", "liang", "liao", "lie", "lin", "ling", "liu", "lo", "long", "lou", "lu",
    "luan", "lun", "luo", "lv", "lve", "ma", "mai", "man", "mang", "mao", "me", "mei", "men",
    "meng", "mi", "mian", "miao", "mie", "min", "ming", "miu", "mo", "mou", "mu", "na", "nai",
    "nan", "nang", "nao", "ne", "nei", "nen", "neng", "ng", "ni", "nian", "niang", "niao", "nie",
    "nin", "ning", "niu", "nong", "nou", "nu", "nuan", "nuo", "nv", "nve", "o", "ou", "pa", "pai",
    "pan", "pang", "pao", "pei", "pen", "peng", "pi", "pian", "piao", "pie", "pin", "ping", "po",
    "pou", "pu", "qi", "qia", "qian", "qiang", "qiao", "qie", "qin", "qing", "qiong", "qiu", "qu",
    "quan", "que", "qun", "ran", "rang", "rao", "re", "ren", "reng", "ri", "rong", "rou", "ru",
    "ruan", "rui", "run", "ruo", "sa", "sai", "san", "sang", "sao", "se", "sen", "seng", "sha",
    "shai", "shan", "shang", "shao", "she", "shei", "shen", "sheng", "shi", "shou", "shu", "shua",
    "shuai", "shuan", "shuang", "shui", "shun", "shuo", "si", "song", "sou", "su", "suan", "sui",
    "sun", "suo", "ta", "tai", "tan", "tang", "tao", "te", "teng", "ti", "tian", "tiao", "tie",
    "ting", "tong", "tou", "tu", "tuan", "tui", "tun", "tuo", "wa", "wai", "wan", "wang", "wei",
    "wen", "weng", "wo", "wu", "xi", "xia", "xian", "xiang", "xiao", "xie", "xin", "xing", "xiong",
    "xiu", "xu", "xuan", "xue", "xun", "ya", "yan", "yang", "yao", "ye", "yi", "yin", "ying", "yo",
    "yong", "you", "yu", "yuan", "yue", "yun", "za", "zai", "zan", "zang", "zao", "ze", "zei",
    "zen", "zeng", "zha", "zhai", "zhan", "zhang", "zhao", "zhe", "zhen", "zheng", "zhi", "zhong",
    "zhou", "zhu", "zhua", "zhuai", "zhuan", "zhuang", "zhui", "zhun", "zhuo", "zi", "zong", "zou",
    "zu", "zuan", "zui", "zun", "zuo",
];

impl Engine {
    /// Construct an Engine from a pre-built `Model` and a `Parser`.
    ///
    /// Uses standard pinyin fuzzy rules configured in the parser.
    pub fn new(model: Model) -> Self {
        let parser = Parser::with_syllables(PINYIN_SYLLABLES);
        Self {
            inner: Arc::new(libchinese_core::Engine::new(model, parser)),
        }
    }

    /// Get a cloned Arc to the inner core engine.
    ///
    /// Useful for sharing the engine with ImeEngine and other components.
    pub fn inner_arc(&self) -> Arc<libchinese_core::Engine<Parser>> {
        Arc::clone(&self.inner)
    }

    /// Load an engine from a model directory containing runtime artifacts.
    ///
    /// Expected layout (data-dir):
    ///  - lexicon.fst + lexicon.bincode    (lexicon)
    ///  - word_bigram.bin                  (word-level bigrams)
    ///  - userdict.redb                    (persistent user dictionary)
    pub fn from_data_dir<P: AsRef<std::path::Path>>(data_dir: P) -> Result<Self, Box<dyn Error>> {
        let data_dir = data_dir.as_ref();

        // Load lexicon from fst + bincode (required)
        let fst_path = data_dir.join("lexicon.fst");
        let bincode_path = data_dir.join("lexicon.bincode");

        let lex = Lexicon::load_from_fst_bincode(&fst_path, &bincode_path).map_err(|e| {
            format!(
                "failed to load lexicon from {:?} and {:?}: {}",
                fst_path, bincode_path, e
            )
        })?;

        // Userdict: use persistent userdict at ~/.pinyin/userdict.redb
        let userdict = {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".to_string());
            let ud_path = std::path::PathBuf::from(home)
                .join(".pinyin")
                .join("userdict.redb");

            // Create directory if needed
            if let Some(parent) = ud_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            UserDict::new(&ud_path)?
        };

        // Load word bigram if present
        let word_bigram = {
            let wb_path = data_dir.join("word_bigram.bin");
            if wb_path.exists() {
                match libchinese_core::WordBigram::load(&wb_path) {
                    Ok(wb) => {
                        eprintln!("Loaded word bigram from {:?}", wb_path);
                        wb
                    }
                    Err(e) => {
                        eprintln!(
                            "warning: failed to load word_bigram.bin: {}, using empty model",
                            e
                        );
                        libchinese_core::WordBigram::new()
                    }
                }
            } else {
                eprintln!("word_bigram.bin not found, using empty model");
                libchinese_core::WordBigram::new()
            }
        };

        let model = Model::new(lex, word_bigram, userdict, libchinese_core::Config::default());
        // let parser = Parser::with_syllables(PINYIN_SYLLABLES);
        Ok(Self::new(model))
    }

    /// Get cache statistics (hits, misses, hit rate)
    pub fn cache_stats(&self) -> (usize, usize, f64) {
        let (hits, misses) = self.inner.cache_stats();
        let total = hits + misses;
        let hit_rate = if total > 0 {
            hits as f64 / total as f64
        } else {
            0.0
        };
        (hits, misses, hit_rate)
    }

    /// Get cache size (number of cached entries)
    pub fn cache_size(&self) -> usize {
        // Note: the core engine doesn't expose cache size directly
        // This is an approximation based on cache stats
        let (hits, misses) = self.inner.cache_stats();
        // Rough estimate: total queries is an upper bound on cache size
        hits + misses
    }

    /// Clear the cache
    pub fn clear_cache(&mut self) {
        self.inner.clear_cache();
    }

    /// Commit a phrase to the user dictionary (learning).
    ///
    /// This increases the frequency/score for the given phrase, allowing the
    /// IME to learn user preferences over time.
    ///
    /// # Example
    /// ```no_run
    /// # use libpinyin::Engine;
    /// # let mut engine = Engine::from_data_dir("data").unwrap();
    /// let candidates = engine.input("nihao");
    /// if let Some(selected) = candidates.first() {
    ///     engine.commit(&selected.text);
    /// }
    /// ```
    pub fn commit(&self, phrase: &str) {
        self.inner.commit(phrase);
    }

    /// Get reference to the user dictionary for learning.
    ///
    /// Provides access to user-learned data including user bigrams
    /// for personalized predictions.
    pub fn userdict(&self) -> &UserDict {
        self.inner.userdict()
    }

    /// Get reference to the configuration.
    pub fn config(&self) -> std::cell::Ref<libchinese_core::Config> {
        self.inner.config()
    }

    /// Get mutable reference to the configuration.
    pub fn config_mut(&self) -> std::cell::RefMut<libchinese_core::Config> {
        self.inner.config_mut()
    }

    /// Main input API. Returns ranked `Candidate` items for the given raw input.
    ///
    /// Delegates to core::Engine which handles:
    /// 1. Parser segmentation into syllable sequences
    /// 2. Fuzzy key generation for all alternatives
    /// 3. Lexicon lookups and n-gram scoring
    /// 4. Penalty application for fuzzy matches
    /// 5. Result caching
    pub fn input(&self, input: &str) -> Vec<Candidate> {
        self.inner.input(input)
    }
}

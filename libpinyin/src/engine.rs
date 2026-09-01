//! Pinyin input method engine
//!
//! Provides a high-level Engine that combines parser, model, and fuzzy matching
//! into a simple `input(text) -> Vec<Candidate>` API with caching optimization.
//!
//! This is now a thin wrapper around the generic core::Engine<Parser>.

use std::error::Error;
use std::sync::Arc;

use crate::parser::Parser;
use libchinese_core::{Candidate, EmojiDict, EnglishDict, Lexicon, Model, UserDict, WordBigram};

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

    /// Construct an Engine with emoji support.
    pub fn new_with_emoji(model: Model, emoji_dict: EmojiDict) -> Self {
        let parser = Parser::with_syllables(PINYIN_SYLLABLES);
        let mut engine = libchinese_core::Engine::new(model, parser);
        engine.set_emoji_dict(emoji_dict);
        Self {
            inner: Arc::new(engine),
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
    ///  - lexicon.fst + lexicon.dat        (mmap lexicon)
    ///  - word_bigram.dat + word_bigram_words.fst  (mmap word bigram)
    ///  - userdict.redb                    (persistent user dictionary)
    pub fn from_data_dir<P: AsRef<std::path::Path>>(data_dir: P) -> Result<Self, Box<dyn Error>> {
        let data_dir = data_dir.as_ref();

        // Load lexicon (mmap)
        let fst_path = data_dir.join("lexicon.fst");
        let dat_path = data_dir.join("lexicon.dat");
        let lexicon = Lexicon::load(&fst_path, &dat_path)
            .map_err(|e| format!("failed to load lexicon from {:?}: {}", data_dir, e))?;

        // Load word bigram (mmap)
        let wb_dat_path = data_dir.join("word_bigram.dat");
        let wb_fst_path = data_dir.join("word_bigram_words.fst");
        let word_bigram = WordBigram::load(&wb_dat_path, &wb_fst_path)
            .map_err(|e| format!("failed to load word_bigram from {:?}: {}", data_dir, e))?;

        // Userdict: `LIBCHINESE_USERDICT` override (tests), else ~/.pinyin/userdict.redb
        let userdict = {
            let ud_path = if let Ok(p) = std::env::var("LIBCHINESE_USERDICT") {
                std::path::PathBuf::from(p)
            } else {
                let home = std::env::var("HOME")
                    .or_else(|_| std::env::var("USERPROFILE"))
                    .unwrap_or_else(|_| ".".to_string());
                std::path::PathBuf::from(home)
                    .join(".pinyin")
                    .join("userdict.redb")
            };

            if let Some(parent) = ud_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            UserDict::new(&ud_path)?
        };

        let model = Model::new(
            lexicon,
            word_bigram,
            userdict,
            libchinese_core::Config::default(),
        );

        // Aux tables may live beside the lexicon (installed simplified/), in the
        // parent data/ dir, or in the source-tree grandparent (converted/simplified
        // → data/). Also check the user-local share path so English can be
        // installed without writing to /usr.
        let mut aux_dirs = vec![data_dir.to_path_buf()];
        if let Some(parent) = data_dir.parent() {
            aux_dirs.push(parent.to_path_buf());
            if let Some(grandparent) = parent.parent() {
                aux_dirs.push(grandparent.to_path_buf());
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let local = std::path::PathBuf::from(home).join(".local/share/libpinyin/data");
            aux_dirs.push(local.join("simplified"));
            aux_dirs.push(local.join("traditional"));
            aux_dirs.push(local);
        }
        // Dedup while preserving order
        {
            let mut seen = std::collections::HashSet::new();
            aux_dirs.retain(|d| seen.insert(d.clone()));
        }

        let mut core = {
            let parser = Parser::with_syllables(PINYIN_SYLLABLES);
            libchinese_core::Engine::new(model, parser)
        };

        let mut emoji_loaded = false;
        for dir in &aux_dirs {
            let emoji_path = dir.join("emoji.table");
            if emoji_path.exists() {
                if let Ok(emoji_dict) = EmojiDict::from_file(&emoji_path) {
                    log::info!("Loaded emoji dictionary with {} entries", emoji_dict.len());
                    core.set_emoji_dict(emoji_dict);
                    emoji_loaded = true;
                    break;
                }
            }
        }
        if !emoji_loaded {
            log::warn!(
                "emoji.table not found beside {:?}; emoji candidates will be unavailable",
                data_dir
            );
        }
        let mut english_loaded = false;
        for dir in &aux_dirs {
            let english_path = dir.join("english.wordlist");
            if english_path.exists() {
                if let Ok(english_dict) = EnglishDict::from_file(&english_path) {
                    log::info!(
                        "Loaded English dictionary with {} entries from {:?}",
                        english_dict.len(),
                        english_path
                    );
                    core.set_english_dict(english_dict);
                    english_loaded = true;
                    break;
                }
            }
        }
        if !english_loaded {
            log::warn!(
                "english.wordlist not found beside {:?}; English mixed-input will be unavailable",
                data_dir
            );
        }

        let engine = Self {
            inner: Arc::new(core),
        };

        // Load addon dictionaries if available
        let addon_dir = data_dir.join("addon");
        if addon_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&addon_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let fst_path = path.join("lexicon.fst");
                        let dat_path = path.join("lexicon.dat");
                        if fst_path.exists() && dat_path.exists() {
                            if let Ok(lexicon) = Lexicon::load(&fst_path, &dat_path) {
                                let name = path
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                log::info!("Loaded addon dictionary: {}", name);
                                engine.add_addon(name, lexicon, false);
                            }
                        }
                    }
                }
            }
        }

        Ok(engine)
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
    pub fn clear_cache(&self) {
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

    /// Forget a phrase (remove from user dict + mask from candidates).
    pub fn forget_phrase(&self, phrase: &str) {
        self.inner.forget_phrase(phrase);
    }

    /// Get reference to the user dictionary for learning.
    ///
    /// Provides access to user-learned data including user bigrams
    /// for personalized predictions.
    pub fn userdict(&self) -> &UserDict {
        self.inner.userdict()
    }

    /// Get reference to the configuration.
    pub fn config(&self) -> std::cell::Ref<'_, libchinese_core::Config> {
        self.inner.config()
    }

    /// Get mutable reference to the configuration.
    pub fn config_mut(&self) -> std::cell::RefMut<'_, libchinese_core::Config> {
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
    /// 6. Emoji candidate appending (if enabled)
    pub fn input(&self, input: &str) -> Vec<Candidate> {
        self.inner.input(input)
    }

    /// Enable or disable emoji candidates in results.
    pub fn set_emoji_enabled(&self, enabled: bool) {
        self.inner.set_emoji_enabled(enabled);
    }

    /// Returns whether emoji candidates are enabled.
    pub fn emoji_enabled(&self) -> bool {
        self.inner.emoji_enabled()
    }

    /// Enable or disable English word candidates (混输).
    pub fn set_english_enabled(&self, enabled: bool) {
        self.inner.set_english_enabled(enabled);
    }

    pub fn english_enabled(&self) -> bool {
        self.inner.english_enabled()
    }

    /// Enable or disable fuzzy AMB / corrections / incomplete in segmentation.
    pub fn set_allow_fuzzy(&self, enabled: bool) {
        self.inner.set_allow_fuzzy(enabled);
    }

    pub fn allow_fuzzy(&self) -> bool {
        self.inner.allow_fuzzy()
    }

    /// Prefer shorter candidates when scores are close.
    pub fn set_sort_by_pinyin_length(&self, enabled: bool) {
        self.inner.set_sort_by_pinyin_length(enabled);
    }

    /// Emit best multi-token sentence into the candidate list (default: true).
    /// Enables long inputs like `woshinuoweiren` → 我是挪威人.
    pub fn set_emit_composed_sentences(&self, enabled: bool) {
        self.inner.set_emit_composed_sentences(enabled);
    }

    pub fn emit_composed_sentences(&self) -> bool {
        self.inner.emit_composed_sentences()
    }

    /// Candidate ordering: matched pinyin span (default) or Chinese phrase length
    /// (closer top-5 overlap with system libpinyin).
    pub fn set_candidate_rank_mode(&self, mode: libchinese_core::CandidateRankMode) {
        self.inner.set_candidate_rank_mode(mode);
    }

    pub fn candidate_rank_mode(&self) -> libchinese_core::CandidateRankMode {
        self.inner.candidate_rank_mode()
    }

    /// `BestUx` (default) vs `LibpinyinCompat` (C++ candidate-bar parity).
    pub fn set_profile(&self, profile: libchinese_core::ImeProfile) {
        self.inner.set_profile(profile);
    }

    pub fn profile(&self) -> libchinese_core::ImeProfile {
        self.inner.profile()
    }

    /// Best full-coverage sentence (works even if composed sentences are not
    /// folded into the candidate list).
    pub fn best_sentence(&self, input: &str) -> Option<String> {
        self.inner.best_sentence(input)
    }

    /// Double-pinyin scheme (`Microsoft`, `ZiRanMa`, …) or `None` for full pinyin.
    pub fn set_double_pinyin_scheme(&self, scheme: Option<String>) {
        self.inner.set_double_pinyin_scheme(scheme);
    }

    /// Swap the lexicon at runtime (e.g., simplified ↔ traditional). Clears cache.
    pub fn swap_lexicon(&self, lexicon: Lexicon) {
        self.inner.swap_lexicon(lexicon);
    }

    /// Swap word bigrams with the lexicon on 简/繁 switch.
    pub fn swap_word_bigram(&self, word_bigram: WordBigram) {
        self.inner.swap_word_bigram(word_bigram);
    }

    /// Add an addon lexicon. Returns its index.
    pub fn add_addon(&self, name: String, lexicon: Lexicon, enabled: bool) -> usize {
        self.inner.add_addon(name, lexicon, enabled)
    }

    /// Enable or disable an addon lexicon by name.
    pub fn set_addon_enabled(&self, name: &str, enabled: bool) {
        self.inner.set_addon_enabled(name, enabled);
    }

    /// Get list of addon names and their enabled state.
    pub fn addon_list(&self) -> Vec<(String, bool)> {
        self.inner.addon_list()
    }
}

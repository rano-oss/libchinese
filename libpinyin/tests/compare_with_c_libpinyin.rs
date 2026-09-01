//! Integration test: compare Rust parser / candidates with system libpinyin C library.
//!
//! Uses runtime dlopen (via libloading) to load the system libpinyin shared library,
//! so no devel package or unversioned .so symlink is needed.
//!
//! Requirements:
//! - System libpinyin installed (libpinyin.so.15 + data in /usr/lib64/libpinyin/data)
//! - Run with: cargo test --test compare_with_c_libpinyin -- --ignored --nocapture
//!
//! The tests are #[ignore]d by default since they require the system library.
//!
//! # Intentional gaps (not regressions)
//! - Rust may surface English / emoji mixed-input extras; C libpinyin does not.
//!   Candidate compares disable those extras and only score phonetic overlap.
//! - Ranking / n-best tie-breaks differ; we assert top-1 agreement and top-N set
//!   overlap, not full list identity.
//! - Incomplete single-letter buffers often yield zero C candidates; those are
//!   skipped in the candidate harness (still covered by segmentation tests).

mod common;

use common::{data_dir, CLibPinyin};
use libpinyin::{Parser, PINYIN_SYLLABLES};

const C_USER_DIR: &str = "/tmp/libpinyin_test_user";

/// Create Rust parser with all standard syllables.
fn rust_parser() -> Parser {
    Parser::with_syllables(PINYIN_SYLLABLES)
}

/// Extract syllable texts from Rust parser result.
fn rust_parse(parser: &Parser, input: &str) -> Vec<String> {
    let seg = parser.segment_best(input, false);
    seg.into_iter().map(|s| s.text).collect()
}

// ============================================================================
// Test vectors: comprehensive pinyin inputs
// ============================================================================

const TEST_INPUTS: &[&str] = &[
    // Simple words
    "nihao",
    "zhongguo",
    "xiexie",
    "zaijian",
    "pengyou",
    "xuesheng",
    "laoshi",
    "tongxue",
    "daxue",
    "zhongwen",
    // Common phrases
    "woaini",
    "ninhao",
    "duibuqi",
    "meiguanxi",
    "xiawu",
    "shangwu",
    "jintian",
    "mingtian",
    "zuotian",
    "xingqi",
    // Retroflexes
    "zhidao",
    "chifan",
    "shuijiao",
    "zhuyi",
    "chufa",
    "shijian",
    "zhaodao",
    "chenggong",
    "shenghuo",
    "zhunbei",
    // Ambiguous segmentation
    "xian",
    "shanghai",
    "changcheng",
    "fangan",
    "xingfu",
    "mingan",
    "yinwei",
    "renmin",
    "qingchu",
    "zhengfu",
    // Nasal finals
    "bangzhu",
    "dengdai",
    "tingshuo",
    "guangming",
    "xiangxin",
    "fangbian",
    "gongzuo",
    "kongqi",
    "zhongyao",
    "dongxi",
    // Three+ syllable phrases
    "zhongguoren",
    "putonghua",
    "diannaoshi",
    "tushuguan",
    "huochezhan",
    "feijichang",
    "yiyuan",
    "gongyuan",
    "chaojihaode",
    "feichanggaoxing",
    // Longer inputs
    "wohenxihuanxuexizhongwen",
    "jintiandetianqihenhaowomenqugongyuanba",
    "zheshiyigebijiaofuzadejuzi",
    "tamenzhengzaichifan",
    "woxiangyaoyibeikafei",
    // Edge cases - single syllables
    "er",
    "a",
    "o",
    "e",
    "ai",
    "ei",
    "ao",
    "ou",
    "an",
    "en",
    "ang",
    "eng",
    // Tricky boundaries (should parse as single syllable)
    "pian",
    "lian",
    "nian",
    "tian",
    "dian",
    "bian",
    "mian",
    "jian",
    "qian",
    "xian",
    // ü syllables
    "lv",
    "nv",
    "lve",
    "nve",
    "ju",
    "qu",
    "xu",
    "yu",
    "yuan",
    "yue",
    // Complex initials with medials
    "zhuangyan",
    "chuangzao",
    "shuangshou",
    "guangchang",
    "zhuangjia",
    "chuangkou",
    "shuangfang",
    "guanggao",
    "zhuanbian",
    "chuangye",
    // Common sentences (continuous pinyin)
    "woshizhongguoren",
    "tamenshixuesheng",
    "zhegeshiqinghenzhongyao",
    "qingwennizainarli",
    "womenmingtianjiandao",
    // Four-character idioms
    "yixinyiyi",
    "qianbianyihua",
    "manmantuntan",
    "rerenaimu",
    "ziyouzizhai",
    // Words with similar initials
    "zhizao",
    "chichang",
    "shishi",
    "riren",
    "zizi",
    "cici",
    "sisi",
    // h/f confusion territory
    "huafei",
    "feihua",
    "hufa",
    "fahu",
    "haofen",
    "fenhao",
    // n/l confusion territory
    "nuli",
    "lunu",
    "nali",
    "lina",
    "nianliang",
    "liannian",
    // Additional complex cases
    "dianhuahaoma",
    "huanying",
    "gonggongqiche",
    "zhonghuarenmingongheguo",
    "kexuejishu",
    "jingjifarhan",
    "wenhuayichan",
    "ziranhuanjing",
    "shehuidazhuyi",
    "gaodengjiaoyu",
    // --- Extended test set: daily vocabulary ---
    "diannao",
    "shouji",
    "yinyue",
    "dianying",
    "dianshi",
    "yundong",
    "zuqiu",
    "lanqiu",
    "youyong",
    "paobu",
    "lvxing",
    "feiji",
    "huoche",
    "gongjiaoche",
    "ditie",
    "chuzuche",
    "zixingche",
    "qiche",
    "jiaotong",
    "yiyuan",
    "yisheng",
    "hushi",
    "yaofang",
    "ganmao",
    "fashao",
    "kesou",
    "touteng",
    "jiankang",
    "shenti",
    "shuiguo",
    "pingguo",
    "xiangjiao",
    "putao",
    "xigua",
    "caomei",
    "shucai",
    "xihongshi",
    "huanggua",
    "tudou",
    "qiezi",
    "baicai",
    "luobo",
    "yumi",
    "dami",
    "miantiao",
    "jiaozi",
    "baozi",
    "mantou",
    "mifan",
    "chaofan",
    "huoguo",
    "kaoya",
    "tangcu",
    "doufu",
    // --- Weather and nature ---
    "tianqi",
    "qingtian",
    "yintian",
    "xiayu",
    "xiaxue",
    "guafeng",
    "taileng",
    "taire",
    "wendu",
    "chuntiian",
    "xiatian",
    "qiutian",
    "dongtian",
    "taiyang",
    "yueliang",
    "xingxing",
    "dahai",
    "gaoshanliushui",
    "huakaifugui",
    "shanqingshuixiu",
    "biyuntiankong",
    // --- Education and work ---
    "xuexiao",
    "zhongxue",
    "xiaoxue",
    "youeryuan",
    "jiaoshou",
    "kecheng",
    "zuoye",
    "kaoshi",
    "chengji",
    "biye",
    "gongsi",
    "jingli",
    "tongshi",
    "huiyi",
    "xiangmu",
    "gongzi",
    "shangban",
    "xiaban",
    "jiaban",
    "cizhi",
    "mianshi",
    "jianli",
    "zhaopin",
    "zhiyuan",
    // --- Family and relationships ---
    "baba",
    "mama",
    "gege",
    "jiejie",
    "didi",
    "meimei",
    "yeye",
    "nainai",
    "waigong",
    "waipo",
    "erzi",
    "nver",
    "zhangfu",
    "qizi",
    "pengyou",
    "tongxue",
    "linju",
    "qinqi",
    "jiaren",
    "haizi",
    // --- Directions and locations ---
    "dongbian",
    "xibian",
    "nanbian",
    "beifang",
    "zuobian",
    "youbian",
    "shangmian",
    "xiamian",
    "qianmian",
    "houmian",
    "limian",
    "waimian",
    "pangbian",
    "fujin",
    "duimian",
    "zhongjian",
    // --- Colors ---
    "hongse",
    "huangse",
    "lanse",
    "lvse",
    "baise",
    "heise",
    "zise",
    "chengse",
    "fenhongse",
    "huise",
    // --- Numbers and time ---
    "yibaiyishi",
    "liangqiansan",
    "wuwanliuqian",
    "zuoshang",
    "zhongwu",
    "xiawu",
    "wanshang",
    "bandian",
    "yike",
    "liangtianhou",
    "xiagexingqi",
    "shanggeyue",
    "mingnian",
    "qunian",
    "hounnian",
    // --- Emotions and descriptions ---
    "gaoxing",
    "nanguo",
    "shengqi",
    "haipa",
    "danxin",
    "jidong",
    "wuliao",
    "youqu",
    "piaoliangde",
    "congmingde",
    "qinlaode",
    "yonggande",
    "renzhende",
    "rexinde",
    "youhaode",
    "nenggan",
    // --- Actions and verbs ---
    "chifan",
    "heshui",
    "shuijiao",
    "qichuang",
    "xilian",
    "shuaya",
    "chuanyi",
    "shangxue",
    "fangxue",
    "zuofan",
    "xiyifu",
    "dasaoweisheng",
    "kandianshe",
    "tingdianhua",
    "shangwang",
    "liaotian",
    "sanbu",
    "guangjie",
    "maicai",
    "zuoye",
    // --- Complex multi-syllable ---
    "zhonghuarenmingongheguo",
    "zhongguogongchandang",
    "shehuizhuyihexinjiazhi",
    "gaigekaitfang",
    "kexuefazhan",
    "hexieshehu",
    "yidaiyilu",
    "renleimingyungongtongti",
    "quanmianxiaokang",
    "minzufuxing",
    "chuangxinqudong",
    "lvsefarhan",
    "kaifanggongxiang",
    "tongchouguihua",
    "quanmianshenhuagaige",
    // --- Idioms and chengyu ---
    "yishierzhong",
    "sanxingwuri",
    "simianbafang",
    "wuyanliuse",
    "liuliushunshun",
    "qishangshibaxia",
    "jiuouniuyimao",
    "shiquanshimei",
    "bailiwutaihai",
    "qianfangbaiqi",
    "wanzhongxuyi",
    "yilaotongyongyi",
    "bukengyibusheng",
    "yibujiyifa",
    "congshantairliu",
    "baitoubulaixin",
    "huanshanhuanshui",
    "fengyutongu",
    "qunceqiunli",
    "zhongyanshuijin",
    "mangrenmouxiang",
    "huashetianzhu",
    "longtenghuodiao",
    "huxiashengwei",
    // --- Technology and internet ---
    "hulianwang",
    "rengongzhineng",
    "dashuju",
    "yunyunsuan",
    "wulianwang",
    "qukuailain",
    "xunixianshi",
    "zengqiangxianshi",
    "jiqixuexi",
    "shenduxuexi",
    "ziranyuyanchuli",
    "tuxiangshibie",
    "yuyinshibie",
    "zidongjiashi",
    "zhihuichengshi",
    "dianzishangwu",
    "yidongzhifu",
    "shejiaomeiti",
    "duanshipin",
    "zhibo",
    // --- Geography and places ---
    "beijing",
    "shanghai",
    "guangzhou",
    "shenzhen",
    "chengdu",
    "hangzhou",
    "nanjing",
    "wuhan",
    "xian",
    "chongqing",
    "tianjin",
    "suzhou",
    "qingdao",
    "dalian",
    "xiamen",
    "kunming",
    "guilin",
    "haerbin",
    "lasa",
    "huhehaote",
    // --- Sentences (continuous pinyin) ---
    "jintiandedianqizhenbucuo",
    "womenyiqilaichizhongguofan",
    "nimingtianyoushijianma",
    "woxiangquwaimianzouyizou",
    "tadezhongwenshuodehenhao",
    "zhejiandongxiduoshaoqian",
    "qingwencesouozainali",
    "woxuyaoyigebangmang",
    "nikebukeiyibangwogeang",
    "tamenzhengzaikaihui",
    "womingtianzaoshangchumen",
    "qingbangjiaowodianshangban",
    "zhegecaizuodefeichanghaochi",
    "womenxueyuanyougeshitang",
    "tabuxiaoxinshuaidaole",
    "jintiandezuoyetaiduole",
    "wogangcaijiandaotalingju",
    "mingtianyoubukaoshi",
    "zhelidefengjinghenpiaoliang",
    "tageiwomaideyishanghenhaokanyiya",
    // --- More ambiguous segmentations ---
    "pianyiru",
    "tiananmen",
    "xianggang",
    "changanjie",
    "tongjiang",
    "fangfaxue",
    "guanxifue",
    "renbianma",
    "jiangdanpin",
    "xiangqishu",
    "rongxinxia",
    "changqiang",
    "zhengtiyiji",
    "chuanjiao",
    "zhuangshi",
    "guangbo",
    "shuangchong",
    "chuangtou",
    "zhuangkuang",
    "guangrong",
    // --- Tone sandhi territory (same syllable repeated) ---
    "maimai",
    "kankan",
    "xiexie",
    "shishi",
    "tingting",
    "xiangxiang",
    "wangwang",
    "changchang",
    "duoduo",
    "manman",
    // --- Words ending in -ng vs -n ---
    "fangxiang",
    "changjiang",
    "zhengzai",
    "denghou",
    "fengjian",
    "zhongxin",
    "gangcai",
    "gongdao",
    "songhua",
    "longfeng",
    "jiangnan",
    "guangmang",
    "qiangzhuang",
    "mengxiang",
    "lingdao",
    "mingbai",
    "ningke",
    "dingji",
    "bingxiang",
    "yingyang",
    // --- All initials exercised ---
    "baoyu",
    "paifang",
    "maotai",
    "feidan",
    "daying",
    "taishan",
    "nahai",
    "laobaixing",
    "gaosu",
    "kaifang",
    "haiguan",
    "jisuan",
    "qiyue",
    "xinwen",
    "zhuchi",
    "chukou",
    "shuohua",
    "renlei",
    "zuijin",
    "cuowu",
    "suiran",
    "yuedu",
    "wenti",
    // --- Rare/unusual combinations ---
    "cengzeng",
    "senlin",
    "zhuozhuang",
    "chuochuobuyu",
    "suosui",
    "nuonuo",
    "luoluo",
    "guaguajiao",
    "kuakua",
    "huahuan",
    "shuashua",
    "zhuazhua",
    "chuachuan",
    "rourou",
    "gougou",
    "moumou",
    "loulou",
    "doudou",
    "koulou",
    "zouzou",
    // Incomplete / progressive typing (common IME buffers)
    "n",
    "ni",
    "nih",
    "niha",
    "z",
    "zh",
    "zho",
    "zhon",
    "zhong",
    "zhongg",
    "zhonggu",
    "sh",
    "shi",
    "x",
    "xi",
    "xin",
    "b",
    "be",
    "bei",
    "beij",
    "beiji",
    "beijin",
    // Mixed syllable boundaries users often mistype progressively
    "xiaina",
    "fangand",
    "minganed",
];

#[test]
#[ignore]
fn compare_segmentation_with_c_libpinyin() {
    let c_lib = match CLibPinyin::new(C_USER_DIR) {
        Some(lib) => lib,
        None => {
            eprintln!(
                "SKIP: Could not initialize system libpinyin \
                 (not installed or data missing at /usr/lib64/libpinyin/data)"
            );
            return;
        }
    };

    let parser = rust_parser();

    let mut total = 0;
    let mut matches = 0;
    let mut mismatches: Vec<(String, Vec<String>, Vec<String>, usize)> = Vec::new();

    for &input in TEST_INPUTS {
        let (c_parsed_len, c_syllables) = c_lib.parse(input);
        let rust_syllables = rust_parse(&parser, input);

        total += 1;

        // Normalize: C libpinyin may return syllables with tone numbers or different casing.
        let c_normalized: Vec<String> = c_syllables
            .iter()
            .map(|s| {
                s.trim_end_matches(|c: char| c.is_ascii_digit())
                    .to_lowercase()
            })
            .collect();
        let rust_normalized: Vec<String> =
            rust_syllables.iter().map(|s| s.to_lowercase()).collect();

        if c_normalized == rust_normalized {
            matches += 1;
        } else {
            mismatches.push((
                input.to_string(),
                c_normalized,
                rust_normalized,
                c_parsed_len,
            ));
        }
    }

    println!("\n=== Pinyin Parser Comparison: C libpinyin vs Rust ===");
    println!("Total inputs:  {}", total);
    println!(
        "Matching:      {} ({:.1}%)",
        matches,
        100.0 * matches as f64 / total as f64
    );
    println!("Mismatches:    {}", mismatches.len());

    if !mismatches.is_empty() {
        println!("\n--- Mismatches ---");
        for (input, c_result, rust_result, c_parsed_len) in &mismatches {
            println!(
                "  Input: {:40} C(len={}): {:?}",
                input, c_parsed_len, c_result
            );
            println!("  {:42} Rust:     {:?}", "", rust_result);
        }
    }

    // Expect high agreement. Allow some divergence for scoring/tie-breaking differences.
    let match_ratio = matches as f64 / total as f64;
    assert!(
        match_ratio >= 0.70,
        "Match ratio too low: {:.1}% (expected >= 70%). \
         Rust parser diverges too much from C libpinyin.",
        match_ratio * 100.0
    );
}

/// Test that parsed length (bytes consumed) agrees between implementations.
#[test]
#[ignore]
fn compare_parsed_length() {
    let c_lib = match CLibPinyin::new(C_USER_DIR) {
        Some(lib) => lib,
        None => {
            eprintln!("SKIP: Could not initialize system libpinyin");
            return;
        }
    };

    let parser = rust_parser();
    let mut length_mismatches = Vec::new();

    for &input in TEST_INPUTS {
        let (c_parsed_len, _) = c_lib.parse(input);
        let rust_seg = parser.segment_best(input, false);
        // Rust parsed length = sum of syllable text lengths
        let rust_parsed_len: usize = rust_seg.iter().map(|s| s.text.len()).sum();

        if c_parsed_len != rust_parsed_len {
            length_mismatches.push((input, c_parsed_len, rust_parsed_len));
        }
    }

    println!("\n=== Parsed Length Comparison ===");
    println!("Total inputs: {}", TEST_INPUTS.len());
    println!("Length mismatches: {}", length_mismatches.len());

    if !length_mismatches.is_empty() {
        println!("\n--- Parsed Length Mismatches ---");
        for (input, c_len, rust_len) in &length_mismatches {
            println!("  {:40} C: {:2}  Rust: {:2}", input, c_len, rust_len);
        }
    }

    let match_ratio = 1.0 - length_mismatches.len() as f64 / TEST_INPUTS.len() as f64;
    assert!(
        match_ratio >= 0.80,
        "Parsed length match ratio too low: {:.1}%",
        match_ratio * 100.0
    );
}

/// Every standard pinyin syllable should be recognized by both implementations.
#[test]
#[ignore]
fn compare_single_syllable_recognition() {
    let c_lib = match CLibPinyin::new(C_USER_DIR) {
        Some(lib) => lib,
        None => {
            eprintln!("SKIP: Could not initialize system libpinyin");
            return;
        }
    };

    let parser = rust_parser();

    let mut c_unrecognized = Vec::new();
    let mut rust_unrecognized = Vec::new();
    let mut both_recognized = 0;

    for &syllable in PINYIN_SYLLABLES {
        let (c_parsed_len, c_syls) = c_lib.parse(syllable);
        let rust_syls = rust_parse(&parser, syllable);

        let c_ok = c_parsed_len == syllable.len() && c_syls.len() == 1;
        let rust_ok = rust_syls.len() == 1 && rust_syls[0] == syllable;

        if c_ok && rust_ok {
            both_recognized += 1;
        } else if !c_ok {
            c_unrecognized.push(syllable);
        } else if !rust_ok {
            rust_unrecognized.push(syllable);
        }
    }

    println!("\n=== Single Syllable Recognition ===");
    println!("Total syllables: {}", PINYIN_SYLLABLES.len());
    println!("Both recognize:  {}", both_recognized);
    if !c_unrecognized.is_empty() {
        println!(
            "C unrecognized ({}):\n  {:?}",
            c_unrecognized.len(),
            &c_unrecognized[..c_unrecognized.len().min(20)]
        );
    }
    if !rust_unrecognized.is_empty() {
        println!(
            "Rust unrecognized ({}):\n  {:?}",
            rust_unrecognized.len(),
            &rust_unrecognized[..rust_unrecognized.len().min(20)]
        );
    }

    assert!(
        rust_unrecognized.len() <= 5,
        "Rust fails to recognize too many standard syllables: {:?}",
        rust_unrecognized
    );
}

/// Phrases where C usually returns a non-empty candidate list (skip bare initials).
const CANDIDATE_COMPARE_INPUTS: &[&str] = &[
    "nihao",
    "zhongguo",
    "xiexie",
    "beijing",
    "shanghai",
    "xian",
    "women",
    "zhongguoren",
    "putonghua",
    "diannao",
    "kafei",
    "xihuan",
    "pengyou",
    "xuesheng",
    "gongzuo",
    "shijian",
    "mingtian",
    "jintian",
    "nihaoma",
    "woaini",
    "chenggong",
    "zhunbei",
    "bangzhu",
    "fangbian",
    "beijin",
    "niha",
    "zhonggu",
];

/// Compare top-N phonetic candidate prefixes with C libpinyin.
#[test]
#[ignore]
fn compare_top_candidates_with_c_libpinyin() {
    let c_lib = match CLibPinyin::new(C_USER_DIR) {
        Some(lib) => lib,
        None => {
            eprintln!("SKIP: Could not initialize system libpinyin");
            return;
        }
    };

    let engine = libpinyin::Engine::from_data_dir(data_dir()).expect("rust engine");
    // Keep the comparison phonetic-only; English/emoji are intentional Rust extras.
    engine.set_english_enabled(false);
    engine.set_emoji_enabled(false);

    const TOP_N: usize = 5;
    let mut compared = 0usize;
    let mut top1_matches = 0usize;
    let mut overlap_sum = 0.0f64;
    let mut skipped_empty_c = 0usize;
    let mut top1_mismatches: Vec<(String, String, String)> = Vec::new();

    for &input in CANDIDATE_COMPARE_INPUTS {
        let c_top = c_lib.candidates(input, TOP_N);
        if c_top.is_empty() {
            skipped_empty_c += 1;
            continue;
        }
        let rust_top: Vec<String> = engine
            .input(input)
            .into_iter()
            .take(TOP_N)
            .map(|c| c.text)
            .collect();
        if rust_top.is_empty() {
            top1_mismatches.push((input.to_string(), c_top[0].clone(), String::new()));
            compared += 1;
            continue;
        }

        compared += 1;
        if rust_top[0] == c_top[0] {
            top1_matches += 1;
        } else {
            top1_mismatches.push((input.to_string(), c_top[0].clone(), rust_top[0].clone()));
        }

        let c_set: std::collections::HashSet<&str> = c_top.iter().map(|s| s.as_str()).collect();
        let overlap = rust_top
            .iter()
            .filter(|t| c_set.contains(t.as_str()))
            .count();
        overlap_sum += overlap as f64 / TOP_N as f64;
    }

    let top1_ratio = if compared == 0 {
        0.0
    } else {
        top1_matches as f64 / compared as f64
    };
    let avg_overlap = if compared == 0 {
        0.0
    } else {
        overlap_sum / compared as f64
    };

    println!("\n=== Top-{} Candidate Comparison (phonetic only) ===", TOP_N);
    println!("Inputs compared:     {}", compared);
    println!("Skipped (empty C):   {}", skipped_empty_c);
    println!(
        "Top-1 matches:       {} ({:.1}%)",
        top1_matches,
        100.0 * top1_ratio
    );
    println!("Avg top-N overlap:   {:.1}%", 100.0 * avg_overlap);
    if !top1_mismatches.is_empty() {
        println!("\n--- Top-1 mismatches (first 20) ---");
        for (input, c, rust) in top1_mismatches.iter().take(20) {
            println!("  {:16} C: {:8} Rust: {}", input, c, rust);
        }
    }

    assert!(
        compared >= 10,
        "too few candidate comparisons ({}); C data missing?",
        compared
    );
    assert!(
        top1_ratio >= 0.40,
        "top-1 match ratio too low: {:.1}% (expected >= 40%)",
        top1_ratio * 100.0
    );
    assert!(
        avg_overlap >= 0.20,
        "average top-N overlap too low: {:.1}% (expected >= 20%)",
        avg_overlap * 100.0
    );
}

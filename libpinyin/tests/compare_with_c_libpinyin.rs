//! Integration test: compare Rust parser segmentation with system libpinyin C library.
//!
//! Uses runtime dlopen (via libloading) to load the system libpinyin shared library,
//! so no devel package or unversioned .so symlink is needed.
//!
//! Requirements:
//! - System libpinyin installed (libpinyin.so.15 + data in /usr/lib64/libpinyin/data)
//! - Run with: cargo test --test compare_with_c_libpinyin -- --ignored --nocapture
//!
//! The tests are #[ignore]d by default since they require the system library.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use libloading::Library;
use libpinyin::{Parser, PINYIN_SYLLABLES};

// --- Opaque types for C libpinyin ---

#[repr(C)]
struct PinyinContext {
    _private: [u8; 0],
}

#[repr(C)]
struct PinyinInstance {
    _private: [u8; 0],
}

#[repr(C)]
struct ChewingKey {
    _private: [u8; 0],
}

/// Opaque type for ChewingKeyRest from C libpinyin.
#[repr(C)]
struct ChewingKeyRest {
    _private: [u8; 0],
}

/// Wrapper around system libpinyin loaded at runtime.
struct CLibPinyin {
    _lib: Library,
    _glib: Library,
    context: *mut PinyinContext,
    instance: *mut PinyinInstance,
    // Function pointers
    fn_parse_more: unsafe extern "C" fn(*mut PinyinInstance, *const c_char) -> usize,
    fn_get_key: unsafe extern "C" fn(*mut PinyinInstance, usize, *mut *mut ChewingKey) -> u8,
    fn_get_string:
        unsafe extern "C" fn(*mut PinyinInstance, *mut ChewingKey, *mut *mut c_char) -> u8,
    fn_get_key_rest:
        unsafe extern "C" fn(*mut PinyinInstance, usize, *mut *mut ChewingKeyRest) -> u8,
    fn_get_key_rest_positions:
        unsafe extern "C" fn(*mut PinyinInstance, *mut ChewingKeyRest, *mut u16, *mut u16) -> u8,
    fn_free_instance: unsafe extern "C" fn(*mut PinyinInstance),
    fn_fini: unsafe extern "C" fn(*mut PinyinContext),
    fn_g_free: unsafe extern "C" fn(*mut std::ffi::c_void),
}

impl CLibPinyin {
    fn new() -> Option<Self> {
        unsafe {
            // Load libraries
            let lib = Library::new("libpinyin.so.15").ok()?;
            let glib = Library::new("libglib-2.0.so.0").ok()?;

            // Resolve and copy function pointers (must copy before moving lib)
            type FnInit = unsafe extern "C" fn(*const c_char, *const c_char) -> *mut PinyinContext;
            type FnAlloc = unsafe extern "C" fn(*mut PinyinContext) -> *mut PinyinInstance;
            type FnParseMore = unsafe extern "C" fn(*mut PinyinInstance, *const c_char) -> usize;
            type FnGetKey =
                unsafe extern "C" fn(*mut PinyinInstance, usize, *mut *mut ChewingKey) -> u8;
            type FnGetString =
                unsafe extern "C" fn(*mut PinyinInstance, *mut ChewingKey, *mut *mut c_char) -> u8;
            type FnGetKeyRest =
                unsafe extern "C" fn(*mut PinyinInstance, usize, *mut *mut ChewingKeyRest) -> u8;
            type FnGetKeyRestPositions = unsafe extern "C" fn(
                *mut PinyinInstance,
                *mut ChewingKeyRest,
                *mut u16,
                *mut u16,
            ) -> u8;
            type FnFreeInstance = unsafe extern "C" fn(*mut PinyinInstance);
            type FnFini = unsafe extern "C" fn(*mut PinyinContext);
            type FnGFree = unsafe extern "C" fn(*mut std::ffi::c_void);

            let fn_init = *lib.get::<FnInit>(b"pinyin_init").ok()?;
            let fn_alloc = *lib.get::<FnAlloc>(b"pinyin_alloc_instance").ok()?;
            let fn_parse_more = *lib
                .get::<FnParseMore>(b"pinyin_parse_more_full_pinyins")
                .ok()?;
            let fn_get_key = *lib.get::<FnGetKey>(b"pinyin_get_pinyin_key").ok()?;
            let fn_get_string = *lib.get::<FnGetString>(b"pinyin_get_pinyin_string").ok()?;
            let fn_get_key_rest = *lib
                .get::<FnGetKeyRest>(b"pinyin_get_pinyin_key_rest")
                .ok()?;
            let fn_get_key_rest_positions = *lib
                .get::<FnGetKeyRestPositions>(b"pinyin_get_pinyin_key_rest_positions")
                .ok()?;
            let fn_free_instance = *lib.get::<FnFreeInstance>(b"pinyin_free_instance").ok()?;
            let fn_fini = *lib.get::<FnFini>(b"pinyin_fini").ok()?;
            let fn_g_free = *glib.get::<FnGFree>(b"g_free").ok()?;

            // Initialize context
            let system_dir = CString::new("/usr/lib64/libpinyin/data").ok()?;
            let user_dir = CString::new("/tmp/libpinyin_test_user").ok()?;
            let _ = std::fs::create_dir_all("/tmp/libpinyin_test_user");

            let context = fn_init(system_dir.as_ptr(), user_dir.as_ptr());
            if context.is_null() {
                return None;
            }
            let instance = fn_alloc(context);
            if instance.is_null() {
                fn_fini(context);
                return None;
            }

            Some(Self {
                _lib: lib,
                _glib: glib,
                context,
                instance,
                fn_parse_more,
                fn_get_key,
                fn_get_string,
                fn_get_key_rest,
                fn_get_key_rest_positions,
                fn_free_instance,
                fn_fini,
                fn_g_free,
            })
        }
    }

    /// Parse pinyin string, return (parsed_length, syllable_strings).
    fn parse(&self, input: &str) -> (usize, Vec<String>) {
        let c_input = CString::new(input).unwrap();
        unsafe {
            let parsed_len = (self.fn_parse_more)(self.instance, c_input.as_ptr());

            let mut syllables = Vec::new();
            // The offset is a BYTE POSITION in the input, not an array index.
            // We get key_rest to find the end position, then jump to it.
            let mut offset: usize = 0;
            while offset < parsed_len {
                let mut key: *mut ChewingKey = ptr::null_mut();
                let ok = (self.fn_get_key)(self.instance, offset, &mut key);
                if ok == 0 || key.is_null() {
                    // No key at this offset, advance by 1
                    offset += 1;
                    continue;
                }

                // Get the string for this key
                let mut utf8_str: *mut c_char = ptr::null_mut();
                let ok = (self.fn_get_string)(self.instance, key, &mut utf8_str);
                if ok != 0 && !utf8_str.is_null() {
                    let s = CStr::from_ptr(utf8_str).to_string_lossy().into_owned();
                    syllables.push(s);
                    (self.fn_g_free)(utf8_str as *mut std::ffi::c_void);
                }

                // Get key_rest to find end position
                let mut key_rest: *mut ChewingKeyRest = ptr::null_mut();
                let ok = (self.fn_get_key_rest)(self.instance, offset, &mut key_rest);
                if ok != 0 && !key_rest.is_null() {
                    let mut begin: u16 = 0;
                    let mut end: u16 = 0;
                    (self.fn_get_key_rest_positions)(self.instance, key_rest, &mut begin, &mut end);
                    if end as usize > offset {
                        offset = end as usize;
                    } else {
                        offset += 1;
                    }
                } else {
                    offset += 1;
                }
            }

            (parsed_len, syllables)
        }
    }
}

impl Drop for CLibPinyin {
    fn drop(&mut self) {
        unsafe {
            (self.fn_free_instance)(self.instance);
            (self.fn_fini)(self.context);
        }
    }
}

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
];

#[test]
#[ignore]
fn compare_segmentation_with_c_libpinyin() {
    let c_lib = match CLibPinyin::new() {
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
    let c_lib = match CLibPinyin::new() {
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
    let c_lib = match CLibPinyin::new() {
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

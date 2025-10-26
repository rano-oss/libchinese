//! libpinyin crate root
//!
//! This crate provides the pinyin-specific parser, fuzzy utilities and a high-
//! level `Engine` that composes the parser with the shared `libchinese-core`
//! model types.
//!
//! Public API exported here:
//! - `Parser` and `Syllable` from `parser`
//! - `Engine` from `engine`
//! - `FuzzyMap` from `fuzzy`

// Re-export the language-specific modules.
pub mod config;
pub mod double_pinyin;
pub mod engine;
pub mod parser;

// Re-export IME components from core (now at root level, not in ime::)
pub use libchinese_core::{
    Candidate, CandidateList, Composition, Editor, EditorResult, ImeContext, ImeEngine, ImeSession,
    InputBuffer, InputMode, KeyEvent, KeyResult, PhoneticEditor, PunctuationEditor, Segment,
    SuggestionEditor,
};

// Convenience re-exports for common types used by callers.
pub use config::PinyinConfig;
pub use double_pinyin::{get_scheme_data, DoublePinyinScheme, DoublePinyinSchemeData};
pub use engine::{Engine, PINYIN_SYLLABLES};
pub use parser::{Parser, Syllable};

/// Configuration for standard pinyin fuzzy matching rules.
///
/// These rules match upstream libpinyin's fuzzy matching patterns:
/// - Shengmu (initial) confusions: zh/z, ch/c, sh/s, n/l, f/h, r/l, k/g
/// - Yunmu (final) confusions: an/ang, en/eng, in/ing, ian/iang
/// - Corrections: ng/gn, iu/iou, ui/uei, un/uen, ue/ve, v/u, ong/on
/// - Composed syllables: zi/zhi, fan/fang, ben/beng, etc.
pub fn standard_fuzzy_rules() -> Vec<String> {
    let mut rules = Vec::new();

    // Shengmu (initial) fuzzy rules - penalty 1.0
    let shengmu = [
        "c=ch:1.0", "z=zh:1.0", "s=sh:1.0", "l=n:1.0", "f=h:1.0", "l=r:1.0", "k=g:1.0",
    ];
    rules.extend(shengmu.iter().map(|s| s.to_string()));

    // Composed syllable fuzzy rules (z/zh, c/ch, s/sh groups)
    let composed_zh = [
        "zi=zhi:1.0",
        "za=zha:1.0",
        "ze=zhe:1.0",
        "zu=zhu:1.0",
        "zai=zhai:1.0",
        "zei=zhei:1.0",
        "zao=zhao:1.0",
        "zou=zhou:1.0",
        "zan=zhan:1.0",
        "zen=zhen:1.0",
        "zang=zhang:1.0",
        "zeng=zheng:1.0",
        "zong=zhong:1.0",
        "zuan=zhuan:1.0",
        "zun=zhun:1.0",
        "zui=zhui:1.0",
        "zuo=zhuo:1.0",
    ];
    rules.extend(composed_zh.iter().map(|s| s.to_string()));

    let composed_ch = [
        "ci=chi:1.0",
        "ca=cha:1.0",
        "ce=che:1.0",
        "cu=chu:1.0",
        "cai=chai:1.0",
        "cao=chao:1.0",
        "cou=chou:1.0",
        "can=chan:1.0",
        "cen=chen:1.0",
        "cang=chang:1.0",
        "ceng=cheng:1.0",
        "cong=chong:1.0",
        "cuan=chuan:1.0",
        "cun=chun:1.0",
        "cui=chui:1.0",
        "cuo=chuo:1.0",
    ];
    rules.extend(composed_ch.iter().map(|s| s.to_string()));

    let composed_sh = [
        "si=shi:1.0",
        "sa=sha:1.0",
        "se=she:1.0",
        "su=shu:1.0",
        "sai=shai:1.0",
        "sao=shao:1.0",
        "sou=shou:1.0",
        "san=shan:1.0",
        "sen=shen:1.0",
        "sang=shang:1.0",
        "seng=sheng:1.0",
        "song=shong:1.0",
        "suan=shuan:1.0",
        "sun=shun:1.0",
        "sui=shui:1.0",
        "suo=shuo:1.0",
    ];
    rules.extend(composed_sh.iter().map(|s| s.to_string()));

    // Yunmu (final) fuzzy rules - penalty 1.0
    let yunmu = ["an=ang:1.0", "en=eng:1.0", "in=ing:1.0", "ian=iang:1.0"];
    rules.extend(yunmu.iter().map(|s| s.to_string()));

    // Composed syllable rules for an/ang, en/eng, in/ing confusion
    let an_ang = [
        "ban=bang:1.0",
        "pan=pang:1.0",
        "man=mang:1.0",
        "fan=fang:1.0",
        "dan=dang:1.0",
        "tan=tang:1.0",
        "nan=nang:1.0",
        "lan=lang:1.0",
        "gan=gang:1.0",
        "kan=kang:1.0",
        "han=hang:1.0",
        "ran=rang:1.0",
        "zan=zang:1.0",
        "can=cang:1.0",
        "san=sang:1.0",
        "zhan=zhang:1.0",
        "chan=chang:1.0",
        "shan=shang:1.0",
        "yan=yang:1.0",
        "wan=wang:1.0",
    ];
    rules.extend(an_ang.iter().map(|s| s.to_string()));

    let en_eng = [
        "ben=beng:1.0",
        "pen=peng:1.0",
        "men=meng:1.0",
        "fen=feng:1.0",
        "den=deng:1.0",
        "ten=teng:1.0",
        "nen=neng:1.0",
        "len=leng:1.0",
        "gen=geng:1.0",
        "ken=keng:1.0",
        "hen=heng:1.0",
        "ren=reng:1.0",
        "zen=zeng:1.0",
        "cen=ceng:1.0",
        "sen=seng:1.0",
        "zhen=zheng:1.0",
        "chen=cheng:1.0",
        "shen=sheng:1.0",
        "wen=weng:1.0",
    ];
    rules.extend(en_eng.iter().map(|s| s.to_string()));

    let in_ing = [
        "bin=bing:1.0",
        "pin=ping:1.0",
        "min=ming:1.0",
        "din=ding:1.0",
        "tin=ting:1.0",
        "nin=ning:1.0",
        "lin=ling:1.0",
        "jin=jing:1.0",
        "qin=qing:1.0",
        "xin=xing:1.0",
        "yin=ying:1.0",
    ];
    rules.extend(in_ing.iter().map(|s| s.to_string()));

    // Correction rules - penalty 1.5 (less common)
    let corrections = [
        "ng=gn:1.5",
        "ng=mg:1.5",
        "iu=iou:1.5",
        "ui=uei:1.5",
        "un=uen:1.5",
        "ue=ve:1.5",
        "ong=on:1.5",
    ];
    rules.extend(corrections.iter().map(|s| s.to_string()));

    // V/U correction - penalty 2.0 (least common)
    let vu = [
        "ju=jv:2.0",
        "qu=qv:2.0",
        "xu=xv:2.0",
        "yu=yv:2.0",
        "jue=jve:2.0",
        "que=qve:2.0",
        "xue=xve:2.0",
        "yue=yve:2.0",
        "juan=jvan:2.0",
        "quan=qvan:2.0",
        "xuan=xvan:2.0",
        "yuan=yvan:2.0",
        "jun=jvn:2.0",
        "qun=qvn:2.0",
        "xun=xvn:2.0",
        "yun=yvn:2.0",
    ];
    rules.extend(vu.iter().map(|s| s.to_string()));

    rules
}

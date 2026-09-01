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
    Candidate, CandidateList, CandidateRankMode, Composition, Editor, EditorResult, ImeContext,
    ImeEngine, ImeProfile, ImeSession, InputBuffer, InputMode, KeyEvent, KeyResult, PhoneticEditor,
    PunctuationEditor, Segment, SuggestionEditor,
};

// Convenience re-exports for common types used by callers.
pub use config::{
    fuzzy_rules_for_amb_options, pinyin_default_fuzzy_rules, PinyinConfig, IS_PINYIN,
    PINYIN_AMB_ALL, PINYIN_AMB_AN_ANG, PINYIN_AMB_C_CH, PINYIN_AMB_EN_ENG, PINYIN_AMB_F_H,
    PINYIN_AMB_G_K, PINYIN_AMB_IN_ING, PINYIN_AMB_L_N, PINYIN_AMB_L_R, PINYIN_AMB_S_SH,
    PINYIN_AMB_Z_ZH, PINYIN_CORRECT_ALL, PINYIN_CORRECT_GN_NG, PINYIN_CORRECT_IOU_IU,
    PINYIN_CORRECT_MG_NG, PINYIN_CORRECT_ON_ONG, PINYIN_CORRECT_UEI_UI, PINYIN_CORRECT_UEN_UN,
    PINYIN_CORRECT_UE_VE, PINYIN_CORRECT_V_U, PINYIN_INCOMPLETE, PINYIN_OPTIONS_FULL,
    PINYIN_OPTIONS_STRICT, USE_TONE,
};
pub use double_pinyin::{get_scheme_data, DoublePinyinScheme, DoublePinyinSchemeData};
pub use engine::{Engine, PINYIN_SYLLABLES};
pub use parser::{Parser, Syllable};

/// Standard AMB fuzzy rules (no correction pairs — those are `Parser::apply_corrections`).
pub fn standard_fuzzy_rules() -> Vec<String> {
    fuzzy_rules_for_amb_options(PINYIN_AMB_ALL)
}

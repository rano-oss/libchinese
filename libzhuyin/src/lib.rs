//! # libzhuyin
//!
//! Zhuyin/Bopomofo input method engine built on libchinese-core.

pub mod config;
pub mod parser;
pub mod engine;
pub mod fuzzy_presets;

// Re-export IME components from core (now at root level, not in ime::)
pub use libchinese_core::{
    ImeEngine, ImeSession, ImeContext, InputMode, KeyEvent, KeyResult,
    PhoneticEditor, PunctuationEditor, SuggestionEditor, Candidate, CandidateList,
    Composition, Segment, InputBuffer, Editor, EditorResult,
};

pub use config::ZhuyinConfig;
pub use parser::ZhuyinParser;
pub use engine::{Engine, ZHUYIN_SYLLABLES, create_ime_engine_hsu, create_ime_engine_standard, create_ime_engine_eten};
pub use fuzzy_presets::{hsu_fuzzy_rules, standard_fuzzy_rules, eten_fuzzy_rules, no_fuzzy_rules};

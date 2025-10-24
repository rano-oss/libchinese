//! # libzhuyin
//!
//! Zhuyin/Bopomofo input method engine built on libchinese-core.

pub mod config;
pub mod engine;
pub mod fuzzy_presets;
pub mod parser;

// Re-export IME components from core (now at root level, not in ime::)
pub use libchinese_core::{
    Candidate, CandidateList, Composition, Editor, EditorResult, ImeContext, ImeEngine, ImeSession,
    InputBuffer, InputMode, KeyEvent, KeyResult, PhoneticEditor, PunctuationEditor, Segment,
    SuggestionEditor,
};

pub use config::ZhuyinConfig;
pub use engine::{
    create_ime_engine_eten, create_ime_engine_hsu, create_ime_engine_standard, Engine,
    ZHUYIN_SYLLABLES,
};
pub use fuzzy_presets::{eten_fuzzy_rules, hsu_fuzzy_rules, no_fuzzy_rules, standard_fuzzy_rules};
pub use parser::ZhuyinParser;

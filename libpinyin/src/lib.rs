//! libpinyin crate root
//!
//! This crate provides the pinyin-specific parser, fuzzy utilities and a high-
//! level `Engine` that composes the parser with the shared `libchinese-core`
//! model types. The implementation in this workspace is a correctness-first
//! reimplementation; production optimizations (fst-backed lexicon, redb
//! userdb, etc.) are planned in later phases.
//!
//! Public API exported here:
//! - `Parser` and `Syllable` from `parser`
//! - `Engine` from `engine`
//! - `FuzzyMap` from `fuzzy`
//!
//! Example
//!
//! ```no_run
//! // Simple usage sketch:
//! // let parser = libpinyin::Parser::with_syllables(&["ni", "hao"]);
//! // let model = libchinese_core::Model::new(...);
//! // let engine = libpinyin::Engine::new(model, parser);
//! // let candidates = engine.input("nihao");
//! ```

// Re-export the language-specific modules.
pub mod parser;
pub mod engine;
pub mod fuzzy;

// Convenience re-exports for common types used by callers.
pub use engine::Engine;
pub use parser::{Parser, Syllable};
pub use fuzzy::FuzzyMap;

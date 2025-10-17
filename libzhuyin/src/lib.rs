//! # libzhuyin
//!
//! Zhuyin/Bopomofo input method engine built on libchinese-core.

pub mod parser;
pub mod engine;

pub use parser::ZhuyinParser;
pub use engine::Engine;

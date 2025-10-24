//! Generic IME (Input Method Editor) components.
//!
//! This module contains language-agnostic IME building blocks that work with any
//! phonetic parser. These components were originally in libpinyin and are now
//! generalized for reuse across libpinyin, libzhuyin, and other IME implementations.
//!
//! ## Architecture
//!
//! The IME is built from several layers:
//!
//! - **Input Buffer**: Raw character input tracking (e.g., "nihao")
//! - **Composition**: Visual preedit display with segments
//! - **Candidates**: Ranked conversion options with pagination
//! - **Session**: Combined state management across input lifecycle
//! - **Context**: Platform communication interface (UI state)
//! - **Editors**: Pluggable input handlers (phonetic, punctuation, suggestion)
//! - **ImeEngine**: Main coordinator tying everything together
//!
//! ## Usage
//!
//! ```rust,ignore
//! use libchinese_core::ime::{ImeEngine, KeyEvent};
//! use libchinese_core::Engine;
//!
//! // Create backend engine with your parser
//! let engine = Engine::new(model, parser);
//!
//! // Create IME engine
//! let mut ime = ImeEngine::new(engine);
//!
//! // Process keys
//! ime.process_key(KeyEvent::Char('n'));
//! ime.process_key(KeyEvent::Char('i'));
//! ime.process_key(KeyEvent::Space);
//!
//! // Get UI state
//! let context = ime.context();
//! println!("Commit: {}", context.commit_text);
//! ```

pub mod input_buffer;
pub mod composition;
pub mod candidates;
pub mod context;
pub mod session;
pub mod editor;
pub mod engine;

pub use input_buffer::InputBuffer;
pub use composition::{Composition, Segment};
pub use candidates::{Candidate, CandidateList};
pub use context::ImeContext;
pub use session::{ImeSession, InputMode};
pub use editor::{Editor, EditorResult, PhoneticEditor, PunctuationEditor, SuggestionEditor};
pub use engine::{ImeEngine, KeyEvent, KeyResult};

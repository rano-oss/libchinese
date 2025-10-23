//! Editor trait and implementations for different input modes.
//!
//! The editor architecture provides pluggable input handlers for different
//! modes (phonetic, punctuation, suggestions). Each editor implements the
//! `Editor` trait and processes key events in its specific context.

pub mod phonetic;
pub mod punctuation;
pub mod suggestion;

pub use phonetic::PhoneticEditor;
pub use punctuation::PunctuationEditor;
pub use suggestion::SuggestionEditor;

use super::session::ImeSession;
use super::engine::KeyEvent;

/// Result of processing a key event in an editor.
#[derive(Debug, Clone, PartialEq)]
pub enum EditorResult {
    /// Key was handled, session state updated
    Handled,
    
    /// Text should be committed, stay in current mode
    Commit(String),
    
    /// Text should be committed and mode should reset
    CommitAndReset(String),
    
    /// Request to switch to a different mode
    ModeSwitch(super::session::InputMode),
    
    /// Key not handled by this editor, pass to parent
    PassThrough,
}

/// Editor trait for handling input in specific modes.
///
/// Each editor (Phonetic, Punctuation, Suggestion) implements this trait
/// to provide mode-specific key processing and candidate generation.
///
/// # Example
///
/// ```no_run
/// use libpinyin::editor::{Editor, PhoneticEditor};
/// # use libpinyin::{Engine, ImeSession};
/// # let backend = Engine::from_data_dir("data").unwrap();
/// # let mut session = ImeSession::new();
///
/// let mut editor = PhoneticEditor::new(backend);
/// let result = editor.process_key(KeyEvent::Char('n'), &mut session);
/// ```
pub trait Editor {
    /// Process a key event in this editor's context.
    ///
    /// This is the main entry point for editor interaction. The editor
    /// should update the session state and return an appropriate result.
    fn process_key(&mut self, key: KeyEvent, session: &mut ImeSession) -> EditorResult;
    
    /// Update candidates based on current session input.
    ///
    /// Called when the input buffer changes and candidates need to be
    /// regenerated. The editor queries its backend and updates the
    /// session's candidate list.
    fn update_candidates(&mut self, session: &mut ImeSession);
    
    /// Reset editor state.
    ///
    /// Called when exiting the mode or clearing input. The editor should
    /// clear any internal state and prepare for new input.
    fn reset(&mut self);
    
    /// Get a human-readable name for this editor (for debugging/logging).
    fn name(&self) -> &'static str;
    
    /// Check if this editor can handle the given key event.
    ///
    /// Returns true if this editor should process the key, false if it
    /// should be passed through to the parent or another handler.
    fn can_handle(&self, _key: &KeyEvent) -> bool {
        // Default: handle all keys (editors can override)
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_editor_result_equality() {
        assert_eq!(EditorResult::Handled, EditorResult::Handled);
        assert_eq!(
            EditorResult::Commit("test".to_string()),
            EditorResult::Commit("test".to_string())
        );
        assert_ne!(
            EditorResult::Commit("a".to_string()),
            EditorResult::Commit("b".to_string())
        );
    }
}

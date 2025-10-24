//! Editor trait and implementations for different input modes.
//!
//! The editor architecture provides pluggable input handlers for different
//! modes (phonetic, punctuation, suggestions). Each editor implements the
//! `Editor` trait and processes key events in its specific context.

use crate::candidate::Candidate;
use crate::engine::{Engine, SyllableParser};
use crate::ime_engine::KeyEvent;
use crate::session::ImeSession;
use std::collections::HashMap;
use std::sync::Arc;

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
    ModeSwitch(crate::session::InputMode),

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
/// ```ignore
/// use libchinese_core::{PhoneticEditor, Editor, KeyEvent, ImeSession, Engine};
/// # use std::sync::Arc;
/// # let backend: Arc<Engine<_>> = todo!();
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

// ============================================================================
// PhoneticEditor - Phonetic input (pinyin/zhuyin)
// ============================================================================

/// Phonetic input editor (pinyin/zhuyin).
///
/// This is the main editor for Chinese character input via phonetic typing.
/// It takes raw pinyin/zhuyin input, queries the backend engine for candidates,
/// and handles selection/commit.
///
/// Generic over P: the parser type (PinyinParser, ZhuyinParser, etc.)
pub struct PhoneticEditor<P: SyllableParser> {
    /// Backend engine for linguistic processing
    backend: Arc<Engine<P>>,
}

impl<P: SyllableParser> PhoneticEditor<P> {
    /// Create a new phonetic editor with the given backend.
    pub fn new(backend: Arc<Engine<P>>) -> Self {
        Self { backend }
    }

    /// Get a reference to the backend engine.
    pub fn backend(&self) -> &Engine<P> {
        &self.backend
    }

    /// Handle character input
    fn handle_char(&mut self, ch: char, session: &mut ImeSession) -> EditorResult {
        // Add to input buffer
        session.input_buffer_mut().insert_char(ch);

        // Update candidates
        self.update_candidates(session);

        EditorResult::Handled
    }

    /// Handle backspace.
    fn handle_backspace(&mut self, session: &mut ImeSession) -> EditorResult {
        let deleted = session.input_buffer_mut().delete_before();

        if deleted {
            if session.input_buffer().text().is_empty() {
                // No more input, reset
                EditorResult::CommitAndReset(String::new())
            } else {
                self.update_candidates(session);
                EditorResult::Handled
            }
        } else {
            EditorResult::PassThrough
        }
    }

    /// Handle delete key.
    fn handle_delete(&mut self, session: &mut ImeSession) -> EditorResult {
        let deleted = session.input_buffer_mut().delete_after();

        if deleted {
            if session.input_buffer().text().is_empty() {
                EditorResult::CommitAndReset(String::new())
            } else {
                self.update_candidates(session);
                EditorResult::Handled
            }
        } else {
            EditorResult::PassThrough
        }
    }

    /// Handle space (select first candidate).
    fn handle_space(&mut self, session: &mut ImeSession) -> EditorResult {
        if session.candidates().is_empty() {
            // No candidates, just insert space
            return EditorResult::CommitAndReset(" ".to_string());
        }

        // Select first candidate
        if let Some(candidate) = session.candidates().selected_candidate() {
            let text = candidate.text.clone();

            // Learn the phrase
            self.backend.commit(&text);

            EditorResult::CommitAndReset(text)
        } else {
            EditorResult::PassThrough
        }
    }

    /// Handle enter (commit selection or raw input).
    fn handle_enter(&mut self, session: &mut ImeSession) -> EditorResult {
        if let Some(candidate) = session.candidates().selected_candidate() {
            let text = candidate.text.clone();
            self.backend.commit(&text);
            EditorResult::CommitAndReset(text)
        } else {
            // Commit raw input
            let raw = session.input_buffer().text().to_string();
            EditorResult::CommitAndReset(raw)
        }
    }

    /// Handle number key for candidate selection (1-9).
    fn handle_number(&mut self, n: u8, session: &mut ImeSession) -> EditorResult {
        if session.candidates().is_empty() {
            return EditorResult::PassThrough;
        }

        if !(1..=9).contains(&n) {
            return EditorResult::PassThrough;
        }

        let index = (n - 1) as usize;
        if let Some(candidate) = session.candidates_mut().select_by_index(index) {
            let text = candidate.text.clone();
            self.backend.commit(&text);
            EditorResult::CommitAndReset(text)
        } else {
            EditorResult::PassThrough
        }
    }
}

impl<P: SyllableParser> Editor for PhoneticEditor<P> {
    fn process_key(&mut self, key: KeyEvent, session: &mut ImeSession) -> EditorResult {
        match key {
            KeyEvent::Char(ch) => self.handle_char(ch, session),
            KeyEvent::Backspace => self.handle_backspace(session),
            KeyEvent::Delete => self.handle_delete(session),
            KeyEvent::Space => self.handle_space(session),
            KeyEvent::Enter => self.handle_enter(session),
            KeyEvent::Number(n) => self.handle_number(n, session),

            // Cursor navigation - update session but stay in mode
            KeyEvent::Left => {
                session.input_buffer_mut().move_left();
                EditorResult::Handled
            }
            KeyEvent::Right => {
                session.input_buffer_mut().move_right();
                EditorResult::Handled
            }
            KeyEvent::Up => {
                if !session.candidates().is_empty() {
                    session.candidates_mut().cursor_up();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::Down => {
                if !session.candidates().is_empty() {
                    session.candidates_mut().cursor_down();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::PageUp => {
                if !session.candidates().is_empty() {
                    session.candidates_mut().page_up();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::PageDown => {
                if !session.candidates().is_empty() {
                    session.candidates_mut().page_down();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::Escape => EditorResult::CommitAndReset(String::new()),
            // Global shortcuts handled by ImeEngine before routing
            KeyEvent::Ctrl(_) | KeyEvent::ShiftLock => EditorResult::PassThrough,
        }
    }

    fn update_candidates(&mut self, session: &mut ImeSession) {
        let input = session.input_buffer().text();

        if input.is_empty() {
            session.candidates_mut().clear();
            return;
        }

        // Get candidates from backend
        let backend_candidates = self.backend.input(input);

        // Convert to our Candidate type
        let candidates: Vec<Candidate> = backend_candidates
            .into_iter()
            .map(|c| Candidate::new(c.text, c.score))
            .collect();

        session.candidates_mut().set_candidates(candidates);

        // Update composition
        session.update_composition_from_input();
    }

    fn reset(&mut self) {
        // PhoneticEditor is stateless - backend Engine handles state
    }

    fn name(&self) -> &'static str {
        "PhoneticEditor"
    }

    fn can_handle(&self, key: &KeyEvent) -> bool {
        // Can handle most keys except non-lowercase chars when inactive
        match key {
            KeyEvent::Char(ch) => ch.is_ascii_lowercase(),
            _ => true,
        }
    }
}

// ============================================================================
// SuggestionEditor - Post-commit predictions
// ============================================================================

/// Suggestion editor for predictive text.
///
/// After committing text, this editor can suggest likely next words based
/// on the previous context using n-gram predictions.
///
/// Generic over P: the parser type (PinyinParser, ZhuyinParser, etc.)
pub struct SuggestionEditor<P: SyllableParser> {
    /// Backend engine for predictions
    backend: Arc<Engine<P>>,

    /// Previous committed text (context for predictions)
    context: String,

    /// Whether suggestions are currently active
    active: bool,
}

impl<P: SyllableParser> SuggestionEditor<P> {
    /// Create a new suggestion editor.
    pub fn new(backend: Arc<Engine<P>>) -> Self {
        Self {
            backend,
            context: String::new(),
            active: false,
        }
    }

    /// Activate suggestions based on previous context.
    ///
    /// This should be called after committing text to show predictions
    /// for the next word.
    pub fn activate(&mut self, previous_text: &str, session: &mut ImeSession) {
        self.context = previous_text.to_string();
        self.active = true;

        // Generate prediction candidates
        // Note: This is a simplified implementation. A full implementation
        // would use the n-gram model to predict next words based on context.
        self.update_candidates(session);
    }

    /// Check if suggestions are currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get the current context.
    pub fn context(&self) -> &str {
        &self.context
    }

    /// Learn user bigram when a prediction is selected.
    fn learn_selection(&self, selected_text: &str) {
        // Extract last character from context for bigram learning
        let chars: Vec<char> = self.context.chars().collect();
        if let Some(&last_char) = chars.last() {
            let w1 = last_char.to_string();

            // Learn bigram relationship for each character in selection
            let selected_chars: Vec<char> = selected_text.chars().collect();
            if let Some(&first_selected) = selected_chars.first() {
                let w2 = first_selected.to_string();
                self.backend.userdict().learn_bigram(&w1, &w2);
            }
        }
    }
}

impl<P: SyllableParser> Editor for SuggestionEditor<P> {
    fn process_key(&mut self, key: KeyEvent, session: &mut ImeSession) -> EditorResult {
        match key {
            // Character input - exit suggestion mode and switch to phonetic
            KeyEvent::Char(ch) if ch.is_ascii_lowercase() => {
                EditorResult::ModeSwitch(super::session::InputMode::Phonetic)
            }

            // Number selection
            KeyEvent::Number(n) => {
                if !(1..=9).contains(&n) {
                    return EditorResult::PassThrough;
                }

                let index = (n - 1) as usize;
                if let Some(candidate) = session.candidates_mut().select_by_index(index) {
                    let text = candidate.text.clone();

                    // Learn user bigram for personalization
                    self.learn_selection(&text);

                    // Learn the selection in userdict
                    self.backend.commit(&text);

                    // Stay in suggestion mode with new context
                    self.context.push_str(&text);
                    self.update_candidates(session);

                    EditorResult::Commit(text)
                } else {
                    EditorResult::PassThrough
                }
            }

            // Space - select first candidate
            KeyEvent::Space => {
                if let Some(candidate) = session.candidates().selected_candidate() {
                    let text = candidate.text.clone();

                    // Learn user bigram
                    self.learn_selection(&text);

                    self.backend.commit(&text);

                    // Update context and stay in suggestion mode
                    self.context.push_str(&text);
                    self.update_candidates(session);

                    EditorResult::Commit(text)
                } else {
                    // No candidates, exit suggestion mode
                    EditorResult::CommitAndReset(" ".to_string())
                }
            }

            // Enter - exit suggestion mode
            KeyEvent::Enter => EditorResult::CommitAndReset(String::new()),

            // Navigation
            KeyEvent::Up => {
                if !session.candidates().is_empty() {
                    session.candidates_mut().cursor_up();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::Down => {
                if !session.candidates().is_empty() {
                    session.candidates_mut().cursor_down();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::PageUp => {
                if !session.candidates().is_empty() {
                    session.candidates_mut().page_up();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::PageDown => {
                if !session.candidates().is_empty() {
                    session.candidates_mut().page_down();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }

            // Escape - exit suggestion mode
            KeyEvent::Escape => EditorResult::CommitAndReset(String::new()),

            // Any other key - exit suggestion mode
            _ => EditorResult::CommitAndReset(String::new()),
        }
    }

    fn update_candidates(&mut self, session: &mut ImeSession) {
        if self.context.is_empty() {
            session.candidates_mut().clear();
            return;
        }

        // Extract last 1-2 characters from context for prediction
        let chars: Vec<char> = self.context.chars().collect();
        let prediction_context = if chars.len() >= 2 {
            // Use last 2 characters for trigram prediction
            let start = chars.len() - 2;
            chars[start..].iter().collect::<String>()
        } else {
            // Use all available characters
            self.context.clone()
        };

        // Query n-gram model with user bigram learning
        let ngram = self.backend.ngram();
        let userdict = self.backend.userdict();

        // Use the enhanced prediction API with user learning
        let config = self.backend.config();
        let predictions =
            ngram.predict_next_with_user(&prediction_context, 10, Some(&*config), Some(userdict));

        // Convert predictions to candidates
        let candidates: Vec<Candidate> = predictions
            .into_iter()
            .map(|(text, log_prob)| {
                // Convert log probability to a reasonable score
                // Higher log_prob (less negative) = better score
                let score = (log_prob.exp() * 100.0);
                Candidate::new(text, score as f32)
            })
            .collect();

        // If no predictions from n-gram, fall back to common particles
        let candidates = if candidates.is_empty() {
            vec![
                Candidate::new("吗", 0.1),
                Candidate::new("呢", 0.09),
                Candidate::new("吧", 0.08),
                Candidate::new("啊", 0.07),
                Candidate::new("的", 0.06),
            ]
        } else {
            candidates
        };

        session.candidates_mut().set_candidates(candidates);

        // Clear composition in suggestion mode (no preedit)
        session.composition_mut().clear();
    }

    fn reset(&mut self) {
        self.context.clear();
        self.active = false;
    }

    fn name(&self) -> &'static str {
        "SuggestionEditor"
    }

    fn can_handle(&self, _key: &KeyEvent) -> bool {
        self.active
    }
}

// ============================================================================
// PunctuationEditor - Full-width punctuation selection
// ============================================================================

/// Punctuation editor for full-width character selection.
///
/// When the user types a punctuation key (like comma or period) during
/// Chinese input, this editor shows a list of full-width alternatives
/// to choose from.
pub struct PunctuationEditor {
    /// Map from ASCII punct to full-width alternatives
    punct_map: HashMap<char, Vec<&'static str>>,

    /// Currently active punctuation key (if any)
    active_key: Option<char>,
}

impl PunctuationEditor {
    /// Create a new punctuation editor with default mappings.
    pub fn new() -> Self {
        let mut punct_map = HashMap::new();

        // Comma variants
        punct_map.insert(',', vec!["，", ",", "、", "﹐", "﹑"]);

        // Period variants
        punct_map.insert('.', vec!["。", ".", "·", "﹒", "．"]);

        // Semicolon variants
        punct_map.insert(';', vec!["；", ";", "﹔"]);

        // Colon variants
        punct_map.insert(':', vec!["：", ":", "﹕"]);

        // Question mark variants
        punct_map.insert('?', vec!["？", "?", "﹖"]);

        // Exclamation mark variants
        punct_map.insert('!', vec!["！", "!", "﹗"]);

        // Quote variants
        punct_map.insert('"', vec!["\u{201C}", "\u{201D}", "\"", "＂"]); // ""
        punct_map.insert('\'', vec!["\u{2018}", "\u{2019}", "'", "＇"]); // ''

        // Parentheses
        punct_map.insert('(', vec!["（", "(", "﹙"]);
        punct_map.insert(')', vec!["）", ")", "﹚"]);

        // Brackets
        punct_map.insert('[', vec!["【", "[", "［"]);
        punct_map.insert(']', vec!["】", "]", "］"]);

        // Braces
        punct_map.insert('{', vec!["｛", "{", "「", "『"]);
        punct_map.insert('}', vec!["｝", "}", "」", "』"]);

        // Dash/Hyphen
        punct_map.insert('-', vec!["—", "–", "-", "－"]);

        // Ellipsis
        punct_map.insert('~', vec!["～", "…", "~"]);

        Self {
            punct_map,
            active_key: None,
        }
    }

    /// Check if a character has punctuation alternatives.
    pub fn has_alternatives(&self, ch: char) -> bool {
        self.punct_map.contains_key(&ch)
    }

    /// Activate punctuation selection for a given key.
    pub fn activate(&mut self, key: char, session: &mut ImeSession) -> bool {
        if let Some(alternatives) = self.punct_map.get(&key) {
            self.active_key = Some(key);

            // Set candidates
            let candidates: Vec<Candidate> = alternatives
                .iter()
                .map(|&s| Candidate::new(s, 1.0))
                .collect();

            session.candidates_mut().set_candidates(candidates);

            // Set preedit to show the original key
            session.composition_mut().preedit = key.to_string();
            session.composition_mut().cursor = 1;

            true
        } else {
            false
        }
    }

    /// Handle selection of a punctuation candidate.
    fn select_candidate(&mut self, session: &mut ImeSession) -> Option<String> {
        session.candidates().selected_candidate().map(|candidate| candidate.text.clone())
    }
}

impl Default for PunctuationEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor for PunctuationEditor {
    fn process_key(&mut self, key: KeyEvent, session: &mut ImeSession) -> EditorResult {
        match key {
            // Number selection
            KeyEvent::Number(n) => {
                if !(1..=9).contains(&n) {
                    return EditorResult::PassThrough;
                }

                let index = (n - 1) as usize;
                if let Some(candidate) = session.candidates_mut().select_by_index(index) {
                    let text = candidate.text.clone();
                    EditorResult::CommitAndReset(text)
                } else {
                    EditorResult::PassThrough
                }
            }

            // Space or Enter - select first candidate
            KeyEvent::Space | KeyEvent::Enter => {
                if let Some(text) = self.select_candidate(session) {
                    EditorResult::CommitAndReset(text)
                } else {
                    EditorResult::PassThrough
                }
            }

            // Up/Down for candidate navigation
            KeyEvent::Up => {
                session.candidates_mut().cursor_up();
                EditorResult::Handled
            }
            KeyEvent::Down => {
                session.candidates_mut().cursor_down();
                EditorResult::Handled
            }

            // Page navigation
            KeyEvent::PageUp => {
                session.candidates_mut().page_up();
                EditorResult::Handled
            }
            KeyEvent::PageDown => {
                session.candidates_mut().page_down();
                EditorResult::Handled
            }

            // Escape - cancel and use original character
            KeyEvent::Escape => {
                if let Some(key) = self.active_key {
                    EditorResult::CommitAndReset(key.to_string())
                } else {
                    EditorResult::CommitAndReset(String::new())
                }
            }

            // Any other key - commit first candidate and pass through
            _ => {
                if let Some(text) = self.select_candidate(session) {
                    // Commit the punctuation and indicate pass through for the new key
                    EditorResult::Commit(text)
                } else {
                    EditorResult::PassThrough
                }
            }
        }
    }

    fn update_candidates(&mut self, _session: &mut ImeSession) {
        // Punctuation candidates are set on activation, no dynamic updates
    }

    fn reset(&mut self) {
        self.active_key = None;
    }

    fn name(&self) -> &'static str {
        "PunctuationEditor"
    }

    fn can_handle(&self, key: &KeyEvent) -> bool {
        // Can handle most keys when active
        !matches!(
            key,
            KeyEvent::Char(_) | KeyEvent::Backspace | KeyEvent::Delete
        )
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{SyllableParser, SyllableType};
    use crate::session::{ImeSession, InputMode};
    use crate::{Config, Lexicon, Model, NGramModel, UserDict};
    use std::sync::Arc;

    // Minimal test parser for unit tests
    #[derive(Clone)]
    struct TestParser;

    impl SyllableParser for TestParser {
        type Syllable = TestSyllable;

        fn segment_top_k(
            &self,
            _input: &str,
            _k: usize,
            _allow_fuzzy: bool,
        ) -> Vec<Vec<Self::Syllable>> {
            vec![]
        }
    }

    #[derive(Clone, Debug)]
    struct TestSyllable;

    impl SyllableType for TestSyllable {
        fn text(&self) -> &str {
            ""
        }

        fn is_fuzzy(&self) -> bool {
            false
        }
    }

    fn test_backend() -> Arc<Engine<TestParser>> {
        let lex = Lexicon::new();
        let ngram = NGramModel::new();
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_path = std::env::temp_dir().join(format!("test_editor_{}.redb", unique_id));
        let userdict = UserDict::new(&temp_path).unwrap();
        let model = Model::new(lex, ngram, userdict, Config::default());
        Arc::new(Engine::new(model, TestParser))
    }

    // EditorResult tests
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

    // PhoneticEditor tests
    #[test]
    fn test_phonetic_editor_name() {
        let backend = test_backend();
        let editor = PhoneticEditor::new(backend);
        assert_eq!(editor.name(), "PhoneticEditor");
    }

    #[test]
    fn test_phonetic_can_handle() {
        let backend = test_backend();
        let editor = PhoneticEditor::new(backend);

        assert!(editor.can_handle(&KeyEvent::Char('a')));
        assert!(editor.can_handle(&KeyEvent::Char('z')));
        assert!(!editor.can_handle(&KeyEvent::Char('A')));
        assert!(!editor.can_handle(&KeyEvent::Char('1')));
        assert!(editor.can_handle(&KeyEvent::Space));
    }

    #[test]
    fn test_phonetic_char_input() {
        let backend = test_backend();
        let mut editor = PhoneticEditor::new(backend);
        let mut session = ImeSession::new();

        let result = editor.process_key(KeyEvent::Char('n'), &mut session);
        assert_eq!(result, EditorResult::Handled);
        assert_eq!(session.input_buffer().text(), "n");
    }

    #[test]
    fn test_phonetic_backspace() {
        let backend = test_backend();
        let mut editor = PhoneticEditor::new(backend);
        let mut session = ImeSession::new();

        // Add some input
        editor.process_key(KeyEvent::Char('n'), &mut session);
        editor.process_key(KeyEvent::Char('i'), &mut session);

        // Backspace once
        let result = editor.process_key(KeyEvent::Backspace, &mut session);
        assert_eq!(result, EditorResult::Handled);
        assert_eq!(session.input_buffer().text(), "n");

        // Backspace again should reset
        let result = editor.process_key(KeyEvent::Backspace, &mut session);
        assert!(matches!(result, EditorResult::CommitAndReset(_)));
    }

    #[test]
    fn test_phonetic_space_with_no_candidates() {
        let backend = test_backend();
        let mut editor = PhoneticEditor::new(backend);
        let mut session = ImeSession::new();

        // Space with no input should commit space
        let result = editor.process_key(KeyEvent::Space, &mut session);
        assert!(matches!(result, EditorResult::CommitAndReset(s) if s == " "));
    }

    #[test]
    fn test_phonetic_escape() {
        let backend = test_backend();
        let mut editor = PhoneticEditor::new(backend);
        let mut session = ImeSession::new();

        editor.process_key(KeyEvent::Char('n'), &mut session);

        let result = editor.process_key(KeyEvent::Escape, &mut session);
        assert!(matches!(result, EditorResult::CommitAndReset(_)));
    }

    #[test]
    fn test_phonetic_cursor_navigation() {
        let backend = test_backend();
        let mut editor = PhoneticEditor::new(backend);
        let mut session = ImeSession::new();

        editor.process_key(KeyEvent::Char('n'), &mut session);
        editor.process_key(KeyEvent::Char('i'), &mut session);

        let result = editor.process_key(KeyEvent::Left, &mut session);
        assert_eq!(result, EditorResult::Handled);

        let result = editor.process_key(KeyEvent::Right, &mut session);
        assert_eq!(result, EditorResult::Handled);
    }

    // SuggestionEditor tests
    #[test]
    fn test_new_suggestion_editor() {
        let backend = test_backend();
        let editor = SuggestionEditor::new(backend);
        assert!(!editor.is_active());
        assert_eq!(editor.context(), "");
    }

    #[test]
    fn test_suggestion_activate() {
        let backend = test_backend();
        let mut editor = SuggestionEditor::new(backend);
        let mut session = ImeSession::new();

        editor.activate("你好", &mut session);

        assert!(editor.is_active());
        assert_eq!(editor.context(), "你好");
        assert!(!session.candidates().is_empty());
    }

    #[test]
    fn test_suggestion_char_switches_mode() {
        let backend = test_backend();
        let mut editor = SuggestionEditor::new(backend);
        let mut session = ImeSession::new();

        editor.activate("你好", &mut session);

        let result = editor.process_key(KeyEvent::Char('n'), &mut session);
        assert!(matches!(
            result,
            EditorResult::ModeSwitch(InputMode::Phonetic)
        ));
    }

    #[test]
    fn test_suggestion_space_selection() {
        let backend = test_backend();
        let mut editor = SuggestionEditor::new(backend);
        let mut session = ImeSession::new();

        editor.activate("你好", &mut session);

        // Should have candidates
        assert!(!session.candidates().is_empty());

        let result = editor.process_key(KeyEvent::Space, &mut session);
        assert!(matches!(result, EditorResult::Commit(_)));
    }

    #[test]
    fn test_suggestion_escape_exits() {
        let backend = test_backend();
        let mut editor = SuggestionEditor::new(backend);
        let mut session = ImeSession::new();

        editor.activate("你好", &mut session);

        let result = editor.process_key(KeyEvent::Escape, &mut session);
        assert!(matches!(result, EditorResult::CommitAndReset(_)));
    }

    #[test]
    fn test_suggestion_reset() {
        let backend = test_backend();
        let mut editor = SuggestionEditor::new(backend);
        let mut session = ImeSession::new();

        editor.activate("你好", &mut session);
        assert!(editor.is_active());

        editor.reset();
        assert!(!editor.is_active());
        assert_eq!(editor.context(), "");
    }

    #[test]
    fn test_suggestion_navigation() {
        let backend = test_backend();
        let mut editor = SuggestionEditor::new(backend);
        let mut session = ImeSession::new();

        editor.activate("你好", &mut session);

        let result = editor.process_key(KeyEvent::Down, &mut session);
        assert_eq!(result, EditorResult::Handled);

        let result = editor.process_key(KeyEvent::Up, &mut session);
        assert_eq!(result, EditorResult::Handled);
    }

    // PunctuationEditor tests
    #[test]
    fn test_new_punctuation_editor() {
        let editor = PunctuationEditor::new();
        assert!(editor.has_alternatives(','));
        assert!(editor.has_alternatives('.'));
        assert!(!editor.has_alternatives('a'));
    }

    #[test]
    fn test_punctuation_activate() {
        let mut editor = PunctuationEditor::new();
        let mut session = ImeSession::new();

        let activated = editor.activate(',', &mut session);
        assert!(activated);
        assert_eq!(editor.active_key, Some(','));
        assert!(!session.candidates().is_empty());

        // First candidate should be full-width comma
        let first = session
            .candidates()
            .current_page_candidates()
            .first()
            .unwrap();
        assert_eq!(first.text, "，");
    }

    #[test]
    fn test_punctuation_number_selection() {
        let mut editor = PunctuationEditor::new();
        let mut session = ImeSession::new();

        editor.activate(',', &mut session);

        let result = editor.process_key(KeyEvent::Number(1), &mut session);
        assert!(matches!(result, EditorResult::CommitAndReset(s) if s == "，"));
    }

    #[test]
    fn test_punctuation_space_selection() {
        let mut editor = PunctuationEditor::new();
        let mut session = ImeSession::new();

        editor.activate('.', &mut session);

        let result = editor.process_key(KeyEvent::Space, &mut session);
        assert!(matches!(result, EditorResult::CommitAndReset(s) if s == "。"));
    }

    #[test]
    fn test_punctuation_cursor_navigation() {
        let mut editor = PunctuationEditor::new();
        let mut session = ImeSession::new();

        editor.activate(',', &mut session);

        assert_eq!(session.candidates().cursor(), 0);

        let result = editor.process_key(KeyEvent::Down, &mut session);
        assert_eq!(result, EditorResult::Handled);
        assert_eq!(session.candidates().cursor(), 1);

        let result = editor.process_key(KeyEvent::Up, &mut session);
        assert_eq!(result, EditorResult::Handled);
        assert_eq!(session.candidates().cursor(), 0);
    }

    #[test]
    fn test_punctuation_escape() {
        let mut editor = PunctuationEditor::new();
        let mut session = ImeSession::new();

        editor.activate(',', &mut session);

        let result = editor.process_key(KeyEvent::Escape, &mut session);
        assert!(matches!(result, EditorResult::CommitAndReset(s) if s == ","));
    }

    #[test]
    fn test_punctuation_reset() {
        let mut editor = PunctuationEditor::new();
        let mut session = ImeSession::new();

        editor.activate(',', &mut session);
        assert!(editor.active_key.is_some());

        editor.reset();
        assert!(editor.active_key.is_none());
    }

    #[test]
    fn test_punctuation_all_marks() {
        let editor = PunctuationEditor::new();

        // Test that all expected punctuation marks have alternatives
        for ch in [
            ',', '.', ';', ':', '?', '!', '"', '\'', '(', ')', '[', ']', '{', '}', '-', '~',
        ] {
            assert!(
                editor.has_alternatives(ch),
                "Missing alternatives for '{}'",
                ch
            );
        }
    }
}

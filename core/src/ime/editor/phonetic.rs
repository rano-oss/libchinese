//! Phonetic editor for pinyin/zhuyin input.
//!
//! This editor handles phonetic input (typing pinyin to get Chinese characters).
//! It wraps the backend Engine to generate candidates and manages the phonetic
//! input workflow.

use crate::engine::Engine;
use crate::engine::SyllableParser;
use super::super::session::ImeSession;
use super::super::engine::KeyEvent;
use super::super::candidates::Candidate;
use super::{Editor, EditorResult};
use std::sync::Arc;

/// Check if a character is a tone mark used in zhuyin.
/// Tone marks: ˊ (2nd), ˇ (3rd), ˋ (4th), ˙ (light)
fn is_tone_mark(ch: char) -> bool {
    matches!(ch, '\u{02CA}' | '\u{02C7}' | '\u{02CB}' | '\u{02D9}')
}

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
    
    /// Handle character input (a-z).
    fn handle_char(&mut self, ch: char, session: &mut ImeSession) -> EditorResult {
        // Accept phonetic characters (ASCII lowercase for pinyin, Unicode for bopomofo/zhuyin)
        // Allow tone marks (ˊˇˋ˙) and other combining characters
        if !ch.is_ascii_lowercase() && !ch.is_alphabetic() && !is_tone_mark(ch) {
            return EditorResult::PassThrough;
        }
        
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
        
        if n < 1 || n > 9 {
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
            KeyEvent::Escape => {
                EditorResult::CommitAndReset(String::new())
            }
            // Global shortcuts handled by ImeEngine before routing
            KeyEvent::Ctrl(_) | KeyEvent::ShiftLock => {
                EditorResult::PassThrough
            }
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
            .map(|c| Candidate::new(c.text, c.score as f32))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Model, Lexicon, NGramModel, UserDict, Config};
    use crate::engine::{SyllableParser, SyllableType};
    use std::sync::Arc;
    
    // Minimal test parser for unit tests
    #[derive(Clone)]
    struct TestParser;
    
    impl SyllableParser for TestParser {
        type Syllable = TestSyllable;
        
        fn segment_top_k(&self, _input: &str, _k: usize, _allow_fuzzy: bool) -> Vec<Vec<Self::Syllable>> {
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
        let temp_path = std::env::temp_dir().join(format!("test_phonetic_{}.redb", unique_id));
        let userdict = UserDict::new(&temp_path).unwrap();
        let model = Model::new(lex, ngram, userdict, Config::default());
        Arc::new(Engine::new(model, TestParser))
    }
    
    #[test]
    fn test_phonetic_editor_name() {
        let backend = test_backend();
        let editor = PhoneticEditor::new(backend);
        assert_eq!(editor.name(), "PhoneticEditor");
    }
    
    #[test]
    fn test_can_handle() {
        let backend = test_backend();
        let editor = PhoneticEditor::new(backend);
        
        assert!(editor.can_handle(&KeyEvent::Char('a')));
        assert!(editor.can_handle(&KeyEvent::Char('z')));
        assert!(!editor.can_handle(&KeyEvent::Char('A')));
        assert!(!editor.can_handle(&KeyEvent::Char('1')));
        assert!(editor.can_handle(&KeyEvent::Space));
    }
    
    #[test]
    fn test_char_input() {
        let backend = test_backend();
        let mut editor = PhoneticEditor::new(backend);
        let mut session = ImeSession::new();
        
        let result = editor.process_key(KeyEvent::Char('n'), &mut session);
        assert_eq!(result, EditorResult::Handled);
        assert_eq!(session.input_buffer().text(), "n");
    }
    
    #[test]
    fn test_backspace() {
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
    fn test_space_with_no_candidates() {
        let backend = test_backend();
        let mut editor = PhoneticEditor::new(backend);
        let mut session = ImeSession::new();
        
        // Space with no input should commit space
        let result = editor.process_key(KeyEvent::Space, &mut session);
        assert!(matches!(result, EditorResult::CommitAndReset(s) if s == " "));
    }
    
    #[test]
    fn test_escape() {
        let backend = test_backend();
        let mut editor = PhoneticEditor::new(backend);
        let mut session = ImeSession::new();
        
        editor.process_key(KeyEvent::Char('n'), &mut session);
        
        let result = editor.process_key(KeyEvent::Escape, &mut session);
        assert!(matches!(result, EditorResult::CommitAndReset(_)));
    }
    
    #[test]
    fn test_cursor_navigation() {
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
}

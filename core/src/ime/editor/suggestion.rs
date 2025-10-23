//! Suggestion editor for post-commit predictions.
//!
//! This editor provides word predictions after the user commits text,
//! using n-gram models to suggest likely next words based on context.

use crate::engine::{Engine, SyllableParser};
use super::super::session::ImeSession;
use super::super::engine::KeyEvent;
use super::super::candidates::Candidate;
use super::{Editor, EditorResult};
use std::sync::Arc;

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
                EditorResult::ModeSwitch(super::super::session::InputMode::Phonetic)
            }
            
            // Number selection
            KeyEvent::Number(n) => {
                if n < 1 || n > 9 {
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
            KeyEvent::Enter => {
                EditorResult::CommitAndReset(String::new())
            }
            
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
            KeyEvent::Escape => {
                EditorResult::CommitAndReset(String::new())
            }
            
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
        let predictions = ngram.predict_next_with_user(
            &prediction_context, 
            10, 
            Some(self.backend.config()),
            Some(userdict)
        );

        // Convert predictions to candidates
        let candidates: Vec<Candidate> = predictions
            .into_iter()
            .map(|(text, log_prob)| {
                // Convert log probability to a reasonable score
                // Higher log_prob (less negative) = better score
                let score = (log_prob.exp() * 100.0) as f64;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Model, Lexicon, NGramModel, UserDict, Config};
    use crate::engine::{SyllableParser, SyllableType};
    use crate::ime::session::{ImeSession, InputMode};
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
        let temp_path = std::env::temp_dir().join(format!("test_suggestion_{}.redb", unique_id));
        let userdict = UserDict::new(&temp_path).unwrap();
        let model = Model::new(lex, ngram, userdict, Config::default());
        Arc::new(Engine::new(model, TestParser))
    }
    
    #[test]
    fn test_new_suggestion_editor() {
        let backend = test_backend();
        let editor = SuggestionEditor::new(backend);
        assert!(!editor.is_active());
        assert_eq!(editor.context(), "");
    }
    
    #[test]
    fn test_activate() {
        let backend = test_backend();
        let mut editor = SuggestionEditor::new(backend);
        let mut session = ImeSession::new();
        
        editor.activate("你好", &mut session);
        
        assert!(editor.is_active());
        assert_eq!(editor.context(), "你好");
        assert!(!session.candidates().is_empty());
    }
    
    #[test]
    fn test_char_switches_mode() {
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
    fn test_space_selection() {
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
    fn test_escape_exits() {
        let backend = test_backend();
        let mut editor = SuggestionEditor::new(backend);
        let mut session = ImeSession::new();
        
        editor.activate("你好", &mut session);
        
        let result = editor.process_key(KeyEvent::Escape, &mut session);
        assert!(matches!(result, EditorResult::CommitAndReset(_)));
    }
    
    #[test]
    fn test_reset() {
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
    fn test_navigation() {
        let backend = test_backend();
        let mut editor = SuggestionEditor::new(backend);
        let mut session = ImeSession::new();
        
        editor.activate("你好", &mut session);
        
        let result = editor.process_key(KeyEvent::Down, &mut session);
        assert_eq!(result, EditorResult::Handled);
        
        let result = editor.process_key(KeyEvent::Up, &mut session);
        assert_eq!(result, EditorResult::Handled);
    }
}

//! Punctuation editor for full-width punctuation selection.
//!
//! This editor provides a popup menu for selecting full-width punctuation
//! marks commonly used in Chinese text. It's triggered by typing punctuation
//! keys (comma, period, etc.) during phonetic input.

use super::super::session::ImeSession;
use super::super::engine::KeyEvent;
use super::super::candidates::Candidate;
use super::{Editor, EditorResult};
use std::collections::HashMap;

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
        punct_map.insert('"', vec!["\u{201C}", "\u{201D}", "\"", "＂"]);  // ""
        punct_map.insert('\'', vec!["\u{2018}", "\u{2019}", "'", "＇"]);  // ''
        
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
        if let Some(candidate) = session.candidates().selected_candidate() {
            Some(candidate.text.clone())
        } else {
            None
        }
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
                if n < 1 || n > 9 {
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
        !matches!(key, KeyEvent::Char(_) | KeyEvent::Backspace | KeyEvent::Delete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_new_punctuation_editor() {
        let editor = PunctuationEditor::new();
        assert!(editor.has_alternatives(','));
        assert!(editor.has_alternatives('.'));
        assert!(!editor.has_alternatives('a'));
    }
    
    #[test]
    fn test_activate() {
        let mut editor = PunctuationEditor::new();
        let mut session = ImeSession::new();
        
        let activated = editor.activate(',', &mut session);
        assert!(activated);
        assert_eq!(editor.active_key, Some(','));
        assert!(!session.candidates().is_empty());
        
        // First candidate should be full-width comma
        let first = session.candidates().current_page_candidates().first().unwrap();
        assert_eq!(first.text, "，");
    }
    
    #[test]
    fn test_number_selection() {
        let mut editor = PunctuationEditor::new();
        let mut session = ImeSession::new();
        
        editor.activate(',', &mut session);
        
        let result = editor.process_key(KeyEvent::Number(1), &mut session);
        assert!(matches!(result, EditorResult::CommitAndReset(s) if s == "，"));
    }
    
    #[test]
    fn test_space_selection() {
        let mut editor = PunctuationEditor::new();
        let mut session = ImeSession::new();
        
        editor.activate('.', &mut session);
        
        let result = editor.process_key(KeyEvent::Space, &mut session);
        assert!(matches!(result, EditorResult::CommitAndReset(s) if s == "。"));
    }
    
    #[test]
    fn test_cursor_navigation() {
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
    fn test_escape() {
        let mut editor = PunctuationEditor::new();
        let mut session = ImeSession::new();
        
        editor.activate(',', &mut session);
        
        let result = editor.process_key(KeyEvent::Escape, &mut session);
        assert!(matches!(result, EditorResult::CommitAndReset(s) if s == ","));
    }
    
    #[test]
    fn test_reset() {
        let mut editor = PunctuationEditor::new();
        let mut session = ImeSession::new();
        
        editor.activate(',', &mut session);
        assert!(editor.active_key.is_some());
        
        editor.reset();
        assert!(editor.active_key.is_none());
    }
    
    #[test]
    fn test_all_punctuation_marks() {
        let editor = PunctuationEditor::new();
        
        // Test that all expected punctuation marks have alternatives
        for ch in [',', '.', ';', ':', '?', '!', '"', '\'', '(', ')', '[', ']', '{', '}', '-', '~'] {
            assert!(editor.has_alternatives(ch), "Missing alternatives for '{}'", ch);
        }
    }
}

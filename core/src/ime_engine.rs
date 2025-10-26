//! IME engine with session management and key event processing.
//!
//! The `ImeEngine` wraps the backend `Engine` with session state management,
//! providing a `process_key()` method that handles key events and maintains
//! IME state across multiple interactions. It uses a pluggable editor
//! architecture to support different input modes (phonetic, punctuation, suggestion).

use super::context::ImeContext;
use super::editor::{Editor, EditorResult, PhoneticEditor, PunctuationEditor, SuggestionEditor};
use super::session::{ImeSession, InputMode};
use crate::engine::{Engine, SyllableParser};
use std::sync::Arc;

/// Key event types that the IME can process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyEvent {
    /// Character input (a-z, punctuation, etc.)
    Char(char),
    /// Backspace key
    Backspace,
    /// Delete key
    Delete,
    /// Left arrow key
    Left,
    /// Right arrow key
    Right,
    /// Up arrow key (candidate cursor up)
    Up,
    /// Down arrow key (candidate cursor down)
    Down,
    /// Page up (candidate page up)
    PageUp,
    /// Page down (candidate page down)
    PageDown,
    /// Space key (select first candidate or commit)
    Space,
    /// Enter/Return key (commit current selection)
    Enter,
    /// Escape key (clear/cancel)
    Escape,
    /// Number key for candidate selection (1-9)
    Number(u8),
    /// Ctrl + character (e.g., Ctrl+period for punctuation toggle)
    Ctrl(char),
    /// Shift lock toggle (for passthrough mode)
    ShiftLock,
}

/// Result of processing a key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyResult {
    /// Key was handled by the IME
    Handled,
    /// Key was not handled (pass through to application)
    NotHandled,
}

/// IME engine with session management.
///
/// This struct combines the backend Engine with a session that tracks
/// input state across multiple key events. It uses pluggable editors
/// for different input modes (phonetic, punctuation, suggestion).
pub struct ImeEngine<P: SyllableParser> {
    /// Phonetic input editor
    phonetic_editor: PhoneticEditor<P>,

    /// Punctuation selection editor
    punct_editor: PunctuationEditor,

    /// Suggestion/prediction editor
    suggestion_editor: SuggestionEditor<P>,

    /// Session state
    session: ImeSession,

    /// Context for platform communication
    context: ImeContext,
}

impl<P: SyllableParser> ImeEngine<P> {
    /// Create a new IME engine with the given backend.
    pub fn new(backend: Engine<P>) -> Self {
        let backend_arc = Arc::new(backend);
        Self {
            phonetic_editor: PhoneticEditor::new(backend_arc.clone()),
            punct_editor: PunctuationEditor::new(),
            suggestion_editor: SuggestionEditor::new(backend_arc),
            session: ImeSession::with_page_size(5),
            context: ImeContext::new(),
        }
    }

    /// Create a new IME engine from an Arc-wrapped backend.
    ///
    /// This is useful when you already have an Arc<Engine<P>> from another source.
    pub fn from_arc(backend: Arc<Engine<P>>) -> Self {
        Self {
            phonetic_editor: PhoneticEditor::new(backend.clone()),
            punct_editor: PunctuationEditor::new(),
            suggestion_editor: SuggestionEditor::new(backend),
            session: ImeSession::with_page_size(5),
            context: ImeContext::new(),
        }
    }

    /// Create an IME engine with specified candidate page size.
    pub fn with_page_size(backend: Engine<P>, page_size: usize) -> Self {
        let mut engine = Self::new(backend);
        engine.session = ImeSession::with_page_size(page_size);
        engine
    }

    /// Create an IME engine from Arc with specified candidate page size.
    pub fn from_arc_with_page_size(backend: Arc<Engine<P>>, page_size: usize) -> Self {
        let mut engine = Self::from_arc(backend);
        engine.session = ImeSession::with_page_size(page_size);
        engine
    }

    /// Get a reference to the context for reading IME state.
    pub fn context(&self) -> &ImeContext {
        &self.context
    }

    /// Get a mutable reference to the context.
    pub fn context_mut(&mut self) -> &mut ImeContext {
        &mut self.context
    }

    /// Get a reference to the session.
    pub fn session(&self) -> &ImeSession {
        &self.session
    }

    /// Reset the IME to initial state.
    pub fn reset(&mut self) {
        self.session.clear();
        self.context.clear();
        self.phonetic_editor.reset();
        self.punct_editor.reset();
        // Note: Don't reset suggestion_editor as it may be about to activate
    }

    /// Maybe enter suggestion mode automatically after a commit.
    ///
    /// This checks configuration settings to determine if auto-suggestion
    /// should be triggered based on the committed text.
    fn maybe_auto_suggest(&mut self, committed_text: &str) {
        // Skip if text is empty
        if committed_text.is_empty() {
            return;
        }

        // Get configuration from phonetic editor's backend
        let config = self.phonetic_editor.backend().config();

        // Check if auto-suggestion is enabled
        if !config.auto_suggestion {
            return;
        }

        // Check if text meets minimum length requirement
        let char_count = committed_text.chars().count();
        let should_activate = char_count >= config.min_suggestion_trigger_length;

        // Drop config borrow before mutating self
        drop(config);

        if !should_activate {
            return;
        }

        // Activate suggestion mode
        self.session.activate();
        self.session.set_mode(InputMode::Suggestion);
        self.suggestion_editor
            .activate(committed_text, &mut self.session);
        self.session.sync_to_context(&mut self.context);
        self.update_auxiliary_text();
    }

    /// Process a key event and update IME state.
    ///
    /// This is the main entry point for IME interaction. After calling this,
    /// the platform should read `context()` to update the UI (preedit,
    /// candidates, commit text).
    ///
    /// Returns `KeyResult::Handled` if the IME consumed the key,
    /// or `KeyResult::NotHandled` if it should pass through to the application.
    pub fn process_key(&mut self, key: KeyEvent) -> KeyResult {
        // Clear commit text from previous key
        self.context.commit_text.clear();

        // Translate selection key characters to Number events
        // This allows configurable selection keys (e.g., asdfghjkl vs 123456789)
        let key = if let KeyEvent::Char(ch) = key {
            let config = self.phonetic_editor.backend().config();
            if let Some(index) = config.selection_key_index(ch) {
                drop(config);
                // Convert to 1-based number (index 0 → number 1, etc.)
                KeyEvent::Number((index + 1) as u8)
            } else {
                drop(config);
                key
            }
        } else {
            key
        };

        // Handle global shortcuts first (before mode routing)
        match key {
            KeyEvent::ShiftLock => {
                // Toggle passthrough mode
                if self.session.mode() == InputMode::Passthrough {
                    self.session.set_mode(InputMode::Init);
                    self.context.clear();
                    self.context.auxiliary_text.clear();
                } else {
                    // Save current mode and switch to passthrough
                    self.session.set_mode(InputMode::Passthrough);
                    self.context.clear();
                    self.context.auxiliary_text = "直通模式 | Shift_lock切换".to_string();
                }
                return KeyResult::Handled;
            }
            KeyEvent::Ctrl('.') => {
                // Ctrl+period: toggle punctuation mode
                // But not in passthrough mode
                if self.session.mode() == InputMode::Passthrough {
                    return KeyResult::NotHandled;
                }

                let was_phonetic = self.session.mode() == InputMode::Phonetic;

                if was_phonetic {
                    // Commit current preedit if any
                    if !self.session.input_buffer().is_empty() {
                        let text = self.session.input_buffer().text().to_string();
                        self.context.commit_text = text;
                    }
                    self.reset();
                    // After reset from phonetic, we're done (stay in Init)
                    return KeyResult::Handled;
                }

                // Toggle: if in punctuation, go to init; else go to punctuation
                if self.session.mode() == InputMode::Punctuation {
                    self.reset();
                } else {
                    self.session.set_mode(InputMode::Punctuation);
                    self.session.activate();
                }

                self.session.sync_to_context(&mut self.context);
                self.update_auxiliary_text();
                return KeyResult::Handled;
            }
            _ => {}
        }

        // Passthrough mode: ignore all other keys
        if self.session.mode() == InputMode::Passthrough {
            return KeyResult::NotHandled;
        }

        // Route to appropriate editor based on current mode
        let result = match self.session.mode() {
            InputMode::Init => {
                // Check if this is phonetic input or punctuation
                // Accept:
                // - ASCII lowercase (pinyin: a-z)
                // - Bopomofo/Zhuyin characters (U+3105-U+3129)
                // - Zhuyin tone marks (ˊˇˋ˙)
                let is_phonetic_input = matches!(key, KeyEvent::Char(ch) if
                    ch.is_ascii_lowercase()
                    || ('\u{3105}'..='\u{3129}').contains(&ch)  // Bopomofo block
                    || matches!(ch, 'ˊ' | 'ˇ' | 'ˋ' | '˙')      // Tone marks
                );

                if is_phonetic_input {
                    // Activate phonetic mode
                    self.session.activate();
                    self.session.set_mode(InputMode::Phonetic);
                    self.phonetic_editor.process_key(key, &mut self.session)
                } else if matches!(key, KeyEvent::Char(ch) if self.punct_editor.has_alternatives(ch))
                {
                    // Activate punctuation mode
                    self.session.activate();
                    self.session.set_mode(InputMode::Punctuation);
                    if let KeyEvent::Char(ch) = key {
                        self.punct_editor.activate(ch, &mut self.session);
                    }
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            InputMode::Phonetic => {
                // Check for punctuation trigger
                if matches!(key, KeyEvent::Char(',')) {
                    // Switch to punctuation mode
                    self.session.set_mode(InputMode::Punctuation);
                    self.punct_editor.activate(',', &mut self.session);
                    EditorResult::Handled
                } else {
                    self.phonetic_editor.process_key(key, &mut self.session)
                }
            }
            InputMode::Punctuation => self.punct_editor.process_key(key, &mut self.session),
            InputMode::Suggestion => self.suggestion_editor.process_key(key, &mut self.session),
            InputMode::Passthrough => {
                // Unreachable: passthrough handled before match
                unreachable!("Passthrough mode should be handled before routing")
            }
        };

        // Handle editor result
        match result {
            EditorResult::Handled => {
                // Sync session to context
                self.session.sync_to_context(&mut self.context);
                self.update_auxiliary_text();
                KeyResult::Handled
            }
            EditorResult::Commit(text) => {
                // Apply full-width conversion if enabled
                let text = if self.phonetic_editor.backend().config().is_fullwidth() {
                    crate::utils::to_fullwidth(&text)
                } else {
                    text
                };

                // Commit but stay active
                self.context.commit_text = text.clone();
                self.session.sync_to_context(&mut self.context);
                self.update_auxiliary_text();

                // Auto-enter suggestion mode if enabled and text meets criteria
                self.maybe_auto_suggest(&text);

                KeyResult::Handled
            }
            EditorResult::CommitAndReset(text) => {
                // Apply full-width conversion if enabled
                let text = if self.phonetic_editor.backend().config().is_fullwidth() {
                    crate::utils::to_fullwidth(&text)
                } else {
                    text
                };

                // Commit and prepare for auto-suggestion
                let committed_text = text.clone();
                if !text.is_empty() {
                    self.context.commit_text = text;
                }
                self.reset();

                // Auto-enter suggestion mode after reset if enabled
                self.maybe_auto_suggest(&committed_text);

                // No auxiliary text after reset (inactive)
                KeyResult::Handled
            }
            EditorResult::ModeSwitch(mode) => {
                // Switch to new mode
                self.session.set_mode(mode);
                self.session.sync_to_context(&mut self.context);
                self.update_auxiliary_text();
                KeyResult::Handled
            }
            EditorResult::PassThrough => KeyResult::NotHandled,
        }
    }

    /// Update auxiliary text based on current mode and state.
    fn update_auxiliary_text(&mut self) {
        if !self.session.is_active() && self.session.mode() != InputMode::Passthrough {
            self.context.auxiliary_text.clear();
            return;
        }

        let aux_text = match self.session.mode() {
            InputMode::Init => String::new(),
            InputMode::Phonetic => {
                let num_candidates = self.session.candidates().len();
                if num_candidates > 0 {
                    format!("拼音 | {} 个候选 | Space/数字选择", num_candidates)
                } else {
                    "拼音 | 输入拼音...".to_string()
                }
            }
            InputMode::Punctuation => "标点 | 数字/Space选择 | Esc取消".to_string(),
            InputMode::Suggestion => {
                let num_candidates = self.session.candidates().len();
                if num_candidates > 0 {
                    let context = self.suggestion_editor.context();
                    format!(
                        "预测 [{}...] | {} 个建议 | Space/数字选择 | Esc取消",
                        context.chars().take(3).collect::<String>(),
                        num_candidates
                    )
                } else {
                    "预测 | 无建议".to_string()
                }
            }
            InputMode::Passthrough => "直通模式 | Shift_lock切换".to_string(),
        };

        self.context.auxiliary_text = aux_text;
    }

    // ========== Configuration Management API ==========

    /// Toggle full-width mode on/off.
    pub fn toggle_fullwidth(&mut self) {
        self.phonetic_editor
            .backend()
            .config_mut()
            .toggle_fullwidth();
    }

    /// Set full-width mode explicitly.
    pub fn set_fullwidth(&mut self, enabled: bool) {
        self.phonetic_editor
            .backend()
            .config_mut()
            .set_fullwidth(enabled);
    }

    /// Check if full-width mode is enabled.
    pub fn is_fullwidth(&self) -> bool {
        self.phonetic_editor.backend().config().is_fullwidth()
    }

    /// Set the selection keys string (e.g., "asdfghjkl" or "123456789").
    pub fn set_select_keys(&mut self, keys: &str) {
        self.phonetic_editor
            .backend()
            .config_mut()
            .set_select_keys(keys);
    }

    /// Get the current selection keys.
    pub fn get_select_keys(&self) -> String {
        self.phonetic_editor
            .backend()
            .config()
            .get_select_keys()
            .to_string()
    }

    /// Add a phrase to the mask list (hide from suggestions).
    pub fn mask_phrase(&mut self, phrase: &str) {
        self.phonetic_editor
            .backend()
            .config_mut()
            .mask_phrase(phrase);
    }

    /// Remove a phrase from the mask list (allow in suggestions).
    pub fn unmask_phrase(&mut self, phrase: &str) -> bool {
        self.phonetic_editor
            .backend()
            .config_mut()
            .unmask_phrase(phrase)
    }

    /// Check if a phrase is masked.
    pub fn is_masked(&self, phrase: &str) -> bool {
        self.phonetic_editor.backend().config().is_masked(phrase)
    }

    /// Get all masked phrases.
    pub fn get_masked_phrases(&self) -> Vec<String> {
        self.phonetic_editor.backend().config().get_masked_phrases()
    }
}

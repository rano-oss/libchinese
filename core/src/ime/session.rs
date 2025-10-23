//! IME session management.
//!
//! The `ImeSession` struct combines all the IME components (input buffer,
//! composition, candidates) into a cohesive session that tracks state across
//! multiple key events.

use super::input_buffer::InputBuffer;
use super::composition::Composition;
use super::candidates::CandidateList;
use super::context::ImeContext;

/// Current input mode of the IME session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Initial state, no input yet
    Init,
    /// Phonetic input (pinyin/zhuyin)
    Phonetic,
    /// Punctuation input
    Punctuation,
    /// Suggestion/prediction mode
    Suggestion,
    /// Passthrough mode (keys not processed by IME)
    Passthrough,
}

impl Default for InputMode {
    fn default() -> Self {
        Self::Init
    }
}

/// IME session state combining all session components.
///
/// This struct manages the state across multiple key events. It contains:
/// - Input buffer for raw user input
/// - Composition for visual preedit display
/// - Candidate list for conversion options
/// - Current input mode
///
/// The session is separate from the backend engine - the engine provides
/// the linguistic intelligence, while the session manages UI state.
#[derive(Debug, Clone)]
pub struct ImeSession {
    /// Raw input buffer (e.g., "nihao")
    input_buffer: InputBuffer,

    /// Visual composition (e.g., "你好" with segments)
    composition: Composition,

    /// Available candidates
    candidates: CandidateList,

    /// Current input mode
    mode: InputMode,

    /// Whether the session is active (has state)
    active: bool,
}

impl ImeSession {
    /// Create a new empty session.
    pub fn new() -> Self {
        Self {
            input_buffer: InputBuffer::new(),
            composition: Composition::new(),
            candidates: CandidateList::with_page_size(5),
            mode: InputMode::Init,
            active: false,
        }
    }

    /// Create a session with specified candidate page size.
    pub fn with_page_size(page_size: usize) -> Self {
        Self {
            input_buffer: InputBuffer::new(),
            composition: Composition::new(),
            candidates: CandidateList::with_page_size(page_size),
            mode: InputMode::Init,
            active: false,
        }
    }

    /// Get the input buffer.
    pub fn input_buffer(&self) -> &InputBuffer {
        &self.input_buffer
    }

    /// Get a mutable reference to the input buffer.
    pub fn input_buffer_mut(&mut self) -> &mut InputBuffer {
        &mut self.input_buffer
    }

    /// Get the composition.
    pub fn composition(&self) -> &Composition {
        &self.composition
    }

    /// Get a mutable reference to the composition.
    pub fn composition_mut(&mut self) -> &mut Composition {
        &mut self.composition
    }

    /// Get the candidate list.
    pub fn candidates(&self) -> &CandidateList {
        &self.candidates
    }

    /// Get a mutable reference to the candidate list.
    pub fn candidates_mut(&mut self) -> &mut CandidateList {
        &mut self.candidates
    }

    /// Get the current input mode.
    pub fn mode(&self) -> InputMode {
        self.mode
    }

    /// Set the input mode.
    pub fn set_mode(&mut self, mode: InputMode) {
        self.mode = mode;
    }

    /// Check if the session is active (has input state).
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Mark the session as active.
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Clear all session state and return to Init mode.
    pub fn clear(&mut self) {
        self.input_buffer.clear();
        self.composition.clear();
        self.candidates.clear();
        self.mode = InputMode::Init;
        self.active = false;
    }

    /// Update the composition from the current input buffer.
    /// This is typically called after the input buffer changes.
    pub fn update_composition_from_input(&mut self) {
        let input_text = self.input_buffer.text().to_string();
        self.composition = Composition::from_text(input_text);
    }

    /// Sync session state to an ImeContext for platform communication.
    ///
    /// This method reads the session state and populates the ImeContext
    /// fields so the platform can display the IME UI.
    pub fn sync_to_context(&self, context: &mut ImeContext) {
        // Clear previous state
        context.preedit_text.clear();
        context.candidates.clear();
        context.auxiliary_text.clear();

        // Set preedit from composition
        if !self.composition.preedit.is_empty() {
            context.preedit_text = self.composition.preedit.clone();
            context.preedit_cursor = self.composition.cursor;
        }

        // Set candidates
        let page_candidates = self.candidates.current_page_candidates();
        context.candidates = page_candidates
            .iter()
            .map(|c| c.text.clone())
            .collect();
        context.candidate_cursor = self.candidates.cursor();

        // Set auxiliary text (page indicator if multi-page)
        if self.candidates.num_pages() > 1 {
            let current_page = self.candidates.current_page() + 1; // 1-indexed for display
            let total_pages = self.candidates.num_pages();
            context.auxiliary_text = format!("第{}页/{}", current_page, total_pages);
        }
    }

    /// Update session state from an ImeContext.
    ///
    /// This is less common but allows the platform to inject state if needed.
    pub fn sync_from_context(&mut self, context: &ImeContext) {
        // Update composition
        if !context.preedit_text.is_empty() {
            self.composition = Composition::with_cursor(
                context.preedit_text.clone(),
                context.preedit_cursor
            );
        }
    }
}

impl Default for ImeSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Candidate;

    #[test]
    fn test_new_session() {
        let session = ImeSession::new();
        assert_eq!(session.mode(), InputMode::Init);
        assert!(!session.is_active());
        assert_eq!(session.input_buffer().text(), "");
        assert_eq!(session.composition().preedit, "");
        assert!(session.candidates().is_empty());
    }

    #[test]
    fn test_activate_and_clear() {
        let mut session = ImeSession::new();
        assert!(!session.is_active());

        session.activate();
        assert!(session.is_active());

        session.clear();
        assert!(!session.is_active());
        assert_eq!(session.mode(), InputMode::Init);
    }

    #[test]
    fn test_mode_transitions() {
        let mut session = ImeSession::new();
        assert_eq!(session.mode(), InputMode::Init);

        session.set_mode(InputMode::Phonetic);
        assert_eq!(session.mode(), InputMode::Phonetic);

        session.set_mode(InputMode::Punctuation);
        assert_eq!(session.mode(), InputMode::Punctuation);

        session.clear();
        assert_eq!(session.mode(), InputMode::Init);
    }

    #[test]
    fn test_update_composition_from_input() {
        let mut session = ImeSession::new();

        session.input_buffer_mut().insert_str("nihao");
        session.update_composition_from_input();

        assert_eq!(session.composition().preedit, "nihao");
    }

    #[test]
    fn test_sync_to_context() {
        let mut session = ImeSession::new();
        let mut context = ImeContext::new();

        // Set up session state
        session.input_buffer_mut().insert_str("nihao");
        session.update_composition_from_input();
        session.composition_mut().add_segment(0..2, false);
        session.composition_mut().add_segment(3..5, false);

        let candidates = vec![
            Candidate::new("你好", 1.0),
            Candidate::new("尼好", 0.9),
            Candidate::new("你豪", 0.8),
        ];
        session.candidates_mut().set_candidates(candidates);

        // Sync to context
        session.sync_to_context(&mut context);

        assert_eq!(context.preedit_text, "nihao");
        assert_eq!(context.candidates.len(), 3);
        assert_eq!(context.candidates[0], "你好");
        assert_eq!(context.candidates[1], "尼好");
        assert_eq!(context.candidate_cursor, 0);
    }

    #[test]
    fn test_sync_to_context_with_paging() {
        let mut session = ImeSession::with_page_size(2);
        let mut context = ImeContext::new();

        // Create 5 candidates (3 pages with page size 2)
        let candidates: Vec<_> = (0..5)
            .map(|i| Candidate::new(format!("候选{}", i), 1.0 - i as f32 * 0.1))
            .collect();
        session.candidates_mut().set_candidates(candidates);

        // First page
        session.sync_to_context(&mut context);
        assert_eq!(context.candidates.len(), 2);
        assert_eq!(context.candidates[0], "候选0");
        assert_eq!(context.auxiliary_text, "第1页/3");

        // Second page
        session.candidates_mut().page_down();
        session.sync_to_context(&mut context);
        assert_eq!(context.candidates.len(), 2);
        assert_eq!(context.candidates[0], "候选2");
        assert_eq!(context.auxiliary_text, "第2页/3");

        // Third page (only 1 candidate)
        session.candidates_mut().page_down();
        session.sync_to_context(&mut context);
        assert_eq!(context.candidates.len(), 1);
        assert_eq!(context.candidates[0], "候选4");
        assert_eq!(context.auxiliary_text, "第3页/3");
    }

    #[test]
    fn test_sync_from_context() {
        let mut session = ImeSession::new();
        let mut context = ImeContext::new();

        // Set up context
        context.preedit_text = "你好".to_string();
        context.preedit_cursor = 3;

        // Sync from context
        session.sync_from_context(&context);

        assert_eq!(session.composition().preedit, "你好");
        assert_eq!(session.composition().cursor, 3);
    }

    #[test]
    fn test_clear_removes_all_state() {
        let mut session = ImeSession::new();

        // Set up state
        session.activate();
        session.set_mode(InputMode::Phonetic);
        session.input_buffer_mut().insert_str("test");
        session.update_composition_from_input();
        let candidates = vec![Candidate::new("测试", 1.0)];
        session.candidates_mut().set_candidates(candidates);

        // Clear
        session.clear();

        // Verify everything is cleared
        assert!(!session.is_active());
        assert_eq!(session.mode(), InputMode::Init);
        assert_eq!(session.input_buffer().text(), "");
        assert_eq!(session.composition().preedit, "");
        assert!(session.candidates().is_empty());
    }
}

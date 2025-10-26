//! IME session management.
//!
//! The `ImeSession` struct combines all the IME components (input buffer,
//! composition, candidates) into a cohesive session that tracks state across
//! multiple key events.

use crate::candidate::CandidateList;
use crate::composition::Composition;
use crate::context::ImeContext;
use crate::input_buffer::InputBuffer;

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
        context.candidates = page_candidates.iter().map(|c| c.text.clone()).collect();
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
            self.composition =
                Composition::with_cursor(context.preedit_text.clone(), context.preedit_cursor);
        }
    }
}

impl Default for ImeSession {
    fn default() -> Self {
        Self::new()
    }
}

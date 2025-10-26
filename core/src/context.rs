//! IME context for platform communication.
//!
//! The `ImeContext` struct is a simple data container with public fields that
//! platforms use to communicate with the IME. After calling `process_key()` on
//! a session, the platform reads these fields to update the UI.
//!
//! Design philosophy: Zero abstraction - just data transfer. No callbacks, no
//! traits, no generics. Platform code reads/writes fields directly.

/// Input purpose hint for context-aware input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputPurpose {
    /// Free-form text input (default)
    FreeForm,
    /// Email address
    Email,
    /// URL
    Url,
    /// Password (disable suggestions)
    Password,
    /// Number input
    Number,
    /// Phone number
    Phone,
    /// Terminal/command line
    Terminal,
}

impl Default for InputPurpose {
    fn default() -> Self {
        Self::FreeForm
    }
}

/// IME context for platform communication.
///
/// This struct contains all the information the platform needs to display
/// the IME state. After processing a key event, the platform reads these
/// fields to update preedit text, candidates, and commit text.
///
/// # Fields
///
/// - `preedit_text`: Text being composed (displayed with underline)
/// - `preedit_cursor`: Cursor position within preedit (byte offset)
/// - `commit_text`: Text to commit to application (consume and clear)
/// - `candidates`: List of available candidates for current input
/// - `candidate_cursor`: Which candidate is highlighted (0-based index)
/// - `auxiliary_text`: Optional hint text (e.g., "第2页" for page indicator)
/// - `input_purpose`: Hint about what kind of input is expected
#[derive(Debug, Clone, Default)]
pub struct ImeContext {
    /// Text being composed (preedit/候选)
    pub preedit_text: String,

    /// Cursor position within preedit text (byte offset)
    pub preedit_cursor: usize,

    /// Text to commit to the application
    pub commit_text: String,

    /// List of candidate strings to display
    pub candidates: Vec<String>,

    /// Currently highlighted candidate index (0-based)
    pub candidate_cursor: usize,

    /// Auxiliary text for UI hints (e.g., page numbers)
    pub auxiliary_text: String,

    /// Input purpose hint for context-aware behavior
    pub input_purpose: InputPurpose,
}

impl ImeContext {
    /// Create a new empty IME context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all state (preedit, candidates, auxiliary).
    /// Does NOT clear commit_text (platform should consume it first).
    pub fn clear(&mut self) {
        self.preedit_text.clear();
        self.preedit_cursor = 0;
        self.candidates.clear();
        self.candidate_cursor = 0;
        self.auxiliary_text.clear();
    }

    /// Take the commit text, leaving it empty.
    /// This is a convenience for platforms that want to consume commit_text.
    pub fn take_commit(&mut self) -> String {
        std::mem::take(&mut self.commit_text)
    }

    /// Check if there's any visible state (preedit or candidates).
    pub fn has_visible_state(&self) -> bool {
        !self.preedit_text.is_empty() || !self.candidates.is_empty()
    }

    /// Check if there's text to commit.
    pub fn has_commit(&self) -> bool {
        !self.commit_text.is_empty()
    }

    /// Set the input purpose.
    pub fn set_input_purpose(&mut self, purpose: InputPurpose) {
        self.input_purpose = purpose;
    }
}

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
///
/// # Example
///
/// ```
/// use libchinese_core::ImeContext;
///
/// let mut context = ImeContext::new();
/// context.preedit_text = "ni'hao".to_string();
/// context.candidates = vec!["你好".to_string(), "尼好".to_string()];
///
/// // Platform reads fields
/// if !context.commit_text.is_empty() {
///     // Commit to application
///     context.commit_text.clear();
/// }
///
/// if !context.preedit_text.is_empty() {
///     // Show preedit with cursor
///     let cursor_pos = context.preedit_cursor;
/// }
/// ```
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_context() {
        let ctx = ImeContext::new();
        assert!(ctx.preedit_text.is_empty());
        assert_eq!(ctx.preedit_cursor, 0);
        assert!(ctx.commit_text.is_empty());
        assert!(ctx.candidates.is_empty());
        assert_eq!(ctx.candidate_cursor, 0);
        assert!(ctx.auxiliary_text.is_empty());
        assert_eq!(ctx.input_purpose, InputPurpose::FreeForm);
    }

    #[test]
    fn test_clear() {
        let mut ctx = ImeContext::new();
        ctx.preedit_text = "nihao".to_string();
        ctx.preedit_cursor = 5;
        ctx.commit_text = "你好".to_string();
        ctx.candidates = vec!["你好".to_string()];
        ctx.candidate_cursor = 1;
        ctx.auxiliary_text = "Page 1".to_string();

        ctx.clear();

        // Clear removes preedit and candidates but NOT commit_text
        assert!(ctx.preedit_text.is_empty());
        assert_eq!(ctx.preedit_cursor, 0);
        assert_eq!(ctx.commit_text, "你好"); // Not cleared
        assert!(ctx.candidates.is_empty());
        assert_eq!(ctx.candidate_cursor, 0);
        assert!(ctx.auxiliary_text.is_empty());
    }

    #[test]
    fn test_take_commit() {
        let mut ctx = ImeContext::new();
        ctx.commit_text = "你好".to_string();

        let commit = ctx.take_commit();
        assert_eq!(commit, "你好");
        assert!(ctx.commit_text.is_empty());

        // Taking again returns empty
        let commit2 = ctx.take_commit();
        assert!(commit2.is_empty());
    }

    #[test]
    fn test_has_visible_state() {
        let mut ctx = ImeContext::new();
        assert!(!ctx.has_visible_state());

        ctx.preedit_text = "nihao".to_string();
        assert!(ctx.has_visible_state());

        ctx.preedit_text.clear();
        ctx.candidates = vec!["你好".to_string()];
        assert!(ctx.has_visible_state());

        ctx.clear();
        assert!(!ctx.has_visible_state());
    }

    #[test]
    fn test_has_commit() {
        let mut ctx = ImeContext::new();
        assert!(!ctx.has_commit());

        ctx.commit_text = "你好".to_string();
        assert!(ctx.has_commit());

        ctx.take_commit();
        assert!(!ctx.has_commit());
    }

    #[test]
    fn test_input_purpose() {
        let mut ctx = ImeContext::new();
        assert_eq!(ctx.input_purpose, InputPurpose::FreeForm);

        ctx.set_input_purpose(InputPurpose::Email);
        assert_eq!(ctx.input_purpose, InputPurpose::Email);

        ctx.set_input_purpose(InputPurpose::Password);
        assert_eq!(ctx.input_purpose, InputPurpose::Password);
    }
}

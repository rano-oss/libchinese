//! Preedit composition with segments for display.
//!
//! The composition represents the visual display of the input being composed,
//! including the converted text and segment boundaries. For example, typing
//! "nihao" might show "你好" as the preedit, with segments marking each word.

use std::ops::Range;

/// A segment in the preedit composition.
///
/// Segments mark boundaries between different parts of the composition,
/// such as individual words or syllables that can be converted independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Byte range in the preedit text
    pub range: Range<usize>,
    /// Whether this segment has been confirmed by the user
    pub confirmed: bool,
}

/// Preedit composition for display.
///
/// This represents the visual text shown to the user during input composition,
/// along with cursor position and segment boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composition {
    /// The preedit text to display (e.g., "你好")
    pub preedit: String,
    /// Cursor position in the preedit (byte offset)
    pub cursor: usize,
    /// Segments marking conversion boundaries
    pub segments: Vec<Segment>,
}

impl Composition {
    /// Create a new empty composition.
    pub fn new() -> Self {
        Self {
            preedit: String::new(),
            cursor: 0,
            segments: Vec::new(),
        }
    }

    /// Create a composition with simple text (no segments).
    pub fn from_text(text: String) -> Self {
        let cursor = text.len();
        Self {
            preedit: text,
            cursor,
            segments: Vec::new(),
        }
    }

    /// Create a composition with text and cursor position.
    pub fn with_cursor(text: String, cursor: usize) -> Self {
        Self {
            preedit: text,
            cursor,
            segments: Vec::new(),
        }
    }

    /// Check if the composition is empty.
    pub fn is_empty(&self) -> bool {
        self.preedit.is_empty()
    }

    /// Clear the composition.
    pub fn clear(&mut self) {
        self.preedit.clear();
        self.cursor = 0;
        self.segments.clear();
    }

    /// Get the length of the preedit text in bytes.
    pub fn len(&self) -> usize {
        self.preedit.len()
    }

    /// Set the preedit text.
    pub fn set_text(&mut self, text: String) {
        self.preedit = text;
        self.cursor = self.preedit.len();
    }

    /// Set the cursor position.
    pub fn set_cursor(&mut self, cursor: usize) {
        if cursor <= self.preedit.len() {
            self.cursor = cursor;
        }
    }

    /// Add a segment.
    pub fn add_segment(&mut self, range: Range<usize>, confirmed: bool) {
        self.segments.push(Segment { range, confirmed });
    }

    /// Get the segment at the cursor position, if any.
    pub fn segment_at_cursor(&self) -> Option<&Segment> {
        self.segments
            .iter()
            .find(|seg| seg.range.contains(&self.cursor))
    }

    /// Get mutable reference to segment at cursor.
    pub fn segment_at_cursor_mut(&mut self) -> Option<&mut Segment> {
        let cursor = self.cursor;
        self.segments
            .iter_mut()
            .find(|seg| seg.range.contains(&cursor))
    }

    /// Confirm all segments.
    pub fn confirm_all(&mut self) {
        for seg in &mut self.segments {
            seg.confirmed = true;
        }
    }

    /// Get text of a specific segment.
    pub fn segment_text(&self, segment: &Segment) -> &str {
        &self.preedit[segment.range.clone()]
    }
}

impl Default for Composition {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let comp = Composition::new();
        assert!(comp.is_empty());
        assert_eq!(comp.cursor, 0);
        assert!(comp.segments.is_empty());
    }

    #[test]
    fn test_from_text() {
        let comp = Composition::from_text("你好".to_string());
        assert_eq!(comp.preedit, "你好");
        assert_eq!(comp.cursor, 6); // UTF-8: 3 + 3 bytes
        assert!(comp.segments.is_empty());
    }

    #[test]
    fn test_with_cursor() {
        let comp = Composition::with_cursor("你好".to_string(), 3);
        assert_eq!(comp.preedit, "你好");
        assert_eq!(comp.cursor, 3);
    }

    #[test]
    fn test_set_text() {
        let mut comp = Composition::new();
        comp.set_text("你好".to_string());

        assert_eq!(comp.preedit, "你好");
        assert_eq!(comp.cursor, 6);
    }

    #[test]
    fn test_segments() {
        let mut comp = Composition::from_text("你好世界".to_string());

        // Add segments for "你好" and "世界"
        comp.add_segment(0..6, false);
        comp.add_segment(6..12, false);

        assert_eq!(comp.segments.len(), 2);
        assert_eq!(comp.segments[0].range, 0..6);
        assert_eq!(comp.segments[1].range, 6..12);
    }

    #[test]
    fn test_segment_at_cursor() {
        let mut comp = Composition::from_text("你好世界".to_string());
        comp.add_segment(0..6, false);
        comp.add_segment(6..12, false);

        // Cursor at start of first segment
        comp.set_cursor(0);
        assert!(comp.segment_at_cursor().is_some());
        assert_eq!(comp.segment_at_cursor().unwrap().range, 0..6);

        // Cursor in second segment
        comp.set_cursor(8);
        assert!(comp.segment_at_cursor().is_some());
        assert_eq!(comp.segment_at_cursor().unwrap().range, 6..12);

        // Cursor at end (no segment)
        comp.set_cursor(12);
        assert!(comp.segment_at_cursor().is_none());
    }

    #[test]
    fn test_confirm_segments() {
        let mut comp = Composition::from_text("你好".to_string());
        comp.add_segment(0..6, false);

        assert!(!comp.segments[0].confirmed);

        comp.confirm_all();
        assert!(comp.segments[0].confirmed);
    }

    #[test]
    fn test_segment_text() {
        let mut comp = Composition::from_text("你好世界".to_string());
        comp.add_segment(0..6, false);
        comp.add_segment(6..12, false);

        assert_eq!(comp.segment_text(&comp.segments[0]), "你好");
        assert_eq!(comp.segment_text(&comp.segments[1]), "世界");
    }

    #[test]
    fn test_clear() {
        let mut comp = Composition::from_text("你好".to_string());
        comp.add_segment(0..6, false);

        comp.clear();
        assert!(comp.is_empty());
        assert_eq!(comp.cursor, 0);
        assert!(comp.segments.is_empty());
    }
}

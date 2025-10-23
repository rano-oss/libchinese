//! Input buffer with cursor tracking for IME input.
//!
//! The input buffer stores the raw input characters (e.g., "nihao") and tracks
//! the cursor position within this buffer. This is separate from the visual
//! preedit composition which shows converted text.

/// Input buffer tracking raw input and cursor position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputBuffer {
    text: String,
    cursor: usize, // Byte offset, not char offset
}

impl InputBuffer {
    /// Create a new empty input buffer.
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
        }
    }

    /// Get the raw input text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get the cursor position (byte offset).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Get the length of the buffer in bytes.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Clear the buffer and reset cursor.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    /// Insert a string at the cursor position.
    pub fn insert_str(&mut self, s: &str) {
        self.text.insert_str(self.cursor, s);
        self.cursor += s.len();
    }

    /// Delete the character before the cursor (backspace).
    /// Returns true if a character was deleted.
    pub fn delete_before(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }

        // Find the previous character boundary
        let mut prev = self.cursor;
        while prev > 0 {
            prev -= 1;
            if self.text.is_char_boundary(prev) {
                break;
            }
        }

        self.text.remove(prev);
        self.cursor = prev;
        true
    }

    /// Delete the character after the cursor (delete key).
    /// Returns true if a character was deleted.
    pub fn delete_after(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }

        // Remove character at cursor
        if self.text.is_char_boundary(self.cursor) {
            self.text.remove(self.cursor);
            true
        } else {
            false
        }
    }

    /// Move cursor to the left by one character.
    /// Returns true if cursor moved.
    pub fn move_left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }

        // Find the previous character boundary
        let mut prev = self.cursor;
        while prev > 0 {
            prev -= 1;
            if self.text.is_char_boundary(prev) {
                self.cursor = prev;
                return true;
            }
        }
        false
    }

    /// Move cursor to the right by one character.
    /// Returns true if cursor moved.
    pub fn move_right(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }

        // Find the next character boundary
        let mut next = self.cursor + 1;
        while next < self.text.len() && !self.text.is_char_boundary(next) {
            next += 1;
        }
        if next <= self.text.len() {
            self.cursor = next;
            true
        } else {
            false
        }
    }

    /// Move cursor to the beginning.
    pub fn move_to_start(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to the end.
    pub fn move_to_end(&mut self) {
        self.cursor = self.text.len();
    }

    /// Set the cursor position (must be at a character boundary).
    pub fn set_cursor(&mut self, pos: usize) -> bool {
        if pos <= self.text.len() && self.text.is_char_boundary(pos) {
            self.cursor = pos;
            true
        } else {
            false
        }
    }
}

impl Default for InputBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer() {
        let buf = InputBuffer::new();
        assert_eq!(buf.text(), "");
        assert_eq!(buf.cursor(), 0);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_insert_char() {
        let mut buf = InputBuffer::new();
        buf.insert_char('n');
        assert_eq!(buf.text(), "n");
        assert_eq!(buf.cursor(), 1);

        buf.insert_char('i');
        assert_eq!(buf.text(), "ni");
        assert_eq!(buf.cursor(), 2);
    }

    #[test]
    fn test_insert_str() {
        let mut buf = InputBuffer::new();
        buf.insert_str("nihao");
        assert_eq!(buf.text(), "nihao");
        assert_eq!(buf.cursor(), 5);
    }

    #[test]
    fn test_delete_before() {
        let mut buf = InputBuffer::new();
        buf.insert_str("nihao");
        
        assert!(buf.delete_before());
        assert_eq!(buf.text(), "niha");
        assert_eq!(buf.cursor(), 4);

        assert!(buf.delete_before());
        assert_eq!(buf.text(), "nih");
        assert_eq!(buf.cursor(), 3);
    }

    #[test]
    fn test_delete_before_at_start() {
        let mut buf = InputBuffer::new();
        buf.insert_str("ni");
        buf.move_to_start();
        
        assert!(!buf.delete_before());
        assert_eq!(buf.text(), "ni");
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn test_delete_after() {
        let mut buf = InputBuffer::new();
        buf.insert_str("nihao");
        buf.move_to_start();
        
        assert!(buf.delete_after());
        assert_eq!(buf.text(), "ihao");
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn test_cursor_movement() {
        let mut buf = InputBuffer::new();
        buf.insert_str("nihao");
        
        // At end
        assert_eq!(buf.cursor(), 5);
        
        // Move left
        assert!(buf.move_left());
        assert_eq!(buf.cursor(), 4);
        
        // Move right
        assert!(buf.move_right());
        assert_eq!(buf.cursor(), 5);
        
        // Can't move right at end
        assert!(!buf.move_right());
        assert_eq!(buf.cursor(), 5);
        
        // Move to start
        buf.move_to_start();
        assert_eq!(buf.cursor(), 0);
        
        // Can't move left at start
        assert!(!buf.move_left());
        assert_eq!(buf.cursor(), 0);
    }

    #[test]
    fn test_unicode() {
        let mut buf = InputBuffer::new();
        buf.insert_char('你');
        assert_eq!(buf.text(), "你");
        assert_eq!(buf.cursor(), 3); // UTF-8: 3 bytes
        
        buf.insert_char('好');
        assert_eq!(buf.text(), "你好");
        assert_eq!(buf.cursor(), 6); // 3 + 3 bytes
        
        assert!(buf.delete_before());
        assert_eq!(buf.text(), "你");
        assert_eq!(buf.cursor(), 3);
    }

    #[test]
    fn test_clear() {
        let mut buf = InputBuffer::new();
        buf.insert_str("nihao");
        buf.clear();
        
        assert_eq!(buf.text(), "");
        assert_eq!(buf.cursor(), 0);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_insert_at_position() {
        let mut buf = InputBuffer::new();
        buf.insert_str("niao");
        buf.set_cursor(2);
        buf.insert_char('h');
        
        assert_eq!(buf.text(), "nihao");
        assert_eq!(buf.cursor(), 3);
    }
}

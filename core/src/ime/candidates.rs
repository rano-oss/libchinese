//! Candidate list management with paging and cursor navigation.
//!
//! This module provides data structures for managing IME candidates with pagination,
//! cursor navigation, and selection. It handles the display logic for showing
//! available conversion options to the user.

use std::ops::Range;

// Re-export Candidate from the parent crate
pub use crate::Candidate;

/// A paginated list of candidates with cursor navigation.
#[derive(Debug, Clone)]
pub struct CandidateList {
    /// All available candidates
    candidates: Vec<Candidate>,
    
    /// Number of candidates per page
    page_size: usize,
    
    /// Current page index (0-based)
    current_page: usize,
    
    /// Cursor position within the current page (0-based)
    cursor: usize,
}

impl CandidateList {
    /// Create a new empty candidate list.
    pub fn new() -> Self {
        Self {
            candidates: Vec::new(),
            page_size: 5,
            current_page: 0,
            cursor: 0,
        }
    }

    /// Create a candidate list with specified page size.
    pub fn with_page_size(page_size: usize) -> Self {
        Self {
            candidates: Vec::new(),
            page_size: page_size.max(1), // Ensure at least 1
            current_page: 0,
            cursor: 0,
        }
    }

    /// Create a candidate list from a vector of candidates.
    pub fn from_candidates(candidates: Vec<Candidate>) -> Self {
        Self {
            candidates,
            page_size: 5,
            current_page: 0,
            cursor: 0,
        }
    }

    /// Set the page size.
    pub fn set_page_size(&mut self, page_size: usize) {
        self.page_size = page_size.max(1);
        // Reset to first page if current page is now out of bounds
        if self.current_page >= self.num_pages() && self.num_pages() > 0 {
            self.current_page = 0;
        }
        // Reset cursor if it's out of bounds
        if self.cursor >= self.current_page_len() && self.current_page_len() > 0 {
            self.cursor = 0;
        }
    }

    /// Get the page size.
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// Set the candidates, resetting pagination state.
    pub fn set_candidates(&mut self, candidates: Vec<Candidate>) {
        self.candidates = candidates;
        self.current_page = 0;
        self.cursor = 0;
    }

    /// Get all candidates.
    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    /// Get the total number of candidates.
    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// Check if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// Get the total number of pages.
    pub fn num_pages(&self) -> usize {
        if self.candidates.is_empty() {
            0
        } else {
            (self.candidates.len() + self.page_size - 1) / self.page_size
        }
    }

    /// Get the current page index (0-based).
    pub fn current_page(&self) -> usize {
        self.current_page
    }

    /// Get the cursor position within the current page (0-based).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Get the number of candidates on the current page.
    fn current_page_len(&self) -> usize {
        let start = self.current_page * self.page_size;
        let end = (start + self.page_size).min(self.candidates.len());
        if end > start { end - start } else { 0 }
    }

    /// Get the range of candidates for the current page.
    fn current_page_range(&self) -> Range<usize> {
        let start = self.current_page * self.page_size;
        let end = (start + self.page_size).min(self.candidates.len());
        start..end
    }

    /// Get the candidates for the current page.
    pub fn current_page_candidates(&self) -> &[Candidate] {
        if self.candidates.is_empty() {
            return &[];
        }
        let range = self.current_page_range();
        &self.candidates[range]
    }

    /// Get the currently selected candidate (under cursor).
    pub fn selected_candidate(&self) -> Option<&Candidate> {
        let page_candidates = self.current_page_candidates();
        page_candidates.get(self.cursor)
    }

    /// Get the global index of the currently selected candidate.
    pub fn selected_index(&self) -> Option<usize> {
        if self.is_empty() {
            return None;
        }
        let global_index = self.current_page * self.page_size + self.cursor;
        if global_index < self.candidates.len() {
            Some(global_index)
        } else {
            None
        }
    }

    /// Move cursor up (to previous candidate on current page).
    /// Returns true if the cursor moved.
    pub fn cursor_up(&mut self) -> bool {
        if self.cursor > 0 {
            self.cursor -= 1;
            true
        } else {
            false
        }
    }

    /// Move cursor down (to next candidate on current page).
    /// Returns true if the cursor moved.
    pub fn cursor_down(&mut self) -> bool {
        let page_len = self.current_page_len();
        if page_len > 0 && self.cursor < page_len - 1 {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    /// Move to the previous page.
    /// Returns true if the page changed.
    pub fn page_up(&mut self) -> bool {
        if self.current_page > 0 {
            self.current_page -= 1;
            // Keep cursor valid
            let page_len = self.current_page_len();
            if page_len > 0 && self.cursor >= page_len {
                self.cursor = page_len - 1;
            }
            true
        } else {
            false
        }
    }

    /// Move to the next page.
    /// Returns true if the page changed.
    pub fn page_down(&mut self) -> bool {
        let num_pages = self.num_pages();
        if num_pages > 0 && self.current_page < num_pages - 1 {
            self.current_page += 1;
            // Keep cursor valid
            let page_len = self.current_page_len();
            if page_len > 0 && self.cursor >= page_len {
                self.cursor = page_len - 1;
            }
            true
        } else {
            false
        }
    }

    /// Select a candidate by index within the current page.
    /// Returns the selected candidate if the index is valid.
    pub fn select_by_index(&mut self, page_index: usize) -> Option<&Candidate> {
        let page_len = self.current_page_len();
        if page_index < page_len {
            self.cursor = page_index;
            self.selected_candidate()
        } else {
            None
        }
    }

    /// Clear the candidate list.
    pub fn clear(&mut self) {
        self.candidates.clear();
        self.current_page = 0;
        self.cursor = 0;
    }

    /// Reset pagination state (go to first page, first candidate).
    pub fn reset(&mut self) {
        self.current_page = 0;
        self.cursor = 0;
    }
}

impl Default for CandidateList {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_candidates(n: usize) -> Vec<Candidate> {
        (0..n).map(|i| Candidate::new(format!("候选{}", i), 1.0 - i as f32 * 0.1)).collect()
    }

    #[test]
    fn test_new_list() {
        let list = CandidateList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert_eq!(list.page_size(), 5);
        assert_eq!(list.num_pages(), 0);
    }

    #[test]
    fn test_from_candidates() {
        let candidates = test_candidates(3);
        let list = CandidateList::from_candidates(candidates);
        assert_eq!(list.len(), 3);
        assert_eq!(list.num_pages(), 1);
        assert_eq!(list.current_page(), 0);
        assert_eq!(list.cursor(), 0);
    }

    #[test]
    fn test_pagination() {
        let mut list = CandidateList::with_page_size(3);
        list.set_candidates(test_candidates(10));
        
        assert_eq!(list.len(), 10);
        assert_eq!(list.page_size(), 3);
        assert_eq!(list.num_pages(), 4); // 10 items, 3 per page = 4 pages
        
        // First page has 3 items
        assert_eq!(list.current_page_candidates().len(), 3);
        assert_eq!(list.current_page_candidates()[0].text, "候选0");
        
        // Move to page 2
        assert!(list.page_down());
        assert_eq!(list.current_page(), 1);
        assert_eq!(list.current_page_candidates().len(), 3);
        assert_eq!(list.current_page_candidates()[0].text, "候选3");
        
        // Move to page 3
        assert!(list.page_down());
        assert_eq!(list.current_page(), 2);
        
        // Move to page 4 (last page, only 1 item)
        assert!(list.page_down());
        assert_eq!(list.current_page(), 3);
        assert_eq!(list.current_page_candidates().len(), 1);
        assert_eq!(list.current_page_candidates()[0].text, "候选9");
        
        // Can't page down anymore
        assert!(!list.page_down());
        assert_eq!(list.current_page(), 3);
    }

    #[test]
    fn test_cursor_navigation() {
        let mut list = CandidateList::with_page_size(3);
        list.set_candidates(test_candidates(5));
        
        assert_eq!(list.cursor(), 0);
        
        // Move cursor down
        assert!(list.cursor_down());
        assert_eq!(list.cursor(), 1);
        assert!(list.cursor_down());
        assert_eq!(list.cursor(), 2);
        
        // Can't move down on last item of page
        assert!(!list.cursor_down());
        assert_eq!(list.cursor(), 2);
        
        // Move cursor up
        assert!(list.cursor_up());
        assert_eq!(list.cursor(), 1);
        assert!(list.cursor_up());
        assert_eq!(list.cursor(), 0);
        
        // Can't move up from first item
        assert!(!list.cursor_up());
        assert_eq!(list.cursor(), 0);
    }

    #[test]
    fn test_selected_candidate() {
        let mut list = CandidateList::from_candidates(test_candidates(5));
        
        // Initially first candidate is selected
        assert_eq!(list.selected_candidate().unwrap().text, "候选0");
        assert_eq!(list.selected_index(), Some(0));
        
        // Move cursor and check selection
        list.cursor_down();
        assert_eq!(list.selected_candidate().unwrap().text, "候选1");
        assert_eq!(list.selected_index(), Some(1));
        
        list.cursor_down();
        assert_eq!(list.selected_candidate().unwrap().text, "候选2");
        assert_eq!(list.selected_index(), Some(2));
    }

    #[test]
    fn test_select_by_index() {
        let mut list = CandidateList::with_page_size(3);
        list.set_candidates(test_candidates(10));
        
        // Select within current page
        let candidate = list.select_by_index(2).unwrap();
        assert_eq!(candidate.text, "候选2");
        assert_eq!(list.cursor(), 2);
        
        // Invalid index returns None
        assert!(list.select_by_index(5).is_none());
        
        // Move to next page and select
        list.page_down();
        let candidate = list.select_by_index(1).unwrap();
        assert_eq!(candidate.text, "候选4");
    }

    #[test]
    fn test_cursor_preserved_across_pages() {
        let mut list = CandidateList::with_page_size(3);
        list.set_candidates(test_candidates(10));
        
        // Set cursor to position 2
        list.cursor_down();
        list.cursor_down();
        assert_eq!(list.cursor(), 2);
        
        // Move to next page (has 3 items), cursor should be valid
        list.page_down();
        assert_eq!(list.cursor(), 2);
        
        // Move to last page (has 1 item), cursor should be clamped
        list.page_down();
        list.page_down();
        assert_eq!(list.current_page(), 3);
        assert_eq!(list.cursor(), 0); // Clamped to last valid position
    }

    #[test]
    fn test_set_page_size() {
        let mut list = CandidateList::from_candidates(test_candidates(10));
        assert_eq!(list.page_size(), 5);
        assert_eq!(list.num_pages(), 2);
        
        // Change page size
        list.set_page_size(3);
        assert_eq!(list.page_size(), 3);
        assert_eq!(list.num_pages(), 4);
        assert_eq!(list.current_page(), 0); // Reset to first page
    }

    #[test]
    fn test_clear_and_reset() {
        let mut list = CandidateList::from_candidates(test_candidates(10));
        list.page_down();
        list.cursor_down();
        
        // Clear removes all candidates
        list.clear();
        assert!(list.is_empty());
        assert_eq!(list.current_page(), 0);
        assert_eq!(list.cursor(), 0);
        
        // Reset only resets pagination
        list.set_candidates(test_candidates(10));
        list.page_down();
        list.cursor_down();
        list.reset();
        assert_eq!(list.len(), 10);
        assert_eq!(list.current_page(), 0);
        assert_eq!(list.cursor(), 0);
    }

    #[test]
    fn test_candidate_with_score() {
        let candidate = Candidate::new("你好", 0.95);
        assert_eq!(candidate.text, "你好");
        assert_eq!(candidate.score, 0.95);
    }
}

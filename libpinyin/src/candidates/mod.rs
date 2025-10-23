//! Candidate list management with paging and cursor navigation.
//!
//! This module provides data structures for managing IME candidates with pagination,
//! cursor navigation, and selection. It handles the display logic for showing
//! available conversion options to the user.

pub mod list;

pub use list::{Candidate, CandidateList};

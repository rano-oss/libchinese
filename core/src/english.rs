//! English word dictionary for mixed Chinese–English suggestions.
//!
//! Wordlist format is compatible with common libpinyin distributions
//! (`word\tfreq`). This module only does lookup and local ranking among
//! English hits — how they appear relative to hanzi/emoji is up to the
//! engine merge policy or the UI.

use std::path::Path;

/// In-memory English dictionary supporting prefix lookup.
#[derive(Debug, Clone)]
pub struct EnglishDict {
    /// Words sorted by frequency (desc), then length, then text.
    entries: Vec<(String, f32)>,
}

/// Sensible default: ignore very short buffers (likely pure pinyin).
pub const MINIMAL_ENGLISH_CHARACTERS: usize = 4;
/// Cap English extras so they do not drown phonetic results.
pub const MAXIMAL_ENGLISH_CANDIDATES: usize = 2;

impl EnglishDict {
    /// Load from a wordlist file (`word\tfreq` per line).
    ///
    /// Lines starting with '#' or empty lines are skipped.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_str(&content))
    }

    /// Parse wordlist from string content.
    pub fn from_str(content: &str) -> Self {
        let mut entries: Vec<(String, f32)> = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split('\t');
            let Some(word) = parts.next() else {
                continue;
            };
            let word = word.trim();
            if word.is_empty() || !word.is_ascii() {
                continue;
            }
            let freq = parts
                .next()
                .and_then(|s| s.trim().parse::<f32>().ok())
                .unwrap_or(0.0);
            entries.push((word.to_ascii_lowercase(), freq));
        }

        entries.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.len().cmp(&b.0.len()))
                .then_with(|| a.0.cmp(&b.0))
        });
        entries.dedup_by(|a, b| a.0 == b.0);

        Self { entries }
    }

    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Prefix match; frequency-sorted (highest first), then shorter words.
    pub fn lookup_prefix(&self, prefix: &str) -> Vec<(&str, f32)> {
        let prefix_lower = prefix.to_ascii_lowercase();
        if prefix_lower.is_empty() || !prefix_lower.is_ascii() {
            return Vec::new();
        }
        self.entries
            .iter()
            .filter(|(w, _)| w.starts_with(&prefix_lower))
            .map(|(w, f)| (w.as_str(), *f))
            .collect()
    }

    /// Prefix hits capped for mixed-input use (`MINIMAL` / `MAXIMAL` constants).
    pub fn candidates_for_input(&self, input: &str) -> Vec<(&str, f32)> {
        if input.len() < MINIMAL_ENGLISH_CHARACTERS || !input.is_ascii() {
            return Vec::new();
        }
        let mut words = self.lookup_prefix(input);
        words.sort_by(|a, b| {
            a.0.len()
                .cmp(&b.0.len())
                .then_with(|| {
                    b.1.partial_cmp(&a.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.0.cmp(b.0))
        });
        words.truncate(MAXIMAL_ENGLISH_CANDIDATES);
        words
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_lookup_ranks_by_freq() {
        let dict = EnglishDict::from_str("hello\t2.0\nhelp\t3.0\nhelicopter\t1.0\n");
        let hits = dict.lookup_prefix("hel");
        assert_eq!(hits[0].0, "help");
        assert!(hits.iter().any(|(w, _)| *w == "hello"));
    }

    #[test]
    fn candidates_for_input_respects_caps() {
        let dict = EnglishDict::from_str(
            "hi\t9.0\nhelp\t3.0\nhello\t2.0\nhelicopter\t1.0\nhell\t4.0\n",
        );
        assert!(dict.candidates_for_input("hel").is_empty());
        let hits = dict.candidates_for_input("hell");
        assert!(hits.len() <= MAXIMAL_ENGLISH_CANDIDATES);
        assert_eq!(hits[0].0, "hell");
    }
}

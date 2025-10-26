// core/src/word_bigram.rs
//
// Word-level bigram model for phrase-to-phrase transitions.
// Stores P(word2 | word1) to score word sequences in candidate generation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

/// Entry in a word's bigram distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BigramEntry {
    pub word: String,
    pub count: u32,
}

/// Word-level bigram model
/// Maps word1 -> list of (word2, count) pairs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordBigram {
    /// Bigram data: word1 -> [(word2, count), ...]
    data: HashMap<String, Vec<BigramEntry>>,
    /// Total frequency for each word1 (for normalization)
    totals: HashMap<String, u32>,
    /// Unigram counts from interpolation2.text \1-gram section
    /// Used for P(w2) in interpolation formula
    unigram_counts: HashMap<String, u32>,
    /// Total of all unigram counts (for normalization)
    total_unigram_count: u64,
}

impl WordBigram {
    /// Create an empty word bigram model
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            totals: HashMap::new(),
            unigram_counts: HashMap::new(),
            total_unigram_count: 0,
        }
    }

    /// Get the probability P(word2 | word1)
    /// Returns 0.0 if the bigram doesn't exist
    pub fn get_probability(&self, word1: &str, word2: &str) -> f32 {
        if let Some(entries) = self.data.get(word1) {
            if let Some(entry) = entries.iter().find(|e| e.word == word2) {
                if let Some(&total) = self.totals.get(word1) {
                    if total > 0 {
                        return entry.count as f32 / total as f32;
                    }
                }
            }
        }
        0.0
    }

    /// Get log probability (natural log)
    /// Returns a large negative number if bigram doesn't exist
    pub fn get_log_probability(&self, word1: &str, word2: &str) -> f32 {
        let prob = self.get_probability(word1, word2);
        if prob > 0.0 {
            prob.ln()
        } else {
            -20.0 // Default for missing bigrams (matches character n-gram behavior)
        }
    }

    /// Add a bigram observation
    pub fn add_bigram(&mut self, word1: String, word2: String, count: u32) {
        let entry = BigramEntry { word: word2, count };

        self.data
            .entry(word1.clone())
            .or_default()
            .push(entry);

        *self.totals.entry(word1).or_insert(0) += count;
    }

    /// Add a unigram observation from interpolation2.text
    pub fn add_unigram(&mut self, word: String, count: u32) {
        *self.unigram_counts.entry(word).or_insert(0) += count;
        self.total_unigram_count += count as u64;
    }

    /// Get unigram probability P(word) from interpolation2.text data
    /// Returns 0.0 if word not found
    pub fn get_unigram_probability(&self, word: &str) -> f32 {
        if let Some(&count) = self.unigram_counts.get(word) {
            if self.total_unigram_count > 0 {
                return (count as f64 / self.total_unigram_count as f64) as f32;
            }
        }
        0.0
    }

    /// Get log unigram probability
    pub fn get_log_unigram_probability(&self, word: &str) -> f32 {
        let prob = self.get_unigram_probability(word);
        if prob > 0.0 {
            prob.ln()
        } else {
            -20.0 // Default for missing unigrams
        }
    }

    /// Load from bincode file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let model = bincode::deserialize_from(reader)?;
        Ok(model)
    }

    /// Save to bincode file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        bincode::serialize_into(writer, self)?;
        Ok(())
    }

    /// Get number of unique word1 entries
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get total number of bigram pairs
    pub fn total_bigrams(&self) -> usize {
        self.data.values().map(|v| v.len()).sum()
    }
}

impl Default for WordBigram {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_bigram_probability() {
        let mut wb = WordBigram::new();
        wb.add_bigram("今天".to_string(), "上海".to_string(), 10);
        wb.add_bigram("今天".to_string(), "很好".to_string(), 5);

        // P("上海" | "今天") = 10 / 15 = 0.666...
        let prob = wb.get_probability("今天", "上海");
        assert!((prob - 0.666).abs() < 0.01);

        // P("很好" | "今天") = 5 / 15 = 0.333...
        let prob = wb.get_probability("今天", "很好");
        assert!((prob - 0.333).abs() < 0.01);

        // Missing bigram
        let prob = wb.get_probability("今天", "不存在");
        assert_eq!(prob, 0.0);
    }

    #[test]
    fn test_word_bigram_log_probability() {
        let mut wb = WordBigram::new();
        wb.add_bigram("你好".to_string(), "世界".to_string(), 100);

        let log_prob = wb.get_log_probability("你好", "世界");
        assert!(log_prob == 0.0); // ln(1.0) = 0 since 100/100 = 1.0

        let log_prob = wb.get_log_probability("不存在", "也不存在");
        assert_eq!(log_prob, -20.0);
    }
}

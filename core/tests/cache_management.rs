// core/tests/cache_management.rs
//
// Integration tests for Engine cache management functionality.
//
// Tests cover:
// - LRU eviction behavior with real Engine usage
// - Cache size limits respect Config.max_cache_size
// - Hit/miss tracking statistics
// - Cache invalidation on commit()
// - Cache statistics API

use libchinese_core::{Config, Engine, Model, Lexicon, NGramModel, UserDict, Interpolator};
use std::path::PathBuf;

// Mock parser for testing
#[derive(Debug)]
struct MockParser;

#[derive(Debug, Clone)]
struct MockSyllable {
    text: String,
    fuzzy: bool,
}

impl libchinese_core::SyllableType for MockSyllable {
    fn text(&self) -> &str {
        &self.text
    }
    
    fn is_fuzzy(&self) -> bool {
        self.fuzzy
    }
}

impl libchinese_core::SyllableParser for MockParser {
    type Syllable = MockSyllable;
    
    fn segment_top_k(&self, input: &str, _k: usize, _allow_fuzzy: bool) -> Vec<Vec<Self::Syllable>> {
        // Simple mock: treat entire input as single syllable
        vec![vec![MockSyllable {
            text: input.to_string(),
            fuzzy: false,
        }]]
    }
}

fn setup_test_engine(cache_size: usize) -> Option<Engine<MockParser>> {
    let data_dir = PathBuf::from("data");
    
    // Check if required data files exist
    if !data_dir.join("pinyin.fst").exists() || 
       !data_dir.join("pinyin.redb").exists() ||
       !data_dir.join("ngram.bincode").exists() {
        return None;
    }
    
    // Create minimal model for testing
    let mut cfg = Config::default();
    cfg.max_cache_size = cache_size;
    
    let lex = Lexicon::load_from_fst_bincode(
        &data_dir.join("pinyin.fst"),
        &data_dir.join("pinyin.redb")
    ).ok()?;
    let ngram = NGramModel::load_bincode(&data_dir.join("ngram.bincode")).ok()?;
    let userdict = UserDict::new(&data_dir.join("test_userdict.redb")).ok()?;
    let interpolator = Interpolator::load(
        &data_dir.join("pinyin.lambdas.fst"),
        &data_dir.join("pinyin.lambdas.redb")
    ).ok()?;
    
    let model = Model::new(lex, ngram, userdict, cfg, interpolator);
    let parser = MockParser;
    
    Some(Engine::new(model, parser))
}

#[test]
fn test_cache_hit_miss_tracking() {
    let Some(engine) = setup_test_engine(3) else {
        eprintln!("Data files not present; skipping cache test");
        return;
    };
    
    // Initial state: no hits or misses
    let (hits, misses) = engine.cache_stats();
    assert_eq!(hits, 0);
    assert_eq!(misses, 0);
    assert_eq!(engine.cache_hit_rate(), None);  // No accesses yet
    
    // First access: cache miss
    let _ = engine.input("nihao");
    let (hits, misses) = engine.cache_stats();
    assert_eq!(hits, 0);
    assert_eq!(misses, 1);
    assert!(engine.cache_hit_rate().is_some());
    let hit_rate = engine.cache_hit_rate().unwrap();
    assert!((hit_rate - 0.0).abs() < 0.01);  // 0% hit rate
    
    // Second access to same input: cache hit
    let _ = engine.input("nihao");
    let (hits, misses) = engine.cache_stats();
    assert_eq!(hits, 1);
    assert_eq!(misses, 1);
    let hit_rate = engine.cache_hit_rate().unwrap();
    assert!((hit_rate - 50.0).abs() < 0.01);  // 50% hit rate
    
    // Third access: another hit
    let _ = engine.input("nihao");
    let (hits, misses) = engine.cache_stats();
    assert_eq!(hits, 2);
    assert_eq!(misses, 1);
    let hit_rate = engine.cache_hit_rate().unwrap();
    assert!((hit_rate - 66.67).abs() < 0.1);  // ~66.67% hit rate
}

#[test]
fn test_cache_size_tracking() {
    let Some(engine) = setup_test_engine(3) else {
        eprintln!("Data files not present; skipping cache test");
        return;
    };
    
    // Initial: empty cache
    assert_eq!(engine.cache_size(), 0);
    assert_eq!(engine.cache_capacity(), 3);  // As configured
    
    // Add first entry
    let _ = engine.input("a");
    assert_eq!(engine.cache_size(), 1);
    
    // Add second entry
    let _ = engine.input("b");
    assert_eq!(engine.cache_size(), 2);
    
    // Add third entry (at capacity)
    let _ = engine.input("c");
    assert_eq!(engine.cache_size(), 3);
    
    // Add fourth entry (should evict oldest, size stays at 3)
    let _ = engine.input("d");
    assert_eq!(engine.cache_size(), 3);
}

#[test]
fn test_lru_eviction_behavior() {
    let Some(engine) = setup_test_engine(3) else {
        eprintln!("Data files not present; skipping cache test");
        return;
    };
    
    // Fill cache to capacity (3 entries)
    let _ = engine.input("a");
    let _ = engine.input("b");
    let _ = engine.input("c");
    assert_eq!(engine.cache_size(), 3);
    
    // Access "a" to make it recently used
    let _ = engine.input("a");
    let (hits, _) = engine.cache_stats();
    assert_eq!(hits, 1);  // Should be cache hit
    
    // Add new entry - should evict "b" (oldest), not "a"
    let _ = engine.input("d");
    assert_eq!(engine.cache_size(), 3);
    
    // Verify "a" is still cached (hit)
    let hits_before = engine.cache_stats().0;
    let _ = engine.input("a");
    let hits_after = engine.cache_stats().0;
    assert_eq!(hits_after, hits_before + 1);  // Cache hit
    
    // Verify "b" was evicted (miss)
    let misses_before = engine.cache_stats().1;
    let _ = engine.input("b");
    let misses_after = engine.cache_stats().1;
    assert_eq!(misses_after, misses_before + 1);  // Cache miss
}

#[test]
fn test_cache_clear() {
    let Some(engine) = setup_test_engine(3) else {
        eprintln!("Data files not present; skipping cache test");
        return;
    };
    
    // Add some entries
    let _ = engine.input("a");
    let _ = engine.input("b");
    let _ = engine.input("a");  // Hit
    
    let (hits, misses) = engine.cache_stats();
    assert_eq!(hits, 1);
    assert_eq!(misses, 2);
    assert_eq!(engine.cache_size(), 2);
    
    // Clear cache
    engine.clear_cache();
    
    // Verify everything reset
    assert_eq!(engine.cache_size(), 0);
    let (hits, misses) = engine.cache_stats();
    assert_eq!(hits, 0);
    assert_eq!(misses, 0);
    assert_eq!(engine.cache_hit_rate(), None);
}

#[test]
fn test_cache_with_different_inputs() {
    let Some(engine) = setup_test_engine(3) else {
        eprintln!("Data files not present; skipping cache test");
        return;
    };
    
    // Different inputs should create different cache entries
    let r1 = engine.input("ni");
    let r2 = engine.input("hao");
    let _r3 = engine.input("nihao");
    
    assert_eq!(engine.cache_size(), 3);
    let (hits, misses) = engine.cache_stats();
    assert_eq!(hits, 0);
    assert_eq!(misses, 3);
    
    // Same inputs should hit cache
    let r1_cached = engine.input("ni");
    let r2_cached = engine.input("hao");
    
    assert_eq!(r1, r1_cached);
    assert_eq!(r2, r2_cached);
    
    let (hits, misses) = engine.cache_stats();
    assert_eq!(hits, 2);
    assert_eq!(misses, 3);
}

#[test]
fn test_commit_clears_cache() {
    let Some(engine) = setup_test_engine(3) else {
        eprintln!("Data files not present; skipping cache test");
        return;
    };
    
    // Fill cache
    let _ = engine.input("a");
    let _ = engine.input("b");
    assert_eq!(engine.cache_size(), 2);
    
    // Commit should clear cache to reflect updated user dict
    engine.commit("某");
    
    // Cache should be cleared
    assert_eq!(engine.cache_size(), 0);
    
    // Stats should be reset too
    let (hits, misses) = engine.cache_stats();
    assert_eq!(hits, 0);
    assert_eq!(misses, 0);
}

#[test]
fn test_large_cache_size() {
    let Some(engine) = setup_test_engine(100) else {
        eprintln!("Data files not present; skipping cache test");
        return;
    };  // Larger cache
    
    assert_eq!(engine.cache_capacity(), 100);
    
    // Add many entries
    for i in 0..50 {
        let _ = engine.input(&format!("input{}", i));
    }
    
    assert_eq!(engine.cache_size(), 50);
    let (hits, misses) = engine.cache_stats();
    assert_eq!(hits, 0);
    assert_eq!(misses, 50);
    
    // Verify some hits
    for i in 0..10 {
        let _ = engine.input(&format!("input{}", i));
    }
    
    let (hits, misses) = engine.cache_stats();
    assert_eq!(hits, 10);
    assert_eq!(misses, 50);
}

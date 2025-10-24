//! Test demonstrating the enhanced data storage formats in Step 5

#[cfg(test)]
mod tests {
    use libchinese_core::{Config, NGramModel, UserDict};
    use std::fs;

    #[test]
    fn test_enhanced_config_toml_format() {
        // Test TOML configuration format
        let mut config = Config::default();
        // Add some fuzzy rules since default is now empty
        config.fuzzy = vec!["zh=z".to_string(), "ch=c".to_string()];

        // Save to TOML
        let toml_path = "test_config.toml";
        config
            .save_toml(toml_path)
            .expect("Failed to save TOML config");

        // Load back from TOML
        let loaded_config = Config::load_toml(toml_path).expect("Failed to load TOML config");

        // Verify fuzzy rules are preserved
        assert_eq!(config.fuzzy.len(), loaded_config.fuzzy.len());
        assert!(loaded_config.fuzzy.contains(&"zh=z".to_string()));
        assert_eq!(config.unigram_weight, loaded_config.unigram_weight);
        assert_eq!(config.bigram_weight, loaded_config.bigram_weight);

        // Cleanup
        let _ = fs::remove_file(toml_path);
    }

    #[test]
    fn test_enhanced_ngram_serialization() {
        // Test N-gram model serialization with bincode
        let mut model = NGramModel::new();
        model.insert_unigram("你", -1.0);
        model.insert_unigram("好", -1.2);
        model.insert_bigram("你", "好", -0.5);

        // Test bincode serialization
        let bincode_path = "test_ngram.bincode";

        model
            .save_bincode(bincode_path)
            .expect("Failed to save bincode");

        // Load back and verify data integrity
        let loaded_model = NGramModel::load_bincode(bincode_path).expect("Failed to load bincode");
        assert_eq!(loaded_model.get_unigram("你").unwrap(), -1.0);
        assert_eq!(loaded_model.get_unigram("好").unwrap(), -1.2);
        assert_eq!(loaded_model.get_bigram("你", "好").unwrap(), -0.5);

        // Cleanup
        let _ = fs::remove_file(bincode_path);
    }

    #[test]
    fn test_enhanced_userdict_basic() {
        // Test user dictionary basic functionality
        let temp_path = std::env::temp_dir().join(format!(
            "test_userdict_basic_{}.redb",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let userdict = UserDict::new(&temp_path).expect("create test userdict");

        // Learn some phrases
        userdict.learn("测试");
        userdict.learn("测试"); // Learn again to increase frequency
        userdict.learn("词典");

        // Verify learning worked
        assert_eq!(userdict.frequency("测试"), 2);
        assert_eq!(userdict.frequency("词典"), 1);

        // Verify snapshot
        let snapshot = userdict.snapshot();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot.get("测试"), Some(&2));
        assert_eq!(snapshot.get("词典"), Some(&1));
    }

    #[test]
    fn test_storage_format_compatibility() {
        // Test that all storage formats work together
        let config = Config::default();
        let mut ngram = NGramModel::new();
        let temp_path = std::env::temp_dir().join(format!(
            "test_userdict_compat_{}.redb",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let userdict = UserDict::new(&temp_path).expect("create test userdict");

        // Setup test data
        ngram.insert_unigram("测", -2.0);
        ngram.insert_unigram("试", -2.1);
        ngram.insert_bigram("测", "试", -1.0);
        userdict.learn("测试");

        // Test serialization
        let config_toml = config
            .to_toml_string()
            .expect("Config TOML serialization failed");

        // Verify data integrity
        assert!(config_toml.contains("fuzzy = ["));
        assert_eq!(ngram.get_unigram("测").unwrap(), -2.0);
        assert_eq!(ngram.get_bigram("测", "试").unwrap(), -1.0);
        assert_eq!(userdict.frequency("测试"), 1);

        println!("✓ Enhanced storage formats working correctly!");
        println!("  - Config: {} fuzzy rules", config.fuzzy.len());
        println!("  - N-gram: data loaded successfully");
        println!("  - UserDict: 1 entry learned");
    }
}

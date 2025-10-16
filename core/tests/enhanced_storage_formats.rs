//! Test demonstrating the enhanced data storage formats in Step 5

#[cfg(test)]
mod tests {
    use libchinese_core::{Config, NGramModel, UserDict};
    use std::fs;

    #[test]
    fn test_enhanced_config_toml_format() {
        // Test TOML configuration format
        let config = Config::default();
        
        // Save to TOML
        let toml_path = "test_config.toml";
        config.save_toml(toml_path).expect("Failed to save TOML config");
        
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
    fn test_enhanced_ngram_metadata() {
        // Test N-gram model with metadata
        let mut model = NGramModel::new();
        model.insert_unigram("你", -1.0);
        model.insert_unigram("好", -1.2);
        model.insert_bigram("你", "好", -0.5);
        
        // Get metadata
        let metadata = model.get_metadata();
        assert_eq!(metadata.version, "1.0");
        assert_eq!(metadata.unigram_count, 2);
        assert_eq!(metadata.bigram_count, 1);
        assert_eq!(metadata.trigram_count, 0);
        
        // Test enhanced serialization
        let bincode_path = "test_ngram.bincode";
        let metadata_path = "test_ngram.metadata.json";
        
        model.save_bincode(bincode_path).expect("Failed to save bincode");
        model.save_metadata_json(metadata_path).expect("Failed to save metadata");
        
        // Load back and verify
        let loaded_model = NGramModel::load_bincode(bincode_path).expect("Failed to load bincode");
        assert_eq!(loaded_model.get_unigram("你").unwrap(), -1.0);
        
        // Verify metadata JSON exists and is readable
        let json_content = fs::read_to_string(metadata_path).expect("Failed to read metadata JSON");
        assert!(json_content.contains("\"version\": \"1.0\""));
        assert!(json_content.contains("\"unigram_count\": 2"));
        
        // Cleanup
        let _ = fs::remove_file(bincode_path);
        let _ = fs::remove_file(metadata_path);
    }

    #[test]
    fn test_enhanced_userdict_metadata() {
        // Test user dictionary with metadata
        let userdict = UserDict::new();
        
        // Learn some phrases
        userdict.learn("测试");
        userdict.learn("测试"); // Learn again to increase frequency
        userdict.learn("词典");
        
        // Get metadata
        let metadata = userdict.get_metadata();
        assert_eq!(metadata.version, "1.0");
        assert_eq!(metadata.entry_count, 2);
        assert_eq!(metadata.total_frequency, 3); // "测试" = 2, "词典" = 1
        
        // Test metadata export
        let metadata_path = "test_userdict.metadata.json";
        userdict.export_metadata_json(metadata_path).expect("Failed to export metadata");
        
        // Verify JSON content
        let json_content = fs::read_to_string(metadata_path).expect("Failed to read metadata JSON");
        assert!(json_content.contains("\"entry_count\": 2"));
        assert!(json_content.contains("\"total_frequency\": 3"));
        
        // Cleanup
        let _ = fs::remove_file(metadata_path);
    }

    #[test]
    fn test_storage_format_compatibility() {
        // Test that all enhanced formats work together
        let config = Config::default();
        let mut ngram = NGramModel::new();
        let userdict = UserDict::new();
        
        // Setup test data
        ngram.insert_unigram("测", -2.0);
        ngram.insert_unigram("试", -2.1);
        ngram.insert_bigram("测", "试", -1.0);
        userdict.learn("测试");
        
        // Test all metadata functions
        let config_toml = config.to_toml_string().expect("Config TOML serialization failed");
        let ngram_metadata = ngram.get_metadata();
        let userdict_metadata = userdict.get_metadata();
        
        // Debug TOML content
        println!("TOML content: {}", config_toml);
        
        // Verify metadata consistency - check for fuzzy array instead of [fuzzy] section
        assert!(config_toml.contains("fuzzy = ["));
        assert_eq!(ngram_metadata.unigram_count, 2);
        assert_eq!(ngram_metadata.bigram_count, 1);
        assert_eq!(userdict_metadata.entry_count, 1);
        assert_eq!(userdict_metadata.total_frequency, 1);
        
        println!("✓ Enhanced storage formats working correctly!");
        println!("  - Config: {} fuzzy rules", config.fuzzy.len());
        println!("  - N-gram: {} unigrams, {} bigrams", ngram_metadata.unigram_count, ngram_metadata.bigram_count);
        println!("  - UserDict: {} entries, {} total frequency", userdict_metadata.entry_count, userdict_metadata.total_frequency);
    }
}
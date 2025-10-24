//! Cloud input support for rare phrases and predictions.
//!
//! **STATUS (2025-10-24)**: 
//! - Google Input Tools API: REMOVED (was shut down in 2011)
//! - Baidu Input API: NOT WORKING (network failure, may need auth)
//! - Custom endpoint: WORKING (requires user-deployed server)
//!
//! This module provides online prediction from cloud services.
//! **NOTE**: Cloud input is disabled by default and is not required
//! for normal operation. Local prediction provides excellent results
//! without external dependencies.
//!
//! Uses `reqwest` blocking client for simplicity - no async runtime needed!

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Cloud input provider options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudProvider {
    /// Baidu Input API (most popular in China)
    /// 
    /// **STATUS**: Currently not working (network failure as of 2025-01-16).
    /// May require authentication or different endpoint.
    Baidu,
    /// Custom endpoint URL for user-deployed prediction server
    Custom(String),
}

impl Default for CloudProvider {
    fn default() -> Self {
        Self::Baidu
    }
}

/// A cloud candidate result with confidence score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CloudCandidate {
    /// The candidate text (Chinese characters)
    pub text: String,
    /// Confidence score (0.0-1.0, where 1.0 is highest confidence)
    pub confidence: f32,
}

/// Cloud input client for querying online prediction services.
pub struct CloudInput {
    provider: CloudProvider,
    enabled: bool,
    timeout_ms: u64,
}

impl CloudInput {
    /// Create a new cloud input client with the specified provider.
    pub fn new(provider: CloudProvider) -> Self {
        Self {
            provider,
            enabled: false,
            timeout_ms: 500, // Default 500ms timeout
        }
    }

    /// Enable or disable cloud input.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if cloud input is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set the request timeout in milliseconds.
    pub fn set_timeout(&mut self, timeout_ms: u64) {
        self.timeout_ms = timeout_ms;
    }

    /// Query cloud service for candidates (blocking call with timeout).
    ///
    /// This is a synchronous function that blocks until the network request
    /// completes or times out. Safe to call from any context.
    ///
    /// Returns an empty vector if:
    /// - Cloud input is disabled
    /// - Network request fails
    /// - Timeout occurs
    /// - No results from provider
    pub fn query(&self, pinyin: &str) -> Vec<CloudCandidate> {
        if !self.enabled || pinyin.is_empty() {
            return vec![];
        }

        // Direct blocking call - no async needed!
        match self.query_blocking(pinyin) {
            Ok(candidates) => candidates,
            Err(_) => {
                // Silent failure - cloud input is optional
                vec![]
            }
        }
    }

    /// Blocking query implementation.
    fn query_blocking(&self, pinyin: &str) -> Result<Vec<CloudCandidate>, Box<dyn std::error::Error>> {
        match &self.provider {
            CloudProvider::Baidu => self.query_baidu(pinyin),
            CloudProvider::Custom(url) => self.query_custom(url, pinyin),
        }
    }

    /// Query Baidu Input API.
    ///
    /// **STATUS**: Currently not working (2025-01-16).
    /// Network requests time out or fail. May require:
    /// - Authentication or API key
    /// - Different endpoint URL
    /// - Specific headers (User-Agent, Referer)
    ///
    /// API endpoint: https://olime.baidu.com/py
    /// Parameters:
    /// - input: pinyin string (e.g., "nihao")
    /// - inputtype: "py" for pinyin
    /// - bg: start index (0)
    /// - ed: end index (20 for 20 results)
    /// - result: "hanzi" for Chinese characters
    /// - resultcoding: "utf-8"
    fn query_baidu(&self, pinyin: &str) -> Result<Vec<CloudCandidate>, Box<dyn std::error::Error>> {
        let url = format!(
            "https://olime.baidu.com/py?input={}&inputtype=py&bg=0&ed=20&result=hanzi&resultcoding=utf-8&ch_en=0&clientinfo=web&version=1",
            urlencoding::encode(pinyin)
        );

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(self.timeout_ms))
            .build()?;

        let response = client.get(&url).send()?;
        let text = response.text()?;

        // Baidu returns JSON array of arrays: [["你好","ni'hao"],["拟好","ni'hao"]]
        // We want the first element of each sub-array
        let candidates: Vec<Vec<String>> = serde_json::from_str(&text)?;

        let results = candidates
            .into_iter()
            .filter_map(|arr| arr.into_iter().next())
            .map(|text| CloudCandidate {
                text,
                confidence: 0.8, // Baidu doesn't provide confidence scores
            })
            .collect();

        Ok(results)
    }

    /// Query custom endpoint.
    ///
    /// Expected request format:
    /// POST to custom URL with JSON body: {"query": "pinyin"}
    ///
    /// Expected response format:
    /// JSON array: [{"text": "你好", "confidence": 0.95}, ...]
    fn query_custom(&self, url: &str, pinyin: &str) -> Result<Vec<CloudCandidate>, Box<dyn std::error::Error>> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(self.timeout_ms))
            .build()?;

        let body = serde_json::json!({
            "query": pinyin,
        });

        let response = client
            .post(url)
            .json(&body)
            .send()?;

        let candidates: Vec<CloudCandidate> = response.json()?;
        Ok(candidates)
    }
}

impl Default for CloudInput {
    fn default() -> Self {
        Self::new(CloudProvider::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_cloud_input() {
        let cloud = CloudInput::new(CloudProvider::Baidu);
        assert!(!cloud.is_enabled());
        assert_eq!(cloud.timeout_ms, 500);
    }

    #[test]
    fn test_enable_disable() {
        let mut cloud = CloudInput::new(CloudProvider::Baidu);
        assert!(!cloud.is_enabled());
        
        cloud.set_enabled(true);
        assert!(cloud.is_enabled());
        
        cloud.set_enabled(false);
        assert!(!cloud.is_enabled());
    }

    #[test]
    fn test_set_timeout() {
        let mut cloud = CloudInput::new(CloudProvider::Baidu);
        assert_eq!(cloud.timeout_ms, 500);
        
        cloud.set_timeout(1000);
        assert_eq!(cloud.timeout_ms, 1000);
    }

    #[test]
    fn test_query_when_disabled() {
        let cloud = CloudInput::new(CloudProvider::Baidu);
        assert!(!cloud.is_enabled());
        
        let results = cloud.query("nihao");
        assert!(results.is_empty());
    }

    #[test]
    fn test_query_empty_input() {
        let mut cloud = CloudInput::new(CloudProvider::Baidu);
        cloud.set_enabled(true);
        
        let results = cloud.query("");
        assert!(results.is_empty());
    }

    #[test]
    fn test_cloud_candidate_creation() {
        let candidate = CloudCandidate {
            text: "你好".to_string(),
            confidence: 0.95,
        };
        
        assert_eq!(candidate.text, "你好");
        assert_eq!(candidate.confidence, 0.95);
    }

    #[test]
    fn test_cloud_provider_variants() {
        let baidu = CloudProvider::Baidu;
        let custom = CloudProvider::Custom("https://example.com/api".to_string());
        
        assert_eq!(baidu, CloudProvider::Baidu);
        assert!(matches!(custom, CloudProvider::Custom(_)));
    }

    // Note: Real network tests would require network access
    // These are skipped in normal test runs
    #[test]
    #[ignore]
    fn test_query_baidu_real_network() {
        let mut cloud = CloudInput::new(CloudProvider::Baidu);
        cloud.set_enabled(true);
        
        let results = cloud.query("nihao");
        
        // If network is available, we should get results
        if !results.is_empty() {
            println!("Baidu results for 'nihao': {:?}", results);
            assert!(results[0].text.contains("你好") || results.iter().any(|c| c.text.contains("你好")));
        }
    }
}

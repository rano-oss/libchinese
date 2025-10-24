/// Zhuyin/Bopomofo-specific configuration that extends the base `Config` from core.
///
/// This configuration includes:
/// - All generic options from `libchinese_core::Config` (flattened via serde)
/// - Zhuyin-specific incomplete syllable matching
/// - Zhuyin correction options for keyboard layouts (HSU, ETEN26, shuffle)
/// - Zhuyin-specific fuzzy matching rules
///
/// # Example
///
/// ```rust
/// use libzhuyin::ZhuyinConfig;
///
/// let config = ZhuyinConfig::default();
/// let base_config = config.into_base();
/// // Use base_config with Model::new()
/// ```
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ZhuyinConfig {
    /// Base configuration fields (fuzzy, weights, sorting, etc.)
    #[serde(flatten)]
    pub base: libchinese_core::Config,

    /// Allow incomplete Zhuyin syllables (e.g., "ㄓ", "ㄕ", "ㄈ" without finals)
    pub zhuyin_incomplete: bool,

    // Zhuyin keyboard layout correction options
    /// Handle shuffle errors (e.g., tone mark position in some layouts)
    pub zhuyin_correct_shuffle: bool,

    /// HSU keyboard layout corrections
    pub zhuyin_correct_hsu: bool,

    /// ETEN26 keyboard layout corrections
    pub zhuyin_correct_eten26: bool,
}

impl Default for ZhuyinConfig {
    fn default() -> Self {
        let mut base = libchinese_core::Config::default();
        base.fuzzy = zhuyin_default_fuzzy_rules();

        Self {
            base,
            zhuyin_incomplete: true,
            zhuyin_correct_shuffle: true,
            zhuyin_correct_hsu: true,
            zhuyin_correct_eten26: true,
        }
    }
}

impl ZhuyinConfig {
    /// Convert this zhuyin config into the base config for use with `Model::new()`
    pub fn into_base(self) -> libchinese_core::Config {
        self.base
    }

    /// Get a reference to the base config
    pub fn base(&self) -> &libchinese_core::Config {
        &self.base
    }

    /// Get a mutable reference to the base config
    pub fn base_mut(&mut self) -> &mut libchinese_core::Config {
        &mut self.base
    }
}

/// Returns the default fuzzy matching rules for Zhuyin/Bopomofo input.
///
/// Currently empty, as Zhuyin fuzzy rules are typically handled via keyboard
/// layout corrections (HSU, ETEN26, shuffle) rather than phonetic confusion.
/// Language crates can extend this with custom rules if needed.
fn zhuyin_default_fuzzy_rules() -> Vec<String> {
    vec![
        // Zhuyin typically uses keyboard layout corrections instead of fuzzy phonetic rules
        // Add custom rules here if needed for specific use cases
    ]
}

/// Pinyin-specific configuration that extends the base `Config` from core.
///
/// This configuration includes:
/// - All generic options from `libchinese_core::Config` (flattened via serde)
/// - Pinyin-specific correction options (ue/ve, v/u, uen/un, etc.)
/// - Double pinyin scheme support
/// - Pinyin-specific fuzzy matching rules
///
/// # Example
///
/// ```rust
/// use libpinyin::PinyinConfig;
/// 
/// let config = PinyinConfig::default();
/// let base_config = config.into_base();
/// // Use base_config with Model::new()
/// ```

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PinyinConfig {
    /// Base configuration fields (fuzzy, weights, sorting, etc.)
    #[serde(flatten)]
    pub base: libchinese_core::Config,
    
    // Pinyin incomplete syllable matching (e.g., "zh", "ch", "sh" without finals)
    pub pinyin_incomplete: bool,
    
    // Pinyin correction options for common misspellings
    pub correct_ue_ve: bool,      // nue ↔ nve
    pub correct_v_u: bool,         // nv ↔ nu  
    pub correct_uen_un: bool,      // juen ↔ jun
    pub correct_gn_ng: bool,       // bagn ↔ bang
    pub correct_mg_ng: bool,       // bamg ↔ bang
    pub correct_iou_iu: bool,      // liou ↔ liu
    
    /// Double pinyin scheme (e.g., "Microsoft", "ZiRanMa", "XiaoHe")
    pub double_pinyin_scheme: Option<String>,
    
    /// Sort candidates by pinyin length (prefer shorter pinyin sequences)
    pub sort_by_pinyin_length: bool,
}

impl Default for PinyinConfig {
    fn default() -> Self {
        let mut base = libchinese_core::Config::default();
        base.fuzzy = pinyin_default_fuzzy_rules();
        
        Self {
            base,
            pinyin_incomplete: true,
            correct_ue_ve: true,
            correct_v_u: true,
            correct_uen_un: true,
            correct_gn_ng: true,
            correct_mg_ng: true,
            correct_iou_iu: true,
            double_pinyin_scheme: None,
            sort_by_pinyin_length: false,
        }
    }
}

impl PinyinConfig {
    /// Convert this pinyin config into the base config for use with `Model::new()`
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

/// Returns the default fuzzy matching rules for Pinyin input.
///
/// These rules handle common confusion patterns in Mandarin pronunciation:
/// - Retroflex vs non-retroflex initials: zh=z, ch=c, sh=s
/// - Nasal finals: an=ang, en=eng, in=ing
/// - Front/back nasal confusion: ian=iang, uan=uang
/// - Common consonant confusions: l=n, f=h, k=g
pub fn pinyin_default_fuzzy_rules() -> Vec<String> {
    vec![
        // Retroflex vs non-retroflex
        "zh=z".into(), "z=zh".into(),
        "ch=c".into(), "c=ch".into(),
        "sh=s".into(), "s=sh".into(),
        
        // Nasal finals (n vs ng)
        "an=ang".into(), "ang=an".into(),
        "en=eng".into(), "eng=en".into(),
        "in=ing".into(), "ing=in".into(),
        
        // Front vs back nasal with medials
        "ian=iang".into(), "iang=ian".into(),
        "uan=uang".into(), "uang=uan".into(),
        
        // Common consonant confusions
        "l=n".into(), "n=l".into(),
        "f=h".into(), "h=f".into(),
        "k=g".into(), "g=k".into(),
    ]
}

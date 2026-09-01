//! Chinese / full-width punctuation helpers (Mac/Windows-class defaults).

/// Preferred Chinese punctuation for an ASCII key (Mac/Windows 中-mode default).
///
/// Returns the primary form that should be committed immediately without a picker.
pub fn preferred_chinese_punct(ch: char) -> Option<&'static str> {
    match ch {
        ',' => Some("，"),
        '.' => Some("。"),
        ';' => Some("；"),
        ':' => Some("："),
        '?' => Some("？"),
        '!' => Some("！"),
        '"' => Some("\u{201C}"),
        '\'' => Some("\u{2018}"),
        '(' => Some("（"),
        ')' => Some("）"),
        '[' => Some("【"),
        ']' => Some("】"),
        '{' => Some("「"),
        '}' => Some("」"),
        '<' => Some("《"),
        '>' => Some("》"),
        '~' => Some("～"),
        '\\' => Some("、"),
        '^' => Some("……"),
        '$' => Some("￥"),
        _ => None,
    }
}

/// Whether `ch` is a punctuation key that has a Chinese preferred form.
pub fn is_chinese_punct_key(ch: char) -> bool {
    preferred_chinese_punct(ch).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferred_basics() {
        assert_eq!(preferred_chinese_punct('.'), Some("。"));
        assert_eq!(preferred_chinese_punct(','), Some("，"));
        assert_eq!(preferred_chinese_punct('?'), Some("？"));
        assert_eq!(preferred_chinese_punct('a'), None);
    }
}

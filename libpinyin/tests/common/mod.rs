//! Shared helpers for libpinyin integration tests.

#![allow(dead_code)]

pub mod c_libpinyin;

pub use c_libpinyin::CLibPinyin;

use std::path::PathBuf;

/// Simplified lexicon data directory (repo checkout or system install).
pub fn data_dir() -> PathBuf {
    for p in [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/converted/simplified"),
        PathBuf::from("/usr/share/libpinyin/data/simplified"),
    ] {
        if p.join("lexicon.fst").exists() {
            return p;
        }
    }
    panic!("no rust pinyin data dir");
}

/// Point user dictionary at an isolated temp file (avoids cross-test pollution).
pub fn isolate_userdict(prefix: &str) {
    let ud = std::env::temp_dir().join(format!(
        "libchinese-{prefix}-userdict-{}-{}.redb",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::env::set_var("LIBCHINESE_USERDICT", &ud);
}

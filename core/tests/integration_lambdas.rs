use std::path::PathBuf;

#[test]
fn load_generated_pinyin_lambdas_if_present() {
    // If the pinyin lambdas fst+redb were generated in data/, ensure they can be loaded by Interpolator::load
    let fst = PathBuf::from("data/pinyin.lambdas.fst");
    let redb = PathBuf::from("data/pinyin.lambdas.redb");
    if fst.exists() && redb.exists() {
        let interp = libchinese_core::Interpolator::load(&fst, &redb).expect("load interp");
        // don't assert on content (may vary); just ensure the object constructed and can do a lookup safely (may return None)
        let _ = interp.lookup("#k1");
    } else {
        eprintln!("pinyin lambdas artifacts not present; skipping integration test");
    }
}

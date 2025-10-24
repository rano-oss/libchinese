use libchinese_core::Lambdas;
use redb::{ReadableTable, TableDefinition};

#[test]
fn check_pinyin_lambdas_content_if_present() {
    let fst = std::path::Path::new("data/pinyin.lambdas.fst");
    let redb = std::path::Path::new("data/pinyin.lambdas.redb");
    if !fst.exists() || !redb.exists() {
        eprintln!("pinyin lambdas artifacts not present; skipping content test");
        return;
    }

    // Open the redb and read first lambdas entry
    let db = match redb::Database::open(redb) {
        Ok(d) => d,
        Err(e) => {
            panic!("failed to open redb: {}", e);
        }
    };

    let rt = db.begin_read().expect("begin_read");
    let table: redb::ReadOnlyTable<u64, Vec<u8>> = rt
        .open_table(TableDefinition::new("lambdas"))
        .expect("open table");

    // find first entry and deserialize
    for item in table.iter().expect("table.iter") {
        let (_k, v) = item.expect("item");
        let bytes = v.value();
        let l = bincode::deserialize::<Lambdas>(&bytes).expect("deserialize lambdas");
        let arr = l.0;
        for w in arr.iter() {
            assert!(
                (*w >= 0.0) && (*w <= 1.0),
                "lambda weight out of range: {}",
                w
            );
        }
        let sum: f32 = arr.iter().copied().sum();
        assert!(
            (sum - 1.0).abs() < 1e-2,
            "lambdas do not sum to ~1: {}",
            sum
        );
        // success for first entry
        return;
    }

    panic!("no entries found in lambdas table");
}

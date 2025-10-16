convert_tables - simple table conversion tool

This small CLI converts a JSON key -> [ { text, freq } ] mapping into:
- an `fst` map file mapping keys -> numeric index
- a `redb` database storing serialized phrase lists keyed by index

Usage (from workspace root):

```pwsh
cargo run -p convert_tables -- --input tools/convert_tables/examples/sample.json --out-fst data/lexicon.fst --out-redb data/lexicon.redb
```

The produced files can be loaded by downstream crates:
- Use `fst::Map::from_bytes` to open the fst map and lookup keys to indices
- Use `redb::Database::open` and read the `phrases` table to retrieve the serialized phrase list for an index

cargo run -p convert_tables --bin estimate_interpolation --manifest-path .\tools\Cargo.toml -- data\interpolation2.text --out data\interpolation.estimated.txt
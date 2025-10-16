# Copilot Instructions for libchinese

## Project Overview
- **libchinese** is a Rust workspace for Chinese input method engines, containing three main crates:
  - `core`: Shared logic for models, dictionaries, and n-gram processing
  - `libpinyin`: Pinyin input engine, depends on `core`
  - `libzhuyin`: Zhuyin/Bopomofo input engine, depends on `core`

## Architecture & Data Flow
- `core` provides reusable components (model, dictionary, n-gram logic) for both input engines.
- `libpinyin` and `libzhuyin` are CLI binaries, each importing `core` for backend logic.
- All crates use Rust 2021 edition and MIT license.
- Data serialization uses `serde` and `bincode`.
- Database/storage uses `redb` and fast string matching via `fst`.
- Unicode normalization and tracing are enabled in `core`.

## Developer Workflows
- **Build all crates:**
  ```pwsh
  cargo build --workspace
  ```
- **Test core logic:**
  ```pwsh
  cargo test -p libchinese-core
  ```
- **Run input engines:**
  ```pwsh
  cargo run -p libpinyin
  cargo run -p libzhuyin
  ```
- **Add dependencies:**
  - Add to the relevant crate's `Cargo.toml`.
  - For shared logic, prefer adding to `core`.

## Conventions & Patterns
- Shared logic lives in `core/src/lib.rs`.
- Input engines have entry points in `libpinyin/src/main.rs` and `libzhuyin/src/main.rs`.
- Use `phf` for static maps and `regex` for pattern matching in input engines.
- Command-line parsing via `clap` in binaries.
- Tests are colocated in modules using Rust's `#[cfg(test)]` pattern.

## Integration Points
- No external services; all dependencies are Rust crates.
- Cross-crate communication is via Rust module imports and workspace paths.

## Example: Adding a Shared Utility
- Implement in `core/src/lib.rs`.
- Expose via `pub fn` or `pub mod`.
- Use from input engines by importing `libchinese_core`.

---

For questions about unclear workflows, missing conventions, or new patterns, ask for clarification or examples from maintainers.

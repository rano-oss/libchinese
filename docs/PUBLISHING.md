# Publishing to crates.io

The workspace is **prepared** for publishing; crates have **not** been uploaded yet.

## Intended order

1. **`libchinese-core`** — no internal path dependencies.
2. **`libpinyin`** and **`libzhuyin`** — depend on `libchinese-core` (publish core first, then bump/path-remove as needed).

Workspace `tools/*` crates set `publish = false`.

## Data

Published crates contain **source only**. Runtime lexicons and n-gram binaries live in the git repository under `data/` (often gitignored or built locally). Downstream apps must ship or download converted data separately; see each crate `README.md`.

## Before the first publish

- Resolve crate ownership on crates.io (`libchinese-core`, `libpinyin`, `libzhuyin`).
- Confirm crates.io names are free / owned (`libchinese-core`, `libpinyin`, `libzhuyin`).
- Choose release version(s) and tag; update `version` in all three publishable crates together.
- Replace `path = "../core"` with a crates.io version only when publishing from a standalone checkout (workspace `path` + `version` is the usual pattern).
- Run `cargo package -p libchinese-core --list` (and libpinyin/libzhuyin) and confirm no `target/`, coverage, or generated `.fst`/`.redb` artifacts.
- `cargo publish -p libchinese-core` then dependents (not automated here).

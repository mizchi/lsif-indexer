# Repository Guidelines

## Project Structure & Module Organization
- `src/`: Rust library and binaries
  - `src/core/`: graph, LSIF, incremental, query, call hierarchy
  - `src/cli/`: CLI, LSP adapters (`go_adapter.rs`, `python_adapter.rs`, `typescript_adapter.rs`, etc.)
  - `src/bin/lsif.rs`, `src/bin/main.rs`: CLI entry points
- `tests/`: integration/E2E tests (e.g., `graph_*_test.rs`, `typescript_*_test.rs`)
- `benches/`: Criterion benchmarks
- `scripts/`: utilities (`self-index.sh`, `clean.sh`)
- `tmp/`: local artifacts (indices, coverage, scratch)

## Build, Test, and Development Commands
- Build: `cargo build --release` or `make build`
- Test (unit+integration): `cargo test` or `make test`
- Specific suites: `make test-unit`, `make test-integration`, `make test-reference`
- Lints/format: `cargo clippy -- -D warnings`, `cargo fmt` or `make check`
- Benchmarks: `cargo bench` or `make bench`
- Self-index demo: `./scripts/self-index.sh` then `./target/release/lsif interactive --db tmp/self-index.lsif`

## Coding Style & Naming Conventions
- Rust 2021 edition; format with `rustfmt` (CI enforces `cargo fmt --check`).
- Keep Clippy clean (`-D warnings` in CI); prefer explicit error handling (`anyhow`, `thiserror`).
- Naming: modules/files `snake_case`, types/traits `CamelCase`, functions `snake_case`.
- Layout: keep CLI concerns in `src/cli`, graph/domain in `src/core`, binary glue in `src/bin`.

## Testing Guidelines
- Framework: `cargo test` (unit in `src`, integration in `tests/`).
- E2E/LSP tests may be `#[ignore]` locally; run explicitly with `cargo test -- --ignored`.
- Serial E2E (match CI): `cargo test --test '*' -- --test-threads=1`.
- Coverage (optional, recommended): `cargo install cargo-llvm-cov && cargo llvm-cov --lib`.
- Add tests alongside features; fixtures live under `tests/fixtures`.

## Commit & Pull Request Guidelines
- Commits follow Conventional Commits seen in history: `feat:`, `fix:`, `refactor:`, `docs:`, `test:` (e.g., `feat: TypeScript references via LSP`).
- PRs: use `.github/pull_request_template.md`; include description, motivation, and testing notes.
- Required before review: `cargo fmt`, `cargo clippy`, `cargo test` all pass; add/adjust tests; link issues; include screenshots/logs for CLI changes when useful.

## Tips & Config
- Common env: `RUST_LOG=debug`, `RUST_BACKTRACE=1`.
- Large/local artifacts go to `tmp/`; index DB defaults to `.lsif-index.db` but can be set via `--db`.

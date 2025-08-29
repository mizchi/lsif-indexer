# Repository Guidelines

## Project Structure & Module Organization
- `src/`: library and binaries
  - `src/core/`: graph, LSIF, incremental, queries, relations
  - `src/cli/`: CLI, adapters (Go/Python/TypeScript), utilities
  - `src/bin/lsif.rs`, `src/bin/main.rs`: CLI entry points
- `tests/`: integration/E2E (`*_test.rs`), fixtures under `tests/fixtures`
- `benches/`: Criterion benchmarks
- `scripts/`: `self-index.sh`, `clean.sh`
- `tmp/`: local artifacts (indices, scratch)

## Build, Test, and Development Commands
- Build: `cargo build --release` or `make build`
- Run locally: `cargo run --bin lsif -- <command> [args]`
- Test (all): `cargo test` or `make test`; suites: `make test-unit`, `make test-integration`, `make test-reference`
- Lint/format: `cargo clippy -- -D warnings`, `cargo fmt` or `make check`
- Benchmarks: `cargo bench` or `make bench`
- Self-index + interactive: `./scripts/self-index.sh` then `./target/release/lsif interactive --db tmp/self-index.lsif`

## Coding Style & Naming Conventions
- Rust 2021; 4-space indent; format with `cargo fmt`.
- Keep Clippy clean (`-D warnings`); use `anyhow`/`thiserror` for errors.
- Names: modules/files `snake_case`; types/traits `UpperCamelCase`; fns/vars `snake_case`; constants `SCREAMING_SNAKE_CASE`.
- Separation: CLI in `src/cli`, domain/graph in `src/core`, binaries in `src/bin`.

## Testing Guidelines
- Use `cargo test`. Unit tests live beside code; integration tests under `tests/`.
- Some E2E are `#[ignore]`; run explicitly, e.g.: `cargo test typescript_e2e -- --ignored --nocapture`.
- To mirror CI for integration: `cargo test --test '*' -- --test-threads=1`.
- Prefer adding tests with new features; reuse fixtures in `tests/fixtures`.

## Commit & Pull Request Guidelines
- Conventional Commits: `feat:`, `fix:`, `refactor:`, `test:`, `docs:` (see `git log`).
- Before opening a PR: run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test`.
- PRs should include a clear description, linked issues, and notes on testing; use `.github/pull_request_template.md`.

## Security & Configuration Tips
- Useful env: `RUST_LOG=debug`, `RUST_BACKTRACE=1`.
- Index DB defaults to `.lsif-index.db`; override with `--db <path>` (store under `tmp/` for local runs).

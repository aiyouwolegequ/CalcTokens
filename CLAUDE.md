# Repository Guidelines

## Project Structure & Module Organization

CalcTokens is a Rust workspace. The root package builds the `calctokens` CLI from `src/main.rs`; Antigravity-specific CLI integration lives in `src/antigravity.rs`. Shared token parsing, scanning, aggregation, pricing, caching, and client support live in `crates/calctokens-core/src/`. Keep new reusable logic in `calctokens-core`; keep command parsing, output selection, and CLI wiring in the root `src/` package. Release notes belong in `CHANGELOG.md`; user-facing behavior should stay reflected in `README.md`.

## Build, Test, and Development Commands

- `cargo build`: compile the workspace in debug mode.
- `cargo build --release`: build the optimized CLI at `target/release/calctokens`.
- `cargo test`: run all workspace unit tests.
- `cargo test -p calctokens-core pricing`: run a focused test subset by package and name filter.
- `cargo fmt --check`: verify Rust formatting.
- `cargo clippy --all-targets -- -D warnings`: run lint checks with warnings treated as failures.
- `cargo run -- --no-sync`: run the CLI locally without refreshing external data.

## Coding Style & Naming Conventions

Use standard Rust 2021 style and `rustfmt` defaults. Prefer `snake_case` for functions, variables, modules, and test names; use `PascalCase` for types and traits; use `SCREAMING_SNAKE_CASE` for constants. Match existing module boundaries and avoid broad refactors for narrow fixes. Preserve raw model IDs for audit paths and use canonical IDs only where aggregation or pricing requires them.

## Testing Guidelines

Tests are mostly inline `#[cfg(test)]` modules near the code under test. Add focused unit tests when changing parsers, pricing aliases, client detection, cache behavior, or aggregation logic. Name tests after the behavior being protected, for example `parses_codex_usage_with_cache_tokens`. For changes touching persistent data, prefer temporary directories or test databases over `~/.calctokens.db`.

## Commit & Pull Request Guidelines

Recent history uses concise conventional-style messages such as `fix: add missing kimi-k2.6 identity mapping`, `chore: release v0.9.6`, and `refactor: switch reqwest to rustls-tls`. Keep commits scoped to one concern. Pull requests should include the user-visible change, validation commands run, any database or compatibility impact, and linked issue context when available.

## Agent-Specific Instructions

`CLAUDE.md` is the source of truth. `AGENTS.md` must be a symlink to `CLAUDE.md`; do not edit `AGENTS.md` directly. If guide rules change, update `CLAUDE.md` first and recreate the symlink if needed.

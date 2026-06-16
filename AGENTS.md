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

## Release Workflow

1. Bump version
   - Update the version in `Cargo.toml` (root and workspace members as needed).
   - Add a new section to `CHANGELOG.md` summarizing the release.
   - Commit with a conventional-style message such as `chore: release vX.Y.Z`.

2. Build release binaries locally
   - Run `cargo build --release` to produce the optimized binary at `target/release/calctokens`.
   - Verify the binary works: `./target/release/calctokens --version`.

3. Tag and push
   - Create an annotated tag: `git tag -a vX.Y.Z -m "Release vX.Y.Z"`.
   - Push the tag to trigger CI: `git push origin vX.Y.Z`.

4. GitHub Actions automated release
   - The repository has a GitHub Actions workflow that builds release artifacts for supported targets when a `v*` tag is pushed.
   - Wait for the workflow to complete and publish the release on GitHub.

5. Update the Homebrew formula
   - In the `homebrew-calctokens` repository, edit `Formula/calctokens.rb`.
   - Update the `version` field to the new tag.
   - Update the `sha256` for the macOS artifact (download the release tarball or use `shasum -a 256 <file>`).
   - Validate the formula with `brew audit --strict aiyouwolegequ/calctokens/calctokens` and `brew test aiyouwolegequ/calctokens/calctokens` after the tap is updated. If `brew audit --strict --online` reports a newly published GitHub Release asset as unreachable while `curl -I <asset-url>`, `brew upgrade calctokens`, and `brew test` all succeed, treat it as a transient online reachability false negative and record that fallback explicitly in the release notes.
   - Commit and push the formula change.

6. Upgrade on remote hosts
   - Local macOS (this machine) — run directly:
     ```bash
     brew update && brew upgrade calctokens
     ```
   - MacMini (SSH: `joey@163.7.9.109:6000`) — run:
     ```bash
     export PATH=/opt/homebrew/bin:$PATH
     brew update && brew upgrade calctokens
     ```
   - Jakarta (SSH: `darchrow@163.7.9.109:22223`) — run:
     ```bash
     export PATH=/home/linuxbrew/.linuxbrew/bin:$PATH
     brew update && brew upgrade calctokens
     ```

7. Update project documentation
   - Update the Obsidian project doc at `/Users/felix/Library/Mobile Documents/iCloud~md~obsidian/Documents/Darchrow-Obsidian/Vibe_Coding/Project/CalcTokens.md` with the release notes. Follow the existing version-section format (date, added/fixed/changed subsections) and keep the document in sync with `CHANGELOG.md`.


<claude-mem-context>
# Memory Context

# [CalcTokens] recent context, 2026-06-16 11:59am GMT+8

Legend: 🎯session 🔴bugfix 🟣feature 🔄refactor ✅change 🔵discovery ⚖️decision 🚨security_alert 🔐security_note
Format: ID TIME TYPE TITLE
Fetch details: get_observations([IDs]) | Search: mem-search skill

Stats: 50 obs (18,914t read) | 494,202t work | 96% savings

### May 21, 2026
S3097 Fixed table alignment issues in totaltokens CLI output when displaying CJK/fullwidth characters (May 21 at 10:20 PM)
5945 10:23p 🔵 totaltokens CLI displays multi-machine token usage statistics
5946 10:24p 🔴 Fixed table alignment for full-width Unicode characters in calc-tokens.sh
5947 " ✅ Committed table alignment fix for CJK characters to Hermes-Memory
5948 " ✅ Pushed CJK table alignment fix to remote Hermes-Memory repository
S3098 Complete documentation update for CJK table alignment fix in CalcTokens.md including development history and version tracking (May 21 at 10:25 PM)
5949 10:25p ✅ Documented CJK table alignment fix in CalcTokens project documentation
S3101 CalcTokens v0.9.0 release deployment and verification across multiple platforms, investigating antigravity client data issue (May 21 at 10:26 PM)
5950 10:30p 🔵 Model Naming Inconsistency Identified in CalcTokens
5951 " 🔵 Model Naming Inconsistency Extends Across Remote Systems
5952 10:31p 🔵 Model Aliasing System Defines Canonical and Pretty Names
5953 " 🔵 JSON Output Bypasses Pretty Name Resolution
5954 10:32p 🔵 Pretty Name Resolution Already Used in Non-JSON Output Paths
### May 22, 2026
5961 2:40p 🟣 CalcTokens v0.9.0 released with binary assets
5962 " ✅ Homebrew formula updated to v0.9.0
5963 2:41p ✅ Homebrew formula v0.9.0 deployed to repository
5964 2:42p 🔵 CalcTokens v0.9.0 Homebrew upgrade verified
5966 " 🔵 CalcTokens v0.9.0 verified on remote MacMini
5967 2:43p 🔵 CalcTokens v0.9.0 verified on Linux (Jakarta)
5968 " 🔵 CalcTokens antigravity client filter returns no data
S3102 CalcTokens v0.9.0 documentation update across project README and Obsidian knowledge base (May 22 at 2:44 PM)
5969 2:49p ✅ CalcTokens v0.9.0 documentation updated with agy CLI compatibility and performance optimizations
5970 " ✅ CalcTokens README updated and published for v0.9.0 features
S3126 CalcTokens项目代码优化调查：应用流量统计功能改进，重点关注应用命名展示混乱问题和macOS直连应用功能有效性验证 (May 22 at 2:50 PM)
### May 26, 2026
6248 2:21p 🔵 CalcTokens multi-client architecture and Antigravity macOS process connection system
S3148 Compared two CalcTokens binary files to identify platform differences (May 26 at 2:22 PM)
### May 27, 2026
6300 11:56a 🔵 MCP codex_apps authentication token invalidated
6301 12:45p 🔵 MCP codex_apps client startup failure
6302 " 🔵 codex_apps MCP server network connectivity failure root cause identified
### May 29, 2026
6407 11:52a 🔵 Kimi model name resolution path traced from config to pricing lookup
6408 " 🔵 Kimi model k2.6 missing from MODEL_ALIASES canonical mapping
6409 11:53a 🔵 kimi-k2.6 missing identity mapping in MODEL_ALIASES causes pricing resolution to k2.5
6410 " 🔵 Root cause confirmed: kimi-k2.6 only exists in PRETTY_NAMES, not MODEL_ALIASES
6411 " 🔵 CHANGELOG confirms kimi-k2.6 added as pretty name only in v0.8.5
6412 11:54a 🔵 CalcTokens uses resolve_alias for pricing/storage, resolve_pretty_name for display only
6413 11:55a 🔵 Complete aliases.rs confirms kimi-k2.6 asymmetry between MODEL_ALIASES and PRETTY_NAMES
6414 " 🔵 Agent investigation confirms kimi-k2.6 missing MODEL_ALIASES entry causing k2.5 fallback
6415 4:51p 🔵 CalcTokens binary variants target different platforms
S3149 Unified release binary naming convention to include platform identifiers (May 29 at 4:51 PM)
6416 4:54p ✅ Renamed Linux binary to include platform architecture
6417 4:55p ✅ Updated workflow conditionals and binary output for Linux build
6418 " ✅ Renamed existing Linux binary to match new naming convention
6419 " 🔵 Release binaries are tracked in git without gitignore patterns
6420 " ✅ Added gitignore patterns for compiled binary artifacts
6421 4:56p 🔵 Gitignore patterns successfully exclude renamed binary from tracking
6422 " ✅ Deployed platform-specific binary naming convention to production
S3150 Enhanced release workflow documentation with Obsidian sync step (May 29 at 4:56 PM)
6423 4:57p ✅ Added Obsidian documentation sync step to release workflow
6424 " ✅ Deployed release workflow documentation update to production
S3151 Used workflow to add specific deployment host details to release workflow documentation (May 29 at 4:57 PM)
6425 " ✅ Launched workflow to add specific host details to release upgrade step
6426 " ✅ Expanded release workflow with specific deployment host details
6427 4:58p ✅ Deployed host-specific deployment instructions to production
### Jun 3, 2026
6589 5:38p 🔵 CalcTokens security review scope and architecture
6590 5:41p 🔵 CalcTokens security scan shard A identified two candidate vulnerabilities
### Jun 4, 2026
6600 8:31a ✅ CalcTokens upgraded to 1.0.3 on MacMini remote machine
6601 " 🔵 CalcTokens Security Scan Discovery Phase Completed
### Jun 13, 2026
7308 10:49a ✅ Added Claude-Fable-5 pretty name alias mapping
S3263 Fix Claude Fable-5 display name capitalization in CalcTokens reports and release v1.0.9 (Jun 13 at 10:49 AM)
7309 11:04a 🔄 AGENTS.md converted to symlink

Access 494k tokens of past work via get_observations([IDs]) or mem-search skill.
</claude-mem-context>

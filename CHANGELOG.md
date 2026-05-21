# Changelog

## [0.8.2] - 2026-05-21

### Fixed
- **Model Display Names**: Model column in DETAIL table no longer wraps to next line; removed fixed-width column constraint.
- **Pretty Name Resolution at Display Time**: All model names are now resolved to their pretty display names at render time, ensuring consistent naming across all clients (Claude Code, OpenCode, Gemini CLI, Codex, Kimi CLI), not just Antigravity.

### Added
- **Comprehensive Model Mappings**: Added pretty name mappings for DeepSeek-V4-Flash, Claude-Sonnet-4.5, GPT-5.4-Mini, GPT-5.3-Codex, Kimi-K2.5, MiniMax-M2.7/M2.5, Doubao-Seed-Code, and Gemini preview variants.
- **Database Migrations**: Added migrations for new model pretty names and fixed DeepSeek-V4-Pro capitalization.

## [0.8.1] - 2026-05-21

### Added
- **Model Display Name Unification**: Added mappings and aliases for DeepSeek-v4-Pro, DeepSeek-V3, GPT-5.5, GPT-5.2, GPT-4o, and Claude-Opus-4.7 to display clean pretty names.
- **Database Schema Migration**: Added database migrations to automatically update legacy logs to the new model names.

## [0.8.0] - 2026-05-21

### Added
- **Native Library Migration**: Replaced the external `tokscale-core` library dependency entirely with the native local crate `calctokens-core` in the workspace, fully aligning environment overrides to `CALCTOKENS_*` (e.g. `CALCTOKENS_CONFIG_DIR`) and config/caching to `~/.config/calctokens`.
- **Native Antigravity Sync Hook**: Built native process and port scanner support in `calctokens` to sync active Antigravity workspace DB session states natively, removing python dependencies and manual trigger requirements.

### Optimized
- **Database Performance**: Added substring-based `LIKE` pre-filtering constraints to SQLite queries in `opencode` and `kilo` session parsers, avoiding expensive JSON parsing calls on millions of non-target rows and resolving CPU spikes.

## [0.7.3] - 2026-05-14

### Optimized
- Database: Added `idx_daily_summary_model` index on `daily_summary` table to speed up TOP x COST groupings.
- SQL: Refactored `TOP x COST` query to use a pure SQL `GROUP BY model_id` for accurate and efficient cost aggregation across all clients.

## [0.7.2] - 2026-05-10

### Optimized
- SQLite Performance: Added `busy_timeout = 5000` to handle concurrent access better in WAL mode.
- Database: Added index `idx_history_range_id` to the `history` table for faster snapshot lookups.

## [0.7.1] - 2026-05-10

### Changed
- TOP X COST table: "Share" column now calculates percentage based on CNY (cost) instead of total tokens.

## [0.7.0] - 2026-05-10

### Added
- `--all` flag for all-time usage report with TOP 10 COST and no DELTA section.
- Smarter DELTA comparisons:
  - Default: Today vs last check (cached in history).
  - `--today`: Today vs yesterday's full day data.
  - `--month`: This month vs last month's full data.
- Variable TOP X results based on report type:
  - Default / `--today`: TOP 3 COST.
  - `--month`: TOP 5 COST.
  - `--all`: TOP 10 COST.

### Changed
- Improved display labels: metric labels and delta descriptions are now more context-aware.
- Project License: Officially adopted MIT License (same as tokscale).

### Optimized
- SQLite Performance: Added PRAGMA optimizations including WAL mode, synchronous=NORMAL, and memory-based temp storage for faster reporting and aggregation.

## [0.6.4] - 2026-05-08

### Changed
- Column headers: Cache Write → Cache W, Cache Read → Cache R
- Share percentage now reflects share of total tokens (not total cost)

## [0.6.3] - 2026-05-08

### Changed
- Share display: Unicode bar chart replaced with numeric percentage (e.g. "59.3%")
- DETAIL table: CNY column moved after Model, rows sorted by Total tokens (desc)
- TOP 3 COST: sorted by CNY (desc) instead of Total tokens

## [0.6.2] - 2026-05-08

## [0.6.1] - 2026-05-06

### Fixed
- Share bar visualization now divides by total cost (not top model's cost),
  so each model's bar width accurately reflects its share of total spending

## [0.6.0] - 2026-05-06

### Changed
- Architecture: SQLite is now the authoritative data source instead of ephemeral log files
- All reports (default, monthly, hourly) read from local SQLite database, not tokscale-core
- Added `messages` table storing every message with dedup (97K+ rows supported)
- Added `daily_summary` pre-aggregation table (124 rows from 98K messages, 99.8% reduction)
- Sync: parse log files once, store permanently; subsequent runs only add new messages via `INSERT OR IGNORE`
- Reports persist across client log file deletions — deleting `~/.claude/` no longer loses history

### Added
- `sync_messages()`: parses all client logs and persists to SQLite with dedup
- `refresh_daily_summary()`: rebuilds pre-aggregated table from raw messages
- SQLite-based report queries replace tokscale-core `get_model_report` / `get_monthly_report` / `get_hourly_report`

## [0.5.0] - 2026-05-06

### Changed
- Core migration: replace direct Tokscale API calls with `tokscale-core` library integration
- Async runtime: add `tokio` for proper async API calls

### Added
- `--monthly` view for monthly trend reports
- `--hourly` view for hourly usage history
- `--pricing` view for model pricing lookup with CNY conversion
- `--clients` view showing all detected clients
- `--json-output` flag for all report types (models, monthly, hourly, pricing, clients)
- Time filtering: `--since`, `--until`, `--year` flags
- Enhanced DB schema with `exchange_cache` and `token_cache` tables

## [0.4.5] - 2026-05-06

### Fixed
- Dual-platform CI: matrix workflow now correctly builds and uploads **both** macOS ARM64 and Linux x86_64 binaries
- Homebrew formula `install` method reliability: uses `bin.mkpath + FileUtils.cp + chmod(0755)` for correct binary installation on all platforms
- macOS ARM64 formula: corrected URL pointing to v0.4.2 archive (was causing wrong binary to be downloaded)

## [0.4.3] - 2026-05-06

### Fixed
- GitHub Actions cargo cache contamination: `cargo clean --release` added before build to prevent macOS binary being uploaded as Linux binary

## [0.4.2] - 2026-05-06

### Added
- `--json-output` flag for programmatic JSON output (used by remote aggregation scripts)

## [0.4.1] - 2026-05-06

### Added
- `-c/--client` flag to filter by specific client
- Supports: opencode, claude, codex, gemini, openclaw, kimi, hermes, antigravity, etc.
- Cache per client+range combination

## [0.4.0] - 2026-05-06

### Added
- `--pricing` view showing model pricing with CNY conversion
- `--clients` view showing all detected clients and session counts

### Fixed
- PricingDetail serde rename_all attribute

## [0.3.3] - 2026-05-06

### Added
- SQLite storage for exchange rate and tokscale result caching (daily)
- Delta comparison with last check showing token and cost changes
- New `--monthly` view for monthly trend analysis
- New `--hourly` view for hourly usage history
- Exchange rate and API results cached per day

## [0.3.2] - 2026-05-05

### Added
- Homebrew tap support
- Pre-built binaries for macOS ARM64 and Linux x86_64

## [0.3.1] - 2026-05-05

### Fixed
- Binary filename fix for correct installation

## [0.3.0] - 2026-05-05

### Added
- Token usage by client and model
- K/M/B/T number formatting
- Live USD → CNY exchange rate
- Cache Write / Cache Read token breakdown
- Share bar chart in detail and TOP 3

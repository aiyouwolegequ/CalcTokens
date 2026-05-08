# Changelog

## [0.6.2] - 2026-05-08

### Changed
- Share display: Unicode bar chart replaced with numeric percentage (e.g. "59.3%")
- DETAIL table: CNY column moved after Model, rows sorted by Total tokens (desc)
- TOP 3 COST: sorted by CNY (desc) instead of Total tokens
- Cache Write / Cache Read: zero values hidden (empty cell) instead of showing "0"

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

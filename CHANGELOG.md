# Changelog

## [1.0.7] - 2026-06-08

### Fixed
- **Antigravity SQLite Database Support**: Added `.db` file extension scanning to `get_agy_cli_sessions()` filesystem fallback. This allows CalcTokens to auto-discover and query token metrics for newer agy CLI / Antigravity conversations, which now store trajectory data in local SQLite databases (`.db` files) instead of Protobuf (`.pb`) files.

## [1.0.6] - 2026-06-07

### Fixed
- **Timezone Mismatch in Date Filtering**: Changed query filters and boundary calculations in `calctokens` from `Utc::now()` to `Local::now()`. This ensures that recent token usage (e.g. messages generated shortly after midnight local time) aligns correctly with the local date stored in the database, preventing them from being hidden in "today's" usage report.

### Changed
- **Database Maintenance**: Added `PRAGMA optimize` execution before closing connections to automatically analyze tables and indexes when significant changes occur, maintaining optimal SQLite query plans.

## [1.0.5] - 2026-06-05

### Added
- **Kimi Code Support**: Added parsing support for the new Kimi Code CLI (v0.10.1+) which stores data in `~/.kimi-code/sessions/` using a new `usage.record` JSONL format.
- **Model Aliases**: Added `kimi-code/kimi-for-coding` mapping to `Kimi-K2.6` (`kimi-k2.6`) for correct pricing and display.
- **Robust Path & Config Handling**: Enhanced session UUID extraction to parse nested agent paths (`/agents/main/wire.jsonl`) and implemented recursive ancestor configuration lookup to parse both TOML config (`config.toml`) and JSON config (`config.json`).

## [1.0.4] - 2026-06-04

### Fixed
- **OpenRouter Pricing Sync**: Fixed a pricing sync bug where models from providers not explicitly mapped in `get_author_provider_name` (e.g. MiniMax, ByteDance/Doubao) were completely dropped during OpenRouter models fetch.
- **Cache Read/Write Pricing**: Added support for parsing cache read and cache write prices directly from OpenRouter models list response.
- **Doubao Seed Code Pricing**: Mapped `doubao-seed-code` to `seed-1.6` so it correctly resolves to OpenRouter's `bytedance-seed/seed-1.6` pricing.
- **Aggregated Detail Table Filter**: Updated `calc-tokens.sh` to filter aggregated models by total tokens > 0 instead of cost > 0, ensuring models with missing pricing or zero cost still appear in the report.

## [1.0.3] - 2026-06-03

### Changed
- **OpenRouter-only Pricing**: Removed the LiteLLM pricing module from the pricing pipeline. Pricing now uses OpenRouter as the single reference source.
- **Agent Model Attribution**: Improved provider inference for models used through Claude Code, OpenCode, Codex, Gemini CLI, Kimi CLI, MiniMax CLI, and Antigravity, including DeepSeek, MiniMax, and Kimi/Moonshot models.

### Fixed
- **Model Identification Accuracy**: Improved model normalization and provider attribution to avoid collapsing newer or third-party model IDs into older same-family aliases.
- **Security Hardening**: Added bounds and validation across Antigravity sync responses, OpenRouter model sync fanout, local session log parsing, parser arithmetic, environment-derived roots, Claude sidechain parent lookup, exchange-rate metadata, and pricing cache reads.
- **Validation Cleanup**: Resolved root binary clippy findings so `cargo clippy --all-targets -- -D warnings` passes for the full workspace.

## [1.0.2] - 2026-05-29

### Fixed
- **GPT-OSS Mapping**: Ensured `gpt-oss-120b-medium` is correctly resolved and displayed as `GPT-OSS-120B` even when provided as a raw model ID.

## [1.0.1] - 2026-05-29

### Fixed
- **M18 Placeholder Resolution**: Added alias mappings for `MODEL_PLACEHOLDER_M18` to correctly resolve to `Gemini-3.5-Flash` in `calctokens-core`.
- **GPT-OSS Display Name**: Renamed `GPT-OSS-120B-Medium` to `GPT-OSS-120B` in `MODEL_ALIASES` and `PRETTY_NAMES` for cleaner output alignment.

## [1.0.0] - 2026-05-29

### Added
- **Top X Usage Report**: Added a new "TOP X USAGE" table below the existing "TOP X COST" section. This new table summarizes the highest-volume models sorted by their combined total tokens (input + output + cache read/write) and calculates their share against the total volume, giving clearer visibility into raw consumption alongside financial cost.

## [0.9.9] - 2026-05-29

### Added
- **M132 Placeholder Resolution**: Added alias mappings for `MODEL_PLACEHOLDER_M132` to correctly resolve to `Gemini-3.5-Flash` in `calctokens-core`, fixing an issue where raw placeholder strings appeared in the aggregated usage logs.

## [0.9.8] - 2026-05-29

### Added
- **Claude Opus/Sonnet 4.8 Support**: Added `claude-opus-4-8` and `claude-sonnet-4-8` identity mappings, pretty names, and `normalize_model_name` resolution to prevent incorrect fallback to older versions.
- **Claude Haiku 4.5 Pretty Names**: Added missing `MODEL_ALIASES` and `PRETTY_NAMES` entries for `claude-haiku-4-5` and its date-suffixed variant, ensuring consistent display as `Claude-Haiku-4.5`.

### Changed
- **Pricing Source**: CalcTokens now loads pricing data exclusively from OpenRouter, removing the LiteLLM fetch path. This simplifies the pricing pipeline and avoids stale or subscription-based ($0.00) entries from LiteLLM.

### Fixed
- **Claude Sonnet 4.6 Misclassification**: `normalize_model_name` now correctly resolves `claude-sonnet-4-6` instead of falling back to `claude-sonnet-4`.
- **GPT-OSS Pretty Name Format**: Changed display name from `GPT-OSS-120B（Medium）` (full-width brackets) to `GPT-OSS-120B-Medium` for consistency with other model names.

## [0.9.7] - 2026-05-29

### Fixed
- **Kimi Model Aliasing**: Added missing `kimi-k2.6` identity mapping in `MODEL_ALIASES`, preventing incorrect fallback to `kimi-k2.5` during pricing lookup and token aggregation.

## [0.9.6] - 2026-05-28

### Added
- **Agy Health Check Alert**: Add warning message to stderr when the `agy` daemon process is detected running, but 0 new messages are synced (improving visibility for compatibility breakages during agy updates).
- **Agy Probing Diagnostics**: Added stderr warning alerts when ports fail to probe or `GetAllCascadeTrajectories` returns HTTP/JSON errors.

## [0.9.5] - 2026-05-28

### Fixed
- **Antigravity Sync Timeout**: Increased the request connection/handshake timeout in `sync_antigravity()` from 500ms to 2000ms to prevent silent sync failures on slower/busy environments under load.

## [0.9.4] - 2026-05-26

### Optimized
- **Antigravity Port Probing**: Parallelized loopback port probing for multiple candidate daemons, and reduced connection timeout from 1500ms to 500ms to eliminate accumulated network delays.
- **Fingerprinting Optimization**: Limited the database prefix hash length to at most 10MB to prevent `hash_prefix` from parsing huge database files (e.g. 2.4 GB `opencode.db`), reducing fingerprinting latency from several seconds to less than 15 milliseconds.
- **Unit Testing**: Isolated the Zed path resolution unit test from host `XDG_DATA_HOME` environment variables to prevent testing failures under customized environments.

## [0.9.3] - 2026-05-26

### Fixed
- **Antigravity synchronization**: Expanded session discovery fallback to scan not only `~/.gemini/antigravity-cli/` but also `~/.gemini/antigravity/` and `~/.gemini/antigravity-ide/` conversations, and de-duplicate sessions by keeping the most recently modified `.pb` file.

## [0.9.2] - 2026-05-23

### Added
- **Positional Subcommands**: Support running standard reports without `--` prefix (e.g., `calctokens today`, `calctokens month`, `calctokens all`, `calctokens upgrade`, etc.) as convenient positional subcommands.

## [0.9.1] - 2026-05-23

### Fixed
- **Antigravity model canonicalization**: Added `gemini-3.5-flash` self-alias mapping to prevent base models from canonicalizing to capitalized `Gemini-3.5-Flash` and causing split rows in detail views.
- **DB migration**: Added automatic database migration to fix casing of existing `canonical_id` entries in `messages` and clean up stale duplicates in `daily_summary`.

## [0.9.0] - 2026-05-22

### Added
- **`--no-sync` flag**: Skip message sync and daily_summary refresh for read-only historical queries. Reports return in ~5ms instead of ~5s.
- **agy CLI support**: Auto-discover agy CLI sessions from `~/.gemini/antigravity-cli/conversations/*.pb` when `GetAllCascadeTrajectories` API returns empty (agy v1.0.1+ / Antigravity v2.0.1+).

### Changed
- **`daily_summary` persistence**: Table is no longer dropped and recreated on every invocation. Uses `INSERT OR REPLACE` upsert semantics, and refresh is skipped when no new messages are synced.
- **Pricing cache**: FIFO eviction (deterministic) replaces HashMap random eviction for stable cache hit rates.
- **Scanner**: `par_bridge` replaced with collect + `into_par_iter` to eliminate rayon mutex contention during file scanning.

### Performance
- **`lsof` calls**: Merged N per-process `lsof` calls into a single `lsof -iTCP -sTCP:LISTEN` for Antigravity port discovery.
- **Heartbeat probing**: HTTP and HTTPS probes now run in parallel (worst-case latency halved).
- **New indexes**: `messages(timestamp)` and `messages(date, client)` compound index for time-range and filtered queries.

### Fixed
- **Antigravity sync**: agy CLI v1.0.1 sessions are now discovered via filesystem fallback when the gRPC trajectory listing API returns empty.
- **API response format**: Handle new agy CLI response where token values are strings and `responseOutputTokens` is separate from `outputTokens`.

### Technical
- **Response parsing**: `process_trajectory()` falls back to `chatModel.usage` directly when `retryInfos` is empty.
- **Output token resolution**: `resolve_output_and_reasoning()` correctly separates visible output from thinking tokens in the new API format.

## [0.8.8] - 2026-05-22

### Fixed
- **Gemini Flash canonical split**: Antigravity models (`gemini-3-flash-a/high/flash/flash-c/m47`) now canonicalize to `gemini-3.5-flash` → display "Gemini-3.5-Flash". Gemini CLI (`gemini-3-flash-preview`) stays on `gemini-3-flash-preview` → display "Gemini-3.1-Flash". Verified against OpenRouter: `google/gemini-3-flash-preview` and `google/gemini-3.5-flash` are distinct models.
- **DB migration**: Re-backfills Antigravity messages previously mis-canonicalized.

## [0.8.7] - 2026-05-22

### Fixed
- **Gemini Flash naming**: `gemini-3-flash-preview` canonical now displays as "Gemini-3.1-Flash" (previously incorrectly labeled "Gemini-3.5-Flash", which is a different OpenRouter model).

## [0.8.6] - 2026-05-22

### Changed
- **Display Names**: Claude-Sonnet-4.6 and Claude-Opus-4.6 no longer show "(Thinking)" suffix (all variants merged into one canonical group).
- **MiniMax Casing**: HighSpeed → Highspeed (lowercase 's') for both M2.7 and M2.5 models.
- **GLM-5.1**: `z-ai/glm-5.1` now displays as `GLM-5.1`.
- **Authors**: Updated to Felix Lau.

## [0.8.5] - 2026-05-22

### Added
- **Canonical ID Layer**: `messages` table now has `canonical_id` column — raw `model_id` preserved as observation, `canonical_id` computed via `resolve_alias()` for stable aggregation. Historical data is never rewritten.
- **`calctokens --upgrade`**: New subcommand to sync OpenRouter model metadata (237 models) and live exchange rates to local SQLite (`openrouter_models` + `exchange_rates` tables).
- **OpenRouter Metadata Tables**: `openrouter_models` (model→pricing mapping) and `exchange_rates` (historical rate tracking) for persistent metadata.
- **Kimi Model Pretty Names**: Added `kimi-k2.6`, `k2p5`, `k2-p5`, `kimi-latest` mappings.

### Changed
- **Database Schema**: `daily_summary` rebuilt — PK changed from `(date, client, model_id)` to `(date, client, canonical_id)`. Same-model variants (High/Low, Thinking/non-Thinking) now merge into one canonical group.
- **Display Name**: `gemini-3.1-pro` now shows as "Gemini-3.1-Pro" (without "(Low)" suffix, since High/Low variants are merged).

### Removed
- **Pretty-Name UPDATE Migrations**: Removed 18 `UPDATE messages SET model_id = 'PrettyName'` statements from `init_db()` that overwrote historical observation data.

### Fixed
- **Gemini-3.1-Pro Display**: After canonical merge of High/Low variants, display name no longer misleadingly shows tier suffix.

## [0.8.4] - 2026-05-21

### Fixed
- **Silent Antigravity Sync**: Removed all diagnostic output from the Antigravity session synchronizer. Sync runs silently in the background and no longer pollutes stdout (which was breaking `--json-output` parsing in multi-machine aggregation scripts).

## [0.8.3] - 2026-05-21

### Added
- `--version` / `-V` CLI option for printing the installed calctokens version.

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

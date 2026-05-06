# Changelog

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

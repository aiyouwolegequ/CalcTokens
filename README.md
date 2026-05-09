# calctokens

Token usage report powered by [tokscale-core](https://github.com/junhoyeo/tokscale) with human-readable K/M/B units & RMB conversion.

Data is persisted in SQLite and survives client log file deletions — delete `~/.claude/` or any other client logs without losing history.

## Features

- Token usage by client and model
- All messages stored permanently in `~/.calctokens.db` — independent of source log files
- Pre-aggregated `daily_summary` for fast model/monthly reports (99.8% row reduction)
- K/M/B/T number formatting
- Live USD → CNY exchange rate
- Cache Write / Cache Read token breakdown
- Share bar chart in detail and TOP 3 (accurate percentage of total cost)
- SQLite storage with delta comparison (since last check)
- Daily caching for exchange rate and API results
- Monthly & Hourly trend reports
- Client filtering (`-c/--client`)
- Pricing lookup with CNY conversion (`--pricing`)
- Clients overview (`--clients`)
- Time filtering: `--since`, `--until`, `--year`
- `--json-output` flag for all report types

## Install

### macOS / Linux (Homebrew)

```bash
brew install aiyouwolegequ/homebrew-calctokens/calctokens
```

Works on macOS (arm64) and Linux (x86_64). No additional dependencies required.

### Build from source

```bash
cargo install --git https://github.com/aiyouwolegequ/CalcTokens
```

### Build manually

```bash
git clone https://github.com/aiyouwolegequ/CalcTokens.git
cd CalcTokens
cargo build --release
cp target/release/calctokens ~/.local/bin/   # macOS
sudo cp target/release/calctokens /usr/local/bin/   # Linux
```

## Usage

```bash
calctokens                     # today's usage vs last check (cached)
calctokens --today             # today's usage vs yesterday (full)
calctokens --month             # this month vs last month (full)
calctokens --all               # all-time usage, TOP 10 COST, no delta
calctokens --monthly           # monthly trend report
calctokens --hourly            # hourly usage history
calctokens --pricing MODEL_ID  # model pricing lookup (CNY)
calctokens --clients           # all detected clients
calctokens -c claude           # filter by client
calctokens -c kimi --month     # filter by client + time range
calctokens --since 2026-01-01  # filter by start date
calctokens --year 2026          # filter by year
calctokens --json-output        # output JSON for scripts
```

**Reporting Logic:**
- **Default:** Shows **Today** vs **Last Check**, TOP 3 COST.
- **`--today`:** Shows **Today** vs **Yesterday**, TOP 3 COST.
- **`--month`:** Shows **This Month** vs **Last Month**, TOP 5 COST.
- **`--all`:** Shows **All-time**, TOP 10 COST.

**Supported clients:** `opencode`, `claude`, `codex`, `gemini`, `openclaw`, `kimi`, `hermes`, `antigravity`, etc.

## Output

```
  calctokens  --  Token Usage Report   [ TODAY ]

  SUMMARY
╭────────┬───────┬────────┬─────────┬─────────┬────────┬────────╮
│ Metric │ Input │ Output │ Cache W │ Cache R │ Total  │ CNY    │
├────────┼───────┼────────┼─────────┼─────────┼────────┼────────┤
│ TODAY  │ 4.58M │ 34.27K │ 301.65K │ 6.29M   │ 11.21M │ ¥12.03 │
╰────────┴───────┴────────┴─────────┴─────────┴────────┴────────╯

  DELTA (vs yesterday)
╭──────────────┬─────────┬──────────┬───────────┬───────────┬─────────┬─────────╮
│ Δ Metric     │ Δ Input │ Δ Output │ Δ Cache W │ Δ Cache R │ Δ Total │ Δ CNY   │
├──────────────┼─────────┼──────────┼───────────┼───────────┼─────────┼─────────┤
│ vs yesterday │ +1.2M   │ +5.2K    │ +100K     │ +2.1M     │ +3.4M   │ +¥2.50  │
╰──────────────┴─────────┴──────────┴───────────┴───────────┴─────────┴─────────╯

  DETAIL
╭───────┬───────────────────────┬────────┬───────┬─────────┬─────────┬───────┬───────┬───────╮
│Client │Model                  │CNY     │Input  │Output   │Cache W  │Cache R│Total  │Share  │
├───────┼───────────────────────┼────────┼───────┼─────────┼─────────┼───────┼───────┼───────┤
│claude │minimax-m2.7-highspeed │¥10.00  │4.14M  │19.69K   │301.65K  │2.03M  │6.49M  │57.9%  │
│claude │minimax-m2.7           │¥2.03   │444.90K│14.58K   │0        │4.26M  │4.72M  │42.1%  │
╰───────┴───────────────────────┴────────┴───────┴─────────┴─────────┴───────┴───────┴───────╯

  TOP 3 COST
╭───┬────────────────────────┬───────┬────────┬───────╮
│ # │ Model                  │ Total │ CNY    │ Share │
├───┼────────────────────────┼───────┼────────┼───────┤
│ 1 │ minimax-m2.7-highspeed │ 6.49M │ ¥10.00 │ 57.9% │
│ 2 │ minimax-m2.7           │ 4.72M │ ¥2.03  │ 42.1% │
╰───┴────────────────────────┴───────┴────────┴───────╯
```

## Tech Stack

- Rust
- `tokscale-core` — token data engine (async, multi-client)
- `tokio` — async runtime
- `clap` — CLI argument parsing
- `reqwest` — HTTP client for exchange rate API
- `serde` / `serde_json` — JSON serialization
- `rusqlite` — SQLite authoritative data store with WAL optimization

## License

MIT (same as [tokscale](https://github.com/junhoyeo/tokscale))


# calctokens

AI coding assistant token usage tracker with human-readable K/M/B/T units & real-time CNY conversion. Data persisted in SQLite — survives client log deletions.

## Features

- **OpenRouter-standard model naming**: models canonicalized and displayed with unified pretty names
- **`calctokens --upgrade`**: sync OpenRouter model metadata + exchange rates to local DB
- **Canonical ID layer**: raw `model_id` preserved for audit, `canonical_id` used for aggregation — historical data never rewritten
- All messages stored permanently in `~/.calctokens.db` — independent of source log files
- Pre-aggregated `daily_summary` for fast model/monthly reports
- K/M/B/T number formatting
- Live USD → CNY exchange rate with daily caching + history tracking
- Cache Write / Cache Read token breakdown
- Share percentages (tokens in detail, cost in TOP X)
- Intelligent delta comparison (last check, yesterday, or last month)
- Monthly & Hourly trend reports
- **`--no-sync` flag**: skip sync+refresh for ~5ms read-only reports (~700x speedup)
- **agy CLI support**: auto-discover Antigravity CLI sessions when gRPC listing API is unavailable
- Client filtering (`-c/--client`)
- Pricing lookup with CNY conversion (`--pricing`)
- Clients overview (`--clients`)
- Time filtering: `--since`, `--until`, `--year`
- `--json-output` flag for all report types
- Multi-machine aggregation via `totaltokens` script
- Multi-client support: Claude Code, OpenCode, Codex, Gemini CLI, Kimi CLI, Antigravity, etc.

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
calctokens --version           # print version
calctokens --upgrade           # sync OpenRouter metadata + exchange rates
calctokens -c claude           # filter by client
calctokens -c kimi --month     # filter by client + time range
calctokens --no-sync            # skip sync for instant reports (~5ms)
calctokens --since 2026-01-01  # filter by start date
calctokens --year 2026         # filter by year
calctokens --json-output       # output JSON for scripts
```

**Reporting Logic:**
- **Default:** Shows **Today** vs **Last Check**, TOP 3 COST.
- **`--today`:** Shows **Today** vs **Yesterday**, TOP 3 COST.
- **`--month`:** Shows **This Month** vs **Last Month**, TOP 5 COST.
- **`--all`:** Shows **All-time**, TOP 10 COST.
- **`--no-sync`:** Skip message sync and refresh. Works with any report type for instant (~5ms) read-only queries. Useful for historical browsing, script aggregation, and repeated lookups.

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
╭───────┬────────────────────────┬────────┬───────┬─────────┬─────────┬───────┬───────┬───────╮
│Client │Model                   │CNY     │Input  │Output   │Cache W  │Cache R│Total  │Share  │
├───────┼────────────────────────┼────────┼───────┼─────────┼─────────┼───────┼───────┼───────┤
│claude │MiniMax-M2.7-HighSpeed  │¥10.00  │4.14M  │19.69K   │301.65K  │2.03M  │6.49M  │57.9%  │
│claude │MiniMax-M2.7            │¥2.03   │444.90K│14.58K   │0        │4.26M  │4.72M  │42.1%  │
╰───────┴────────────────────────┴────────┴───────┴─────────┴─────────┴───────┴───────┴───────╯

  TOP 3 COST
╭───┬────────────────────────┬───────┬────────┬───────╮
│ # │ Model                  │ Total │ CNY    │ Share │
├───┼────────────────────────┼───────┼────────┼───────┤
│ 1 │ MiniMax-M2.7-HighSpeed │ 6.49M │ ¥10.00 │ 57.9% │
│ 2 │ MiniMax-M2.7           │ 4.72M │ ¥2.03  │ 42.1% │
╰───┴────────────────────────┴───────┴────────┴───────╯
```

## Architecture

```
messages.model_id (原始，append-only)
        │
        ▼ resolve_alias()
messages.canonical_id (归一化，用于聚合)
        │
        ▼ GROUP BY canonical_id
daily_summary (预聚合表)
        │
        ▼ resolve_pretty_name()
报表展示 (美化名称)
```

Historical `messages.cost` is computed at insert time and never backfilled — price changes don't affect recorded data.

## Database

`~/.calctokens.db` (SQLite, WAL mode)

| Table | Purpose |
|-------|---------|
| `messages` | Authoritative raw data (model_id + canonical_id, append-only) |
| `daily_summary` | Pre-aggregated by (date, client, canonical_id) |
| `openrouter_models` | OpenRouter model metadata + pricing, upserted by `--upgrade` |
| `exchange_rates` | USD/CNY rate history |
| `exchange_cache` | Daily rate cache |
| `history` | Snapshot history for delta comparison |

## Tech Stack

- Rust
- `calctokens-core` — built-in workspace crate (async, multi-client token engine)
- `tokio` — async runtime
- `clap` — CLI argument parsing
- `reqwest` — HTTP client (exchange rate + OpenRouter APIs)
- `serde` / `serde_json` — JSON serialization
- `rusqlite` — SQLite with WAL & concurrency optimizations
- `tabled` — terminal table rendering
- `chrono` — date/time handling

## License

MIT

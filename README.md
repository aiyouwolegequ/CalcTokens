# calctokens

Token usage report from [Tokscale](https://tokscale.com) with human-readable K/M/B units & RMB conversion.

## Features

- Token usage by client and model from Tokscale API
- K/M/B/T number formatting
- Live USD → CNY exchange rate
- Cache Write / Cache Read token breakdown
- Share bar chart in detail and TOP 3
- SQLite storage with delta comparison (since last check)
- Daily caching for exchange rate and API results
- Monthly & Hourly usage reports

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

> `tokscale login` — only run if `tokscale models` fails without login.

## Usage

```bash
calctokens --all     # all-time usage (default)
calctokens --today  # today's usage
calctokens --month  # current month usage
calctokens --monthly # monthly trend report
calctokens --hourly # hourly usage history
```

## Output

```
  calctokens  --  Token Usage Report   [ Today ]

  SUMMARY
╭────────┬───────┬────────┬─────────────┬────────────┬────────┬────────╮
│ Metric │ Input │ Output │ Cache Write │ Cache Read │ Total  │ CNY    │
├────────┼───────┼────────┼─────────────┼────────────┼────────┼────────┤
│ TODAY  │ 4.58M │ 34.27K │ 301.65K     │ 6.29M      │ 11.21M │ ¥12.03 │
╰────────┴───────┴────────┴─────────────┴────────────┴────────┴────────╯
  DETAIL
╭───────┬───────────────────────┬────────┬───────┬────────────┬───────────┬──────┬───────┬─────────────────────╮
│Client │Model                  │Input   │Output │Cache Write │Cache Read │Total │CNY    │Share                │
├───────┼───────────────────────┼────────┼───────┼────────────┼───────────┼──────┼───────┼─────────────────────┤
│claude │minimax-m2.7-highspeed │4.14M   │19.69K │301.65K     │2.03M      │6.49M │¥10.00 │████████████████████ │
│claude │minimax-m2.7           │444.90K │14.58K │0           │4.26M      │4.72M │¥2.03  │████░░░░░░░░░░░░░░░░ │
╰───────┴───────────────────────┴────────┴───────┴────────────┴───────────┴──────┴───────┴─────────────────────╯
  TOP 3 COST
╭───┬────────────────────────┬───────┬────────┬────────────╮
│ # │ Model                  │ Total │ CNY    │ Share      │
├───┼────────────────────────┼───────┼────────┼────────────┤
│ 1 │ minimax-m2.7-highspeed │ 6.49M │ ¥10.00 │ ██████████ │
│ 2 │ minimax-m2.7           │ 4.72M │ ¥2.03  │ ██░░░░░░░░ │
╰───┴────────────────────────┴───────┴────────┴────────────╯
```

## Tech Stack

- Rust
- `clap` — CLI argument parsing
- `reqwest` — HTTP client for exchange rate API
- `serde` / `serde_json` — JSON parsing
- `tokscale` — data source (external CLI)

## License

MIT

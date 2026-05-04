# CalcTokens

Token usage report from [Tokscale](https://tokscale.com) with human-readable K/M/B units & RMB conversion.

## Features

- Token usage by client and model from Tokscale API
- K/M/B/T number formatting
- Live USD → CNY exchange rate
- Cache Write / Cache Read token breakdown
- Share bar chart in detail and TOP 3

## Install

### macOS

```bash
# Install tokscale
brew install tokscale

# Build CalcTokens
cargo build --release
cp target/release/CalcTokens ~/.local/bin/
```

### Ubuntu / Linux

```bash
# Install Node.js (required for tokscale)
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs

# Install tokscale
npm install -g @anthropic-ai/tokscale-cli

# Build CalcTokens
cargo build --release
sudo cp target/release/CalcTokens /usr/local/bin/
```

> `tokscale login` — only run if `tokscale models` fails without login.

### Alternative: cargo install

```bash
cargo install --path .
```

## Usage

```bash
CalcTokens --all    # all-time usage
CalcTokens --today  # today's usage
CalcTokens --month  # current month usage
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

# CalcTokens

Token usage report from [Tokscale](https://tokscale.com) with human-readable K/M/B units & RMB conversion.

## Features

- Token usage by client and model from Tokscale API
- K/M/B/T number formatting
- Live USD → CNY exchange rate
-费用占比 bar chart
- TOP 3 cost ranking

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
┌──────────────────────────────────────────────────────────────────────────┐
│  📊  CalcTokens  使用报告          全部                                      │
│  💱  1 USD = ¥6.8400 CNY  (实时汇率)                                         │
└──────────────────────────────────────────────────────────────────────────┘

┌──────────────────┬───────────┬───────────┬───────────┬───────────────┐
│ 指标               │   Input   │  Output   │    USD    │          ¥CNY │
├──────────────────┼───────────┼───────────┼───────────┼───────────────┤
│ 总计               │     2.52B │    29.07M │   1188.56 │ ¥     8129.74 │
└──────────────────┴───────────┴───────────┴───────────┴───────────────┘

┌────────────────────────────────────────────────────────────────────────────────────────┐
│  Client  │            Model             │  Input  │ Output  │   USD   │         费用占比         │
├────────┼──────────────────────────────┼─────────┼─────────┼─────────┼────────────────────┤
│  claude  │ minimax-m2.7-highspeed       │   1.93B │  12.15M │  660.04 │ ████████████████████ │
│  claude  │ kimi-for-coding              │ 294.40M │   6.62M │  315.43 │ ██████████░░░░░░░░░░ │
└────────────────────────────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────┐
│           💰 费用 TOP 3                   │
├────┬────────────────────┬────────────────┤
│ #  │       Model        │      ¥CNY      │
├────┼────────────────────┼────────────────┤
│ 1  │ minimax-m2.7-highs │ ¥     4514.66 │ ██████████ │
│ 2  │  kimi-for-coding   │ ¥     2157.55 │ █████░░░░░ │
│ 3  │    minimax-m2.7    │ ¥     1291.07 │ ███░░░░░░░ │
└────┴────────────────────┴────────────────┘
```

## Tech Stack

- Rust
- `clap` — CLI argument parsing
- `reqwest` — HTTP client for exchange rate API
- `serde` / `serde_json` — JSON parsing
- `tokscale` — data source (external CLI)

## License

MIT

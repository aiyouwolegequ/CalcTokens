use clap::Parser;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::process::Command;

const EXCH_API: &str = "https://api.exchangerate-api.com/v4/latest/USD";

#[derive(Parser, Debug)]
#[command(name = "CalcTokens")]
#[command(about = "Token usage report from Tokscale with K/M/B units & RMB conversion", long_about = None)]
struct Args {
    #[arg(long)]
    today: bool,
    #[arg(long)]
    month: bool,
    #[arg(long)]
    all: bool,
}

#[derive(Deserialize, Debug)]
struct ExchangeResp {
    rates: Rates,
}
#[derive(Deserialize, Debug)]
struct Rates {
    CNY: f64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ModelsResp {
    total_input: Option<f64>,
    total_output: Option<f64>,
    total_cost: Option<f64>,
    entries: Vec<Entry>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct Entry {
    client: String,
    model: String,
    input: f64,
    output: f64,
    cost: f64,
}

fn fmt_num(n: f64) -> String {
    if n >= 1_000_000_000_000.0 {
        format!("{:.2}T", n / 1_000_000_000_000.0)
    } else if n >= 1_000_000_000.0 {
        format!("{:.2}B", n / 1_000_000_000.0)
    } else if n >= 1_000_000.0 {
        format!("{:.2}M", n / 1_000_000.0)
    } else if n >= 1_000.0 {
        format!("{:.2}K", n / 1_000.0)
    } else {
        format!("{:.0}", n)
    }
}

fn run_tokscale(args: &[&str]) -> ModelsResp {
    let output = Command::new("tokscale")
        .args(args)
        .output()
        .expect("failed to execute tokscale");
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).expect("failed to parse tokscale JSON")
}

fn make_bar(cost: f64, max_cost: f64, width: usize) -> String {
    let filled = if max_cost > 0.0 && cost > 0.0 {
        ((cost / max_cost) * width as f64).round() as usize
    } else {
        0
    };
    let filled = filled.min(width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

fn main() {
    let args = Args::parse();

    let (range_flag, label) = if args.today {
        ("--today", "今日")
    } else if args.month {
        ("--month", "本月")
    } else {
        ("", "全部")
    };

    // Fetch exchange rate
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .unwrap();
    let exchange: f64 = client
        .get(EXCH_API)
        .send()
        .ok()
        .and_then(|r| r.json::<ExchangeResp>().ok())
        .map(|e| e.rates.CNY)
        .unwrap_or(7.2);

    // Fetch tokscale data
    let mut tok_args = vec!["models", "--json"];
    if !range_flag.is_empty() {
        tok_args.push(range_flag);
    }
    let data: ModelsResp = run_tokscale(&tok_args);

    let total_in = data.total_input.unwrap_or(0.0);
    let total_out = data.total_output.unwrap_or(0.0);
    let total_cost = data.total_cost.unwrap_or(0.0);
    let total_rmb = total_cost * exchange;

    let max_cost = data
        .entries
        .iter()
        .map(|e| e.cost)
        .fold(0.0_f64, f64::max);

    // Sort by cost desc
    let mut entries = data.entries.clone();
    entries.sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap());

    // ── Render ────────────────────────────────────────────────────
    println!();
    println!("┌──────────────────────────────────────────────────────────────────────────┐");
    println!(
        "│  📊  CalcTokens  使用报告          {}                                      │",
        label
    );
    println!(
        "│  💱  1 USD = ¥{:.4} CNY  (实时汇率)                                         │",
        exchange
    );
    println!("└──────────────────────────────────────────────────────────────────────────┘");
    println!();
    println!(
        "┌──────────────────┬───────────┬───────────┬───────────┬───────────────┐"
    );
    println!(
        "│ {:<16} │ {:^9} │ {:^9} │ {:^9} │ {:>13} │",
        "指标", "Input", "Output", "USD", "¥CNY"
    );
    println!(
        "├──────────────────┼───────────┼───────────┼───────────┼───────────────┤"
    );
    println!(
        "│ {:<16} │ {:>9} │ {:>9} │ {:>9.2} │ ¥{:>12.2} │",
        "总计",
        fmt_num(total_in),
        fmt_num(total_out),
        total_cost,
        total_rmb
    );
    println!(
        "└──────────────────┴───────────┴───────────┴───────────┴───────────────┘"
    );
    println!();

    println!(
        "┌────────────────────────────────────────────────────────────────────────────────────────┐"
    );
    println!(
        "│ {:^8} │ {:^28} │ {:^7} │ {:^7} │ {:^7} │ {:^20} │",
        "Client", "Model", "Input", "Output", "USD", "费用占比"
    );
    println!(
        "├────────┼──────────────────────────────┼─────────┼─────────┼─────────┼────────────────────┤"
    );

    for entry in &entries {
        println!(
            "│ {:^8} │ {:<28} │ {:>7} │ {:>7} │ {:>7.2} │ {} │",
            entry.client,
            entry.model,
            fmt_num(entry.input),
            fmt_num(entry.output),
            entry.cost,
            make_bar(entry.cost, max_cost, 20)
        );
    }
    println!(
        "└────────────────────────────────────────────────────────────────────────────────────────┘"
    );
    println!();

    // TOP 3
    println!("┌──────────────────────────────────────────┐");
    println!("│           💰 费用 TOP 3                   │");
    println!("├────┬────────────────────┬────────────────┤");
    println!("│ {:^2} │ {:^18} │ {:^14} │", "#", "Model", "¥CNY");
    println!("├────┼────────────────────┼────────────────┤");

    for (i, entry) in entries.iter().filter(|e| e.cost > 0.0).take(3).enumerate() {
        let model_short = if entry.model.len() > 18 {
            entry.model[..18].to_string()
        } else {
            entry.model.clone()
        };
        println!(
            "│ {:^2} │ {:^18} │ ¥{:>12.2} │ {} │",
            i + 1,
            model_short,
            entry.cost * exchange,
            make_bar(entry.cost, max_cost, 10)
        );
    }
    println!("└────┴────────────────────┴────────────────┘");
    println!();
}

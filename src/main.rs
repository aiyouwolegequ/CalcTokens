use clap::Parser;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::process::Command;
use tabled::builder::Builder;
use tabled::settings::{object::Segment, Padding, Style, Modify, Width};

const EXCH_API: &str = "https://api.exchangerate-api.com/v4/latest/USD";

#[derive(Parser, Debug)]
#[command(name = "calctokens")]
struct Args {
    #[arg(long, conflicts_with_all = ["month", "all"])]
    today: bool,
    #[arg(long, conflicts_with_all = ["today", "all"])]
    month: bool,
    #[arg(long, conflicts_with_all = ["today", "month"])]
    all: bool,
}

impl Default for Args {
    fn default() -> Self { Args { today: false, month: false, all: true } }
}

fn resolve_range(args: &Args) -> (&'static str, &'static str) {
    if args.today   { ("Today", "--today") }
    else if args.month { ("This Month", "--month") }
    else            { ("All Time", "") }
}

#[derive(Deserialize, Debug)]
struct ExchangeResp { rates: Rates }
#[derive(Deserialize, Debug)]
struct Rates { #[serde(rename = "CNY")] cny: f64 }

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ModelsResp {
    total_input: Option<f64>,
    total_output: Option<f64>,
    total_cache_read: Option<f64>,
    total_cache_write: Option<f64>,
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
    cache_read: f64,
    cache_write: f64,
    cost: f64,
}

fn fmt_num(n: f64) -> String {
    if n >= 1_000_000_000_000.0 { format!("{:.2}T", n / 1_000_000_000_000.0) }
    else if n >= 1_000_000_000.0  { format!("{:.2}B", n / 1_000_000_000.0) }
    else if n >= 1_000_000.0      { format!("{:.2}M", n / 1_000_000.0) }
    else if n >= 1_000.0           { format!("{:.2}K", n / 1_000.0) }
    else { format!("{:.0}", n) }
}

fn bar(cost: f64, max_cost: f64, w: usize) -> String {
    let filled = if max_cost > 0.0 && cost > 0.0 {
        ((cost / max_cost) * w as f64).round() as usize
    } else { 0 };
    let filled = filled.min(w);
    format!("{}{}", "█".repeat(filled), "░".repeat(w - filled))
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let (label, range_flag) = resolve_range(&args);

    let exchange: f64 = Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()?
        .get(EXCH_API)
        .send()?
        .json::<ExchangeResp>()?
        .rates.cny;

    let mut tok_args = vec!["models", "--json"];
    if !range_flag.is_empty() { tok_args.push(range_flag); }
    let output = Command::new("tokscale").args(&tok_args).output()?;
    let data: ModelsResp = serde_json::from_str(&String::from_utf8_lossy(&output.stdout))?;

    let total_in = data.total_input.unwrap_or(0.0);
    let total_out = data.total_output.unwrap_or(0.0);
    let total_cache_read = data.total_cache_read.unwrap_or(0.0);
    let total_cache_write = data.total_cache_write.unwrap_or(0.0);
    let total_cost = data.total_cost.unwrap_or(0.0);
    let total_rmb = total_cost * exchange;
    let max_cost = data.entries.iter().map(|e| e.cost).fold(0.0_f64, f64::max);

    let mut entries = data.entries;
    entries.sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap());

    let metric_label = match range_flag {
        "--today" => "TODAY",
        "--month" => "MONTH",
        _ => "ALL",
    };

    // ── Summary table ───────────────────────────────────────────────
    let mut sum_builder = Builder::new();
    sum_builder.push_record(["Metric", "Input", "Output", "Cache Write", "Cache Read", "Total", "CNY"]);
    sum_builder.push_record([
        metric_label,
        &fmt_num(total_in),
        &fmt_num(total_out),
        &fmt_num(total_cache_write),
        &fmt_num(total_cache_read),
        &fmt_num(total_in + total_out + total_cache_write + total_cache_read),
        &format!("¥{:.2}", total_rmb),
    ]);
    let mut sum_table = sum_builder.build();
    sum_table
        .with(Style::rounded())
        .with(Padding::new(1, 1, 0, 0));

    // ── Detail table ─────────────────────────────────────────────
    let mut detail_builder = Builder::new();
    detail_builder.push_record(["Client", "Model", "Input", "Output", "Cache Write", "Cache Read", "Total", "CNY", "Share"]);
    for entry in &entries {
        if entry.input == 0.0 && entry.output == 0.0 && entry.cache_write == 0.0 && entry.cache_read == 0.0 {
            continue;
        }
        let bar_str = bar(entry.cost, max_cost, 20);
        let total = entry.input + entry.output + entry.cache_write + entry.cache_read;
        detail_builder.push_record([
            &entry.client,
            &entry.model,
            &fmt_num(entry.input),
            &fmt_num(entry.output),
            &fmt_num(entry.cache_write),
            &fmt_num(entry.cache_read),
            &fmt_num(total),
            &format!("¥{:.2}", entry.cost * exchange),
            &bar_str,
        ]);
    }
    let mut detail_table = detail_builder.build();
    detail_table
        .with(Style::rounded())
        .with(Padding::new(0, 1, 0, 0))
        .with(Modify::new(Segment::new(.., 1..2)).with(Width::wrap(25)));

    // ── TOP 3 table ────────────────────────────────────────────────
    let mut top_builder = Builder::new();
    top_builder.push_record(["#", "Model", "Total", "CNY", "Share"]);
    for (i, entry) in entries.iter().filter(|e| e.cost > 0.0).take(3).enumerate() {
        let bar_str = bar(entry.cost, max_cost, 10);
        let total = entry.input + entry.output + entry.cache_write + entry.cache_read;
        top_builder.push_record([
            &format!("{}", i + 1),
            &entry.model,
            &fmt_num(total),
            &format!("¥{:.2}", entry.cost * exchange),
            &bar_str,
        ]);
    }
    let mut top_table = top_builder.build();
    top_table.with(Style::rounded());

    // ── Print ─────────────────────────────────────────────────────
    println!();
    println!("  calctokens  --  Token Usage Report   [ {} ]", label);
    println!();
    println!("  SUMMARY");
    print!("{}", sum_table);
    println!();
    println!("  DETAIL");
    print!("{}", detail_table);
    println!();
    println!("  TOP 3 COST");
    println!("{}", top_table);

    Ok(())
}

use chrono::Utc;
use clap::Parser;
use reqwest::blocking::Client;
use rusqlite::{Connection, params};
use serde::Deserialize;
use std::process::Command;
use tabled::builder::Builder;
use tabled::settings::{object::Segment, Padding, Style, Modify, Width};

const EXCH_API: &str = "https://api.exchangerate-api.com/v4/latest/USD";
const DB_PATH: &str = ".calctokens.db";

#[derive(Parser, Debug)]
#[command(name = "calctokens", bin_name = "calctokens")]
struct Args {
    #[arg(long, conflicts_with_all = ["month", "all", "monthly", "hourly", "pricing", "clients"])]
    today: bool,
    #[arg(long, conflicts_with_all = ["today", "all", "monthly", "hourly", "pricing", "clients"])]
    month: bool,
    #[arg(long, conflicts_with_all = ["today", "month", "monthly", "hourly", "pricing", "clients"])]
    all: bool,
    #[arg(long, conflicts_with_all = ["today", "month", "all", "hourly", "pricing", "clients"])]
    monthly: bool,
    #[arg(long, conflicts_with_all = ["today", "month", "all", "monthly", "pricing", "clients"])]
    hourly: bool,
    #[arg(long, conflicts_with_all = ["today", "month", "all", "monthly", "hourly", "clients"])]
    pricing: bool,
    #[arg(long, conflicts_with_all = ["today", "month", "all", "monthly", "hourly", "pricing"])]
    clients: bool,
}

impl Default for Args {
    fn default() -> Self { Args { today: false, month: false, all: true, monthly: false, hourly: false, pricing: false, clients: false } }
}

fn resolve_mode(args: &Args) -> (&'static str, Vec<&'static str>) {
    if args.today       { ("Today", vec!["--today"]) }
    else if args.month  { ("This Month", vec!["--month"]) }
    else if args.monthly { ("Monthly", vec!["monthly"]) }
    else if args.hourly { ("Hourly", vec!["hourly"]) }
    else                { ("All Time", vec![]) }
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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct MonthlyResp {
    entries: Vec<MonthlyEntry>,
    total_cost: f64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct MonthlyEntry {
    month: String,
    models: Vec<String>,
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
    message_count: i64,
    cost: f64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HourlyResp {
    entries: Vec<HourlyEntry>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HourlyEntry {
    hour: String,
    clients: Vec<String>,
    models: Vec<String>,
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
    message_count: i64,
    cost: f64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct PricingResp {
    model_id: String,
    matched_key: String,
    source: String,
    pricing: PricingDetail,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct PricingDetail {
    input_cost_per_token: f64,
    output_cost_per_token: f64,
    cache_read_input_token_cost: Option<f64>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ClientsResp {
    clients: Vec<ClientInfo>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ClientInfo {
    client: String,
    label: String,
    sessions_path: String,
    sessions_path_exists: bool,
    message_count: i64,
    headless_supported: bool,
    #[serde(default)]
    headless_message_count: i64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct HistoryRecord {
    id: i64,
    timestamp: String,
    range: String,
    total_input: f64,
    total_output: f64,
    total_cache_read: f64,
    total_cache_write: f64,
    total_cost: f64,
    total_rmb: f64,
}

fn init_db(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            range TEXT NOT NULL,
            total_input REAL NOT NULL,
            total_output REAL NOT NULL,
            total_cache_read REAL NOT NULL,
            total_cache_write REAL NOT NULL,
            total_cost REAL NOT NULL,
            total_rmb REAL NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS exchange_cache (
            currency TEXT PRIMARY KEY,
            rate REAL NOT NULL,
            fetched_date TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS token_cache (
            range TEXT PRIMARY KEY,
            json_data TEXT NOT NULL,
            fetched_date TEXT NOT NULL
        )",
        [],
    )?;
    Ok(())
}

fn get_cached_exchange(conn: &Connection, currency: &str) -> rusqlite::Result<Option<f64>> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare("SELECT rate FROM exchange_cache WHERE currency = ? AND fetched_date = ?")?;
    let mut rows = stmt.query(params![currency, today])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

fn save_exchange_cache(conn: &Connection, currency: &str, rate: f64) -> rusqlite::Result<()> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    conn.execute(
        "INSERT OR REPLACE INTO exchange_cache (currency, rate, fetched_date) VALUES (?1, ?2, ?3)",
        params![currency, rate, today],
    )?;
    Ok(())
}

fn get_cached_token_data(conn: &Connection, range: &str) -> rusqlite::Result<Option<String>> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare("SELECT json_data FROM token_cache WHERE range = ? AND fetched_date = ?")?;
    let mut rows = stmt.query(params![range, today])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

fn save_token_cache(conn: &Connection, range: &str, json_data: &str) -> rusqlite::Result<()> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    conn.execute(
        "INSERT OR REPLACE INTO token_cache (range, json_data, fetched_date) VALUES (?1, ?2, ?3)",
        params![range, json_data, today],
    )?;
    Ok(())
}

fn get_last_record(conn: &Connection, range: &str) -> rusqlite::Result<Option<HistoryRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, range, total_input, total_output, total_cache_read,
                total_cache_write, total_cost, total_rmb
         FROM history WHERE range = ? ORDER BY id DESC LIMIT 1"
    )?;

    let mut rows = stmt.query(params![range])?;
    if let Some(row) = rows.next()? {
        Ok(Some(HistoryRecord {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            range: row.get(2)?,
            total_input: row.get(3)?,
            total_output: row.get(4)?,
            total_cache_read: row.get(5)?,
            total_cache_write: row.get(6)?,
            total_cost: row.get(7)?,
            total_rmb: row.get(8)?,
        }))
    } else {
        Ok(None)
    }
}

fn save_record(conn: &Connection, range: &str, total_in: f64, total_out: f64,
               total_cache_read: f64, total_cache_write: f64, total_cost: f64,
               total_rmb: f64) -> rusqlite::Result<()> {
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    conn.execute(
        "INSERT INTO history (timestamp, range, total_input, total_output, total_cache_read,
                              total_cache_write, total_cost, total_rmb)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![timestamp, range, total_in, total_out, total_cache_read,
                total_cache_write, total_cost, total_rmb],
    )?;
    Ok(())
}

fn fmt_num(n: f64) -> String {
    if n >= 1_000_000_000_000.0 { format!("{:.2}T", n / 1_000_000_000_000.0) }
    else if n >= 1_000_000_000.0  { format!("{:.2}B", n / 1_000_000_000.0) }
    else if n >= 1_000_000.0      { format!("{:.2}M", n / 1_000_000.0) }
    else if n >= 1_000.0           { format!("{:.2}K", n / 1_000.0) }
    else { format!("{:.0}", n) }
}

fn fmt_diff(n: f64) -> String {
    if n == 0.0 { String::from("0") }
    else if n.abs() >= 1_000_000_000_000.0 { format!("{:+.2}T", n / 1_000_000_000_000.0) }
    else if n.abs() >= 1_000_000_000.0  { format!("{:+.2}B", n / 1_000_000_000.0) }
    else if n.abs() >= 1_000_000.0      { format!("{:+.2}M", n / 1_000_000.0) }
    else if n.abs() >= 1_000.0           { format!("{:+.2}K", n / 1_000.0) }
    else { format!("{:+.0}", n) }
}

fn bar(cost: f64, max_cost: f64, w: usize) -> String {
    let filled = if max_cost > 0.0 && cost > 0.0 {
        ((cost / max_cost) * w as f64).round() as usize
    } else { 0 };
    let filled = filled.min(w);
    format!("{}{}", "█".repeat(filled), "░".repeat(w - filled))
}

fn print_models_view(data: &ModelsResp, exchange: f64, last_record: &Option<HistoryRecord>, range_flag: &str, range_key: &str) {
    let total_in = data.total_input.unwrap_or(0.0);
    let total_out = data.total_output.unwrap_or(0.0);
    let total_cache_read = data.total_cache_read.unwrap_or(0.0);
    let total_cache_write = data.total_cache_write.unwrap_or(0.0);
    let total_cost = data.total_cost.unwrap_or(0.0);
    let total_rmb = total_cost * exchange;
    let max_cost = data.entries.iter().map(|e| e.cost).fold(0.0_f64, f64::max);

    let metric_label = match range_flag {
        "--today" => "TODAY",
        "--month" => "MONTH",
        _ => "ALL",
    };

    let (delta_in, delta_out, delta_cache_read, delta_cache_write, delta_rmb) =
        if let Some(last) = last_record {
            (
                total_in - last.total_input,
                total_out - last.total_output,
                total_cache_read - last.total_cache_read,
                total_cache_write - last.total_cache_write,
                total_rmb - last.total_rmb,
            )
        } else {
            (0.0, 0.0, 0.0, 0.0, 0.0)
        };

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
    sum_table.with(Style::rounded()).with(Padding::new(1, 1, 0, 0));

    let delta_table = if last_record.is_some() {
        let mut delta_builder = Builder::new();
        delta_builder.push_record(["Δ Metric", "Δ Input", "Δ Output", "Δ Cache Write", "Δ Cache Read", "Δ Total", "Δ CNY"]);
        delta_builder.push_record([
            metric_label,
            &fmt_diff(delta_in),
            &fmt_diff(delta_out),
            &fmt_diff(delta_cache_write),
            &fmt_diff(delta_cache_read),
            &fmt_diff(delta_in + delta_out + delta_cache_write + delta_cache_read),
            &format!("¥{:+.2}", delta_rmb),
        ]);
        let mut dt = delta_builder.build();
        dt.with(Style::rounded()).with(Padding::new(1, 1, 0, 0));
        Some(dt)
    } else {
        None
    };

    let mut entries = data.entries.clone();
    entries.sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap());

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

    println!();
    println!("  calctokens  --  Token Usage Report   [ {} ]", metric_label);
    println!();
    println!("  SUMMARY");
    print!("{}", sum_table);
    println!();
    if let Some(dt) = delta_table {
        println!("  DELTA (since last check)");
        print!("{}", dt);
        println!();
    }
    println!("  DETAIL");
    print!("{}", detail_table);
    println!();
    println!("  TOP 3 COST");
    println!("{}", top_table);
}

fn print_monthly_view(data: &MonthlyResp, exchange: f64) {
    let max_cost = data.entries.iter().map(|e| e.cost).fold(0.0_f64, f64::max);
    let total_rmb = data.total_cost * exchange;

    let mut sum_builder = Builder::new();
    sum_builder.push_record(["Period", "Input", "Output", "Cache Write", "Cache Read", "Total", "CNY", "Msgs"]);
    for entry in &data.entries {
        let total = entry.input + entry.output + entry.cache_write + entry.cache_read;
        sum_builder.push_record([
            &entry.month,
            &fmt_num(entry.input),
            &fmt_num(entry.output),
            &fmt_num(entry.cache_write),
            &fmt_num(entry.cache_read),
            &fmt_num(total),
            &format!("¥{:.2}", entry.cost * exchange),
            &entry.message_count.to_string(),
        ]);
    }
    let mut sum_table = sum_builder.build();
    sum_table.with(Style::rounded()).with(Padding::new(1, 1, 0, 0));

    let mut detail_builder = Builder::new();
    detail_builder.push_record(["Month", "Total Tokens", "CNY", "Share"]);
    for entry in &data.entries {
        let total = entry.input + entry.output + entry.cache_write + entry.cache_read;
        let bar_str = bar(entry.cost, max_cost, 20);
        detail_builder.push_record([
            &entry.month,
            &fmt_num(total),
            &format!("¥{:.2}", entry.cost * exchange),
            &bar_str,
        ]);
    }
    let mut detail_table = detail_builder.build();
    detail_table.with(Style::rounded()).with(Padding::new(0, 1, 0, 0));

    println!();
    println!("  calctokens  --  Monthly Usage Report");
    println!();
    println!("  TOTAL COST: ¥{:.2}", total_rmb);
    println!();
    println!("  MONTHLY BREAKDOWN");
    print!("{}", sum_table);
    println!();
    println!("  TREND");
    print!("{}", detail_table);
}

fn print_hourly_view(data: &HourlyResp, exchange: f64) {
    let max_cost = data.entries.iter().map(|e| e.cost).fold(0.0_f64, f64::max);

    let mut detail_builder = Builder::new();
    detail_builder.push_record(["Hour", "Clients", "Models", "Input", "Output", "Cache", "Total", "CNY", "Share"]);
    for entry in &data.entries {
        let total = entry.input + entry.output + entry.cache_write + entry.cache_read;
        let bar_str = bar(entry.cost, max_cost, 15);
        let clients = entry.clients.iter().take(3).cloned().collect::<Vec<_>>().join(",");
        let models = entry.models.iter().take(2).cloned().collect::<Vec<_>>().join(",");
        detail_builder.push_record([
            &entry.hour,
            &clients,
            &models,
            &fmt_num(entry.input),
            &fmt_num(entry.output),
            &fmt_num(entry.cache_write + entry.cache_read),
            &fmt_num(total),
            &format!("¥{:.2}", entry.cost * exchange),
            &bar_str,
        ]);
    }
    let mut detail_table = detail_builder.build();
    detail_table
        .with(Style::rounded())
        .with(Padding::new(0, 1, 0, 0))
        .with(Modify::new(Segment::new(.., 1..3)).with(Width::wrap(20)));

    println!();
    println!("  calctokens  --  Hourly Usage Report");
    println!();
    println!("  HOURLY BREAKDOWN");
    print!("{}", detail_table);
}

fn print_pricing_view(data: &PricingResp, exchange: f64) {
    let input_rmb = data.pricing.input_cost_per_token * 1_000_000.0 * exchange;
    let output_rmb = data.pricing.output_cost_per_token * 1_000_000.0 * exchange;
    let cache_rmb = data.pricing.cache_read_input_token_cost.unwrap_or(0.0) * 1_000_000.0 * exchange;

    let mut builder = Builder::new();
    builder.push_record(["Model", "Source", "Input/M", "Output/M", "Cache Read/M"]);
    builder.push_record([
        &data.model_id,
        &data.source,
        &format!("¥{:.4}", input_rmb),
        &format!("¥{:.4}", output_rmb),
        &format!("¥{:.4}", cache_rmb),
    ]);
    let mut table = builder.build();
    table.with(Style::rounded()).with(Padding::new(1, 1, 0, 0));

    println!();
    println!("  calctokens  --  Pricing Report");
    println!();
    println!("  MODEL: {}", data.model_id);
    println!("  MATCHED: {}", data.matched_key);
    println!();
    print!("{}", table);
    println!();
    println!("  Per Million Tokens (USD → CNY @ {:.2})", exchange);
}

fn print_clients_view(data: &ClientsResp, exchange: f64) {
    let total_msgs: i64 = data.clients.iter().map(|c| c.message_count).sum();

    let mut sum_builder = Builder::new();
    sum_builder.push_record(["Client", "Label", "Sessions", "Msgs"]);
    for client in &data.clients {
        if client.message_count == 0 {
            continue;
        }
        let sess = if client.sessions_path_exists { "✓" } else { "✗" };
        sum_builder.push_record([
            &client.client,
            &client.label,
            &sess.to_string(),
            &client.message_count.to_string(),
        ]);
    }
    let mut sum_table = sum_builder.build();
    sum_table.with(Style::rounded()).with(Padding::new(1, 1, 0, 0));

    let mut detail_builder = Builder::new();
    detail_builder.push_record(["Client", "Label", "Sessions", "Headless", "Msgs"]);
    for client in &data.clients {
        let headless = if client.headless_supported { "✓" } else { "-" };
        let sess = if client.sessions_path_exists { "✓" } else { "✗" };
        detail_builder.push_record([
            &client.client,
            &client.label,
            &sess.to_string(),
            &headless.to_string(),
            &client.message_count.to_string(),
        ]);
    }
    let mut detail_table = detail_builder.build();
    detail_table.with(Style::rounded()).with(Padding::new(0, 1, 0, 0));

    println!();
    println!("  calctokens  --  Clients Report");
    println!();
    println!("  TOTAL MESSAGES: {}", total_msgs);
    println!();
    println!("  ACTIVE CLIENTS");
    print!("{}", sum_table);
    println!();
    println!("  ALL CLIENTS");
    print!("{}", detail_table);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let (label, mode_args) = resolve_mode(&args);
    let range_key = match args.today {
        true => "today",
        false if args.month => "month",
        false if args.monthly => "monthly",
        false if args.hourly => "hourly",
        _ => "all",
    };
    let range_flag = mode_args.get(0).map(|s| *s).unwrap_or("");

    let conn = Connection::open(DB_PATH)?;
    init_db(&conn)?;

    let exchange: f64 = if let Some(cached) = get_cached_exchange(&conn, "CNY")? {
        cached
    } else {
        let rate: f64 = Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()?
            .get(EXCH_API)
            .send()?
            .json::<ExchangeResp>()?
            .rates.cny;
        save_exchange_cache(&conn, "CNY", rate)?;
        rate
    };

    if args.pricing {
        let output = Command::new("tokscale").args(["pricing", "--json", "minimax-m2.7-highspeed"]).output()?;
        let data: PricingResp = serde_json::from_str(&String::from_utf8_lossy(&output.stdout))?;
        print_pricing_view(&data, exchange);
    } else if args.clients {
        let output = Command::new("tokscale").args(["clients", "--json"]).output()?;
        let data: ClientsResp = serde_json::from_str(&String::from_utf8_lossy(&output.stdout))?;
        print_clients_view(&data, exchange);
    } else if args.monthly {
        let json_data = if let Some(cached) = get_cached_token_data(&conn, "monthly")? {
            cached
        } else {
            let output = Command::new("tokscale").args(["monthly", "--json"]).output()?;
            let json_str = String::from_utf8_lossy(&output.stdout).to_string();
            save_token_cache(&conn, "monthly", &json_str)?;
            json_str
        };
        let data: MonthlyResp = serde_json::from_str(&json_data)?;
        print_monthly_view(&data, exchange);
    } else if args.hourly {
        let json_data = if let Some(cached) = get_cached_token_data(&conn, "hourly")? {
            cached
        } else {
            let output = Command::new("tokscale").args(["hourly", "--json"]).output()?;
            let json_str = String::from_utf8_lossy(&output.stdout).to_string();
            save_token_cache(&conn, "hourly", &json_str)?;
            json_str
        };
        let data: HourlyResp = serde_json::from_str(&json_data)?;
        print_hourly_view(&data, exchange);
    } else {
        let cache_key = if mode_args.is_empty() { "all" } else { range_key };
        let last_record = get_last_record(&conn, range_key)?;

        let json_data = if let Some(cached) = get_cached_token_data(&conn, cache_key)? {
            cached
        } else {
            let mut tok_args = vec!["models", "--json"];
            for arg in &mode_args {
                if !arg.is_empty() {
                    tok_args.push(arg);
                }
            }
            let output = Command::new("tokscale").args(&tok_args).output()?;
            let json_str = String::from_utf8_lossy(&output.stdout).to_string();
            save_token_cache(&conn, cache_key, &json_str)?;
            json_str
        };
        let data: ModelsResp = serde_json::from_str(&json_data)?;

        let total_in = data.total_input.unwrap_or(0.0);
        let total_out = data.total_output.unwrap_or(0.0);
        let total_cache_read = data.total_cache_read.unwrap_or(0.0);
        let total_cache_write = data.total_cache_write.unwrap_or(0.0);
        let total_cost = data.total_cost.unwrap_or(0.0);
        let total_rmb = total_cost * exchange;

        save_record(&conn, range_key, total_in, total_out, total_cache_read, total_cache_write, total_cost, total_rmb)?;

        print_models_view(&data, exchange, &last_record, range_flag, range_key);
    }

    Ok(())
}

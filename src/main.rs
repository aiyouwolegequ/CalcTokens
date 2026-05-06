use chrono::Utc;
use clap::Parser;
use reqwest::blocking::Client;
use rusqlite::{Connection, params};
use serde::Deserialize;
use tabled::builder::Builder;
use tabled::settings::{object::Segment, Padding, Style, Modify, Width};
use tokio::runtime::Runtime;
use tokscale_core::{
    ClientId, LocalParseOptions,
    ModelReport, MonthlyReport, HourlyReport,
    ModelUsage, MonthlyUsage, HourlyUsage,
    pricing,
};

const EXCH_API: &str = "https://api.exchangerate-api.com/v4/latest/USD";
fn db_path() -> String {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| ".".into());
    format!("{}/.calctokens.db", home)
}

#[derive(Parser, Debug)]
#[command(name = "calctokens", bin_name = "calctokens", about = "AI coding assistant token usage tracker (CNY)")]
struct Args {
    /// Filter by client(s): claude, opencode, codex, gemini, openclaw, hermes, kimi, qwen, antigravity, etc.
    #[arg(long, short, num_args = 1..)]
    client: Vec<String>,
    #[arg(long, conflicts_with_all = ["month", "monthly", "hourly", "pricing", "clients"])]
    today: bool,
    #[arg(long, conflicts_with_all = ["today", "monthly", "hourly", "pricing", "clients"])]
    month: bool,
    #[arg(long, conflicts_with_all = ["today", "month", "hourly", "pricing", "clients"])]
    monthly: bool,
    #[arg(long, conflicts_with_all = ["today", "month", "monthly", "pricing", "clients"])]
    hourly: bool,
    /// Look up pricing for a model (e.g. claude-sonnet-4-20250514)
    #[arg(long, value_name = "MODEL_ID")]
    pricing: Option<String>,
    #[arg(long, conflicts_with_all = ["today", "month", "monthly", "hourly"])]
    clients: bool,
    /// Filter: start date (YYYY-MM-DD)
    #[arg(long)]
    since: Option<String>,
    /// Filter: end date (YYYY-MM-DD)
    #[arg(long)]
    until: Option<String>,
    /// Filter: year (e.g. 2026)
    #[arg(long)]
    year: Option<String>,
    /// Output as JSON (works with all report types)
    #[arg(long)]
    json_output: bool,
}

#[derive(Deserialize, Debug)]
struct ExchangeResp { rates: Rates }
#[derive(Deserialize, Debug)]
struct Rates { #[serde(rename = "CNY")] cny: f64 }

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct HistoryRecord {
    id: i64, timestamp: String, range: String,
    total_input: i64, total_output: i64, total_cache_read: i64,
    total_cache_write: i64, total_cost: f64, total_rmb: f64,
}

// ── DB helpers ──────────────────────────────────────────────────────────

fn init_db(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL, range TEXT NOT NULL,
            total_input REAL NOT NULL, total_output REAL NOT NULL,
            total_cache_read REAL NOT NULL, total_cache_write REAL NOT NULL,
            total_cost REAL NOT NULL, total_rmb REAL NOT NULL
        )", [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS exchange_cache (
            currency TEXT PRIMARY KEY, rate REAL NOT NULL, fetched_date TEXT NOT NULL
        )", [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS token_cache (
            range TEXT PRIMARY KEY, json_data TEXT NOT NULL, fetched_date TEXT NOT NULL
        )", [],
    )?;
    // messages is the authoritative raw data store — once synced, reports
    // read from here regardless of whether source log files still exist.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            client TEXT NOT NULL,
            model_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            workspace_key TEXT,
            workspace_label TEXT,
            timestamp INTEGER NOT NULL,
            date TEXT NOT NULL,
            input_tokens INTEGER NOT NULL,
            output_tokens INTEGER NOT NULL,
            cache_read INTEGER NOT NULL DEFAULT 0,
            cache_write INTEGER NOT NULL DEFAULT 0,
            reasoning INTEGER NOT NULL DEFAULT 0,
            cost REAL NOT NULL,
            message_count INTEGER NOT NULL DEFAULT 1,
            agent TEXT,
            is_turn_start INTEGER NOT NULL DEFAULT 0,
            message_key TEXT NOT NULL UNIQUE
        )",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_messages_date ON messages(date)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_messages_client ON messages(client)", [])?;
    // daily_summary pre-aggregates messages by date+client+model so
    // model/monthly reports avoid scanning 100K+ raw rows.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS daily_summary (
            date TEXT NOT NULL,
            client TEXT NOT NULL,
            model_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            input_tokens INTEGER NOT NULL,
            output_tokens INTEGER NOT NULL,
            cache_read INTEGER NOT NULL,
            cache_write INTEGER NOT NULL,
            reasoning INTEGER NOT NULL DEFAULT 0,
            cost REAL NOT NULL,
            message_count INTEGER NOT NULL,
            turn_count INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (date, client, model_id)
        )",
        [],
    )?;
    Ok(())
}

fn get_cached_exchange(conn: &Connection, currency: &str) -> rusqlite::Result<Option<f64>> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare("SELECT rate FROM exchange_cache WHERE currency = ? AND fetched_date = ?")?;
    let mut rows = stmt.query(params![currency, today])?;
    if let Some(row) = rows.next()? { Ok(Some(row.get(0)?)) } else { Ok(None) }
}

fn save_exchange_cache(conn: &Connection, currency: &str, rate: f64) -> rusqlite::Result<()> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    conn.execute("INSERT OR REPLACE INTO exchange_cache (currency, rate, fetched_date) VALUES (?1, ?2, ?3)", params![currency, rate, today])?;
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
            id: row.get(0)?, timestamp: row.get(1)?, range: row.get(2)?,
            total_input: row.get::<_, f64>(3)? as i64,
            total_output: row.get::<_, f64>(4)? as i64,
            total_cache_read: row.get::<_, f64>(5)? as i64,
            total_cache_write: row.get::<_, f64>(6)? as i64,
            total_cost: row.get(7)?, total_rmb: row.get(8)?,
        }))
    } else { Ok(None) }
}

fn save_record(conn: &Connection, range: &str, total_in: i64, total_out: i64,
               total_cache_read: i64, total_cache_write: i64, total_cost: f64,
               total_rmb: f64) -> rusqlite::Result<()> {
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    conn.execute(
        "INSERT INTO history (timestamp, range, total_input, total_output, total_cache_read,
                              total_cache_write, total_cost, total_rmb)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![timestamp, range, total_in as f64, total_out as f64, total_cache_read as f64, total_cache_write as f64, total_cost, total_rmb],
    )?;
    Ok(())
}

// ── Message syncing ─────────────────────────────────────────────────────
// Parse all client log files, store every message in SQLite.
// Dedup by message_key so repeated runs only add new messages.

fn sync_messages(conn: &Connection, rt: &Runtime) -> Result<(), Box<dyn std::error::Error>> {
    let opts = LocalParseOptions {
        home_dir: None,
        use_env_roots: true,
        clients: None,
        since: None,
        until: None,
        year: None,
        scanner_settings: Default::default(),
    };

    let messages = rt.block_on(tokscale_core::parse_local_unified_messages(opts))?;

    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO messages
             (client, model_id, provider_id, session_id,
              workspace_key, workspace_label,
              timestamp, date,
              input_tokens, output_tokens, cache_read, cache_write, reasoning,
              cost, message_count, agent, is_turn_start, message_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                     ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18)"
        )?;
        for msg in &messages {
            // Messages without a natural dedup_key get a synthetic one from
            // their content fields so re-scans never duplicate a row.
            let key = msg.dedup_key.clone().unwrap_or_else(|| {
                format!("{}:{}:{}:{}:{}:{}",
                    msg.client, msg.session_id, msg.timestamp,
                    msg.model_id, msg.tokens.input, msg.tokens.output)
            });
            stmt.execute(params![
                msg.client, msg.model_id, msg.provider_id, msg.session_id,
                msg.workspace_key, msg.workspace_label,
                msg.timestamp, msg.date,
                msg.tokens.input, msg.tokens.output, msg.tokens.cache_read, msg.tokens.cache_write, msg.tokens.reasoning,
                msg.cost, msg.message_count, msg.agent, msg.is_turn_start as i32,
                key,
            ])?;
        }
    }
    tx.commit()?;

    Ok(())
}

/// Rebuild daily_summary from raw messages.
/// Idempotent and fast (~50ms for 100K rows) — called after every sync.
fn refresh_daily_summary(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM daily_summary", [])?;
    conn.execute(
        "INSERT INTO daily_summary
         (date, client, model_id, provider_id,
          input_tokens, output_tokens, cache_read, cache_write, reasoning,
          cost, message_count, turn_count)
         SELECT date, client, model_id, provider_id,
                SUM(input_tokens), SUM(output_tokens),
                SUM(cache_read), SUM(cache_write), SUM(reasoning),
                SUM(cost), SUM(message_count), SUM(is_turn_start)
         FROM messages
         GROUP BY date, client, model_id",
        [],
    )?;
    Ok(())
}

// ── SQLite-based report queries ─────────────────────────────────────────

fn build_date_filter(args: &Args) -> (Option<String>, Option<String>) {
    if args.today {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        (Some(today.clone()), Some(today))
    } else if args.month {
        let now = Utc::now();
        let start = now.format("%Y-%m-01").to_string();
        let end = now.format("%Y-%m-%d").to_string();
        (Some(start), Some(end))
    } else {
        (args.since.clone(), args.until.clone())
    }
}

/// Build SQL WHERE clause and collect parameters for date / year / client filters.
/// Returns (where_clause_sql, param_values).
fn build_where_clause(args: &Args) -> (String, Vec<String>) {
    let mut clauses: Vec<String> = Vec::new();
    let mut params: Vec<String> = Vec::new();

    if let Some(ref year) = args.year {
        clauses.push(format!("date LIKE ?{}", params.len() + 1));
        params.push(format!("{}%", year));
    }

    let (since, until) = build_date_filter(args);
    if let Some(ref s) = since {
        clauses.push(format!("date >= ?{}", params.len() + 1));
        params.push(s.clone());
    }
    if let Some(ref u) = until {
        clauses.push(format!("date <= ?{}", params.len() + 1));
        params.push(u.clone());
    }

    if !args.client.is_empty() {
        let placeholders: Vec<String> = (0..args.client.len())
            .map(|i| format!("?{}", params.len() + i + 1)).collect();
        clauses.push(format!("client IN ({})", placeholders.join(",")));
        params.extend(args.client.clone());
    }

    let where_clause = if clauses.is_empty() {
        String::from("1=1")
    } else {
        clauses.join(" AND ")
    };

    (where_clause, params)
}

fn query_model_report(conn: &Connection, args: &Args) -> Result<ModelReport, Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    let (where_clause, params) = build_where_clause(args);

    let sql = format!(
        "SELECT client, model_id, MAX(provider_id),
                SUM(input_tokens), SUM(output_tokens),
                SUM(cache_read), SUM(cache_write),
                SUM(reasoning), SUM(message_count),
                SUM(cost)
         FROM daily_summary
         WHERE {}
         GROUP BY client, model_id
         ORDER BY SUM(cost) DESC", where_clause
    );

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p as &dyn rusqlite::types::ToSql).collect();
    let mut rows = stmt.query(param_refs.as_slice())?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next()? {
        entries.push(ModelUsage {
            client: row.get(0)?,
            model: row.get(1)?,
            provider: row.get(2)?,
            merged_clients: None,
            workspace_key: None,
            workspace_label: None,
            input: row.get(3)?,
            output: row.get(4)?,
            cache_read: row.get(5)?,
            cache_write: row.get(6)?,
            reasoning: row.get(7)?,
            message_count: row.get(8)?,
            cost: row.get(9)?,
        });
    }

    let total_input: i64 = entries.iter().map(|e| e.input).sum();
    let total_output: i64 = entries.iter().map(|e| e.output).sum();
    let total_cache_read: i64 = entries.iter().map(|e| e.cache_read).sum();
    let total_cache_write: i64 = entries.iter().map(|e| e.cache_write).sum();
    let total_messages: i32 = entries.iter().map(|e| e.message_count).sum();
    let total_cost: f64 = entries.iter().map(|e| e.cost).sum();

    Ok(ModelReport {
        entries,
        total_input,
        total_output,
        total_cache_read,
        total_cache_write,
        total_messages,
        total_cost,
        processing_time_ms: start.elapsed().as_millis() as u32,
    })
}

fn query_monthly_report(conn: &Connection, args: &Args) -> Result<MonthlyReport, Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    let (where_clause, params) = build_where_clause(args);

    let sql = format!(
        "SELECT substr(date, 1, 7) as month,
                GROUP_CONCAT(DISTINCT model_id),
                SUM(input_tokens), SUM(output_tokens),
                SUM(cache_read), SUM(cache_write),
                SUM(message_count), SUM(cost)
         FROM daily_summary
         WHERE {}
         GROUP BY month
         ORDER BY month", where_clause
    );

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p as &dyn rusqlite::types::ToSql).collect();
    let mut rows = stmt.query(param_refs.as_slice())?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next()? {
        let models_str: Option<String> = row.get(1)?;
        entries.push(MonthlyUsage {
            month: row.get(0)?,
            models: models_str.map(|s| s.split(',').map(String::from).collect()).unwrap_or_default(),
            input: row.get(2)?,
            output: row.get(3)?,
            cache_read: row.get(4)?,
            cache_write: row.get(5)?,
            message_count: row.get(6)?,
            cost: row.get(7)?,
        });
    }

    let total_cost: f64 = entries.iter().map(|e| e.cost).sum();
    Ok(MonthlyReport {
        entries,
        total_cost,
        processing_time_ms: start.elapsed().as_millis() as u32,
    })
}

fn query_hourly_report(conn: &Connection, args: &Args) -> Result<HourlyReport, Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    let (where_clause, params) = build_where_clause(args);

    // Build SQL without format! %% escaping — use concat to avoid confusion.
    let sql = [
        "SELECT strftime('%Y-%m-%d %H:00', datetime(timestamp/1000, 'unixepoch')) as hour,",
        "       GROUP_CONCAT(DISTINCT client),",
        "       GROUP_CONCAT(DISTINCT model_id),",
        "       SUM(input_tokens), SUM(output_tokens),",
        "       SUM(cache_read), SUM(cache_write),",
        "       SUM(reasoning), SUM(message_count),",
        "       SUM(is_turn_start), SUM(cost)",
        " FROM messages",
        " WHERE ", &where_clause,
        " GROUP BY hour",
        " ORDER BY hour",
    ].join("\n");
    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p as &dyn rusqlite::types::ToSql).collect();
    let mut rows = stmt.query(param_refs.as_slice())?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next()? {
        let clients_str: Option<String> = row.get(1)?;
        let models_str: Option<String> = row.get(2)?;
        entries.push(HourlyUsage {
            hour: row.get(0)?,
            clients: clients_str.map(|s| s.split(',').map(String::from).collect()).unwrap_or_default(),
            models: models_str.map(|s| s.split(',').map(String::from).collect()).unwrap_or_default(),
            input: row.get(3)?,
            output: row.get(4)?,
            cache_read: row.get(5)?,
            cache_write: row.get(6)?,
            reasoning: row.get(7)?,
            message_count: row.get(8)?,
            turn_count: row.get::<_, i64>(9)? as i32,
            cost: row.get(10)?,
        });
    }

    let total_cost: f64 = entries.iter().map(|e| e.cost).sum();
    Ok(HourlyReport {
        entries,
        total_cost,
        processing_time_ms: start.elapsed().as_millis() as u32,
    })
}

// ── tokscale-core helpers ───────────────────────────────────────────────

fn fetch_pricing_lookup(rt: &Runtime, model_id: &str) -> Result<Option<pricing::lookup::LookupResult>, Box<dyn std::error::Error>> {
    let svc = rt.block_on(pricing::PricingService::get_or_init())
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    Ok(svc.lookup_with_source(model_id, None))
}

// ── Formatting helpers ──────────────────────────────────────────────────

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

fn bar(cost: f64, total_cost: f64, w: usize) -> String {
    // share = cost / total_cost × bar_width (not max_cost), so the bar
    // accurately represents each model's share of total spending.
    let filled = if total_cost > 0.0 && cost > 0.0 {
        ((cost / total_cost) * w as f64).round() as usize
    } else { 0 };
    let filled = filled.min(w);
    format!("{}{}", "█".repeat(filled), "░".repeat(w - filled))
}

// ── View printers ───────────────────────────────────────────────────────

fn print_models_view(report: &ModelReport, exchange: f64, last_record: &Option<HistoryRecord>, range_flag: &str) {
    let total_in = report.total_input;
    let total_out = report.total_output;
    let total_cache_read = report.total_cache_read;
    let total_cache_write = report.total_cache_write;
    let total_cost = report.total_cost;
    let total_rmb = total_cost * exchange;

    let metric_label = match range_flag {
        "today" => "TODAY",
        "month" => "MONTH",
        _ => "ALL",
    };

    let (delta_in, delta_out, delta_cache_read, delta_cache_write, delta_rmb) =
        if let Some(last) = last_record {
            (total_in - last.total_input, total_out - last.total_output,
             total_cache_read - last.total_cache_read, total_cache_write - last.total_cache_write,
             total_rmb - last.total_rmb)
        } else { (0, 0, 0, 0, 0.0) };

    let mut sum_builder = Builder::new();
    sum_builder.push_record(["Metric", "Input", "Output", "Cache Write", "Cache Read", "Total", "CNY"]);
    sum_builder.push_record([
        metric_label, &fmt_num(total_in as f64), &fmt_num(total_out as f64), &fmt_num(total_cache_write as f64),
        &fmt_num(total_cache_read as f64), &fmt_num((total_in + total_out + total_cache_write + total_cache_read) as f64),
        &format!("¥{:.2}", total_rmb),
    ]);
    let mut sum_table = sum_builder.build();
    sum_table.with(Style::rounded()).with(Padding::new(1, 1, 0, 0));

    let delta_table = if last_record.is_some() {
        let mut delta_builder = Builder::new();
        delta_builder.push_record(["Δ Metric", "Δ Input", "Δ Output", "Δ Cache Write", "Δ Cache Read", "Δ Total", "Δ CNY"]);
        delta_builder.push_record([
            metric_label, &fmt_diff(delta_in as f64), &fmt_diff(delta_out as f64), &fmt_diff(delta_cache_write as f64),
            &fmt_diff(delta_cache_read as f64), &fmt_diff((delta_in + delta_out + delta_cache_write + delta_cache_read) as f64),
            &format!("¥{:+.2}", delta_rmb),
        ]);
        let mut dt = delta_builder.build();
        dt.with(Style::rounded()).with(Padding::new(1, 1, 0, 0));
        Some(dt)
    } else { None };

    let mut entries: Vec<_> = report.entries.iter().collect();
    entries.sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap());

    let mut detail_builder = Builder::new();
    detail_builder.push_record(["Client", "Model", "Input", "Output", "Cache Write", "Cache Read", "Total", "CNY", "Share"]);
    for entry in &entries {
        let (inp, out, cw, cr) = (entry.input as f64, entry.output as f64, entry.cache_write as f64, entry.cache_read as f64);
        if inp == 0.0 && out == 0.0 && cw == 0.0 && cr == 0.0 { continue; }
        let bar_str = bar(entry.cost, total_cost, 20);
        let total = inp + out + cw + cr;
        detail_builder.push_record([
            &entry.client, &entry.model, &fmt_num(inp), &fmt_num(out), &fmt_num(cw),
            &fmt_num(cr), &fmt_num(total), &format!("¥{:.2}", entry.cost * exchange), &bar_str,
        ]);
    }
    let mut detail_table = detail_builder.build();
    detail_table.with(Style::rounded()).with(Padding::new(0, 1, 0, 0))
        .with(Modify::new(Segment::new(.., 1..2)).with(Width::wrap(25)));

    let mut top_builder = Builder::new();
    top_builder.push_record(["#", "Model", "Total", "CNY", "Share"]);
    for (i, entry) in entries.iter().filter(|e| e.cost > 0.0).take(3).enumerate() {
        let total = (entry.input + entry.output + entry.cache_write + entry.cache_read) as f64;
        top_builder.push_record([
            &format!("{}", i + 1), &entry.model, &fmt_num(total),
            &format!("¥{:.2}", entry.cost * exchange), &bar(entry.cost, total_cost, 10),
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

fn print_monthly_view(report: &MonthlyReport, exchange: f64) {
    let total_cost = report.total_cost;
    let total_rmb = total_cost * exchange;

    let mut sum_builder = Builder::new();
    sum_builder.push_record(["Period", "Input", "Output", "Cache Write", "Cache Read", "Total", "CNY", "Msgs"]);
    for entry in &report.entries {
        let (inp, out, cw, cr) = (entry.input as f64, entry.output as f64, entry.cache_write as f64, entry.cache_read as f64);
        sum_builder.push_record([
            &entry.month, &fmt_num(inp), &fmt_num(out), &fmt_num(cw), &fmt_num(cr),
            &fmt_num(inp + out + cw + cr), &format!("¥{:.2}", entry.cost * exchange),
            &entry.message_count.to_string(),
        ]);
    }
    let mut sum_table = sum_builder.build();
    sum_table.with(Style::rounded()).with(Padding::new(1, 1, 0, 0));

    let mut detail_builder = Builder::new();
    detail_builder.push_record(["Month", "Total Tokens", "CNY", "Share"]);
    for entry in &report.entries {
        let total = (entry.input + entry.output + entry.cache_write + entry.cache_read) as f64;
        detail_builder.push_record([
            &entry.month, &fmt_num(total), &format!("¥{:.2}", entry.cost * exchange), &bar(entry.cost, total_cost, 20),
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

fn print_hourly_view(report: &HourlyReport, exchange: f64) {
    let total_cost = report.total_cost;

    let mut detail_builder = Builder::new();
    detail_builder.push_record(["Hour", "Clients", "Models", "Input", "Output", "Cache", "Total", "CNY", "Share"]);
    for entry in &report.entries {
        let (inp, out, cw, cr) = (entry.input as f64, entry.output as f64, entry.cache_write as f64, entry.cache_read as f64);
        let total = inp + out + cw + cr;
        let clients = entry.clients.iter().take(3).cloned().collect::<Vec<_>>().join(",");
        let models = entry.models.iter().take(2).cloned().collect::<Vec<_>>().join(",");
        detail_builder.push_record([
            &entry.hour, &clients, &models, &fmt_num(inp), &fmt_num(out),
            &fmt_num(cw + cr), &fmt_num(total), &format!("¥{:.2}", entry.cost * exchange),
            &bar(entry.cost, total_cost, 15),
        ]);
    }
    let mut detail_table = detail_builder.build();
    detail_table.with(Style::rounded()).with(Padding::new(0, 1, 0, 0))
        .with(Modify::new(Segment::new(.., 1..3)).with(Width::wrap(20)));

    println!();
    println!("  calctokens  --  Hourly Usage Report");
    println!();
    println!("  HOURLY BREAKDOWN");
    print!("{}", detail_table);
}

fn print_pricing_view(model_id: &str, result: &pricing::lookup::LookupResult, exchange: f64) {
    let p = &result.pricing;
    let input_rmb = p.input_cost_per_token.unwrap_or(0.0) * 1_000_000.0 * exchange;
    let output_rmb = p.output_cost_per_token.unwrap_or(0.0) * 1_000_000.0 * exchange;
    let cache_rmb = p.cache_read_input_token_cost.unwrap_or(0.0) * 1_000_000.0 * exchange;

    let mut builder = Builder::new();
    builder.push_record(["Model", "Source", "Input/M", "Output/M", "Cache Read/M"]);
    builder.push_record([
        model_id, &result.source,
        &format!("¥{:.4}", input_rmb), &format!("¥{:.4}", output_rmb), &format!("¥{:.4}", cache_rmb),
    ]);
    let mut table = builder.build();
    table.with(Style::rounded()).with(Padding::new(1, 1, 0, 0));

    println!();
    println!("  calctokens  --  Pricing Report");
    println!();
    println!("  MODEL: {}", model_id);
    println!("  MATCHED: {}", result.matched_key);
    println!();
    print!("{}", table);
    println!();
    println!("  Per Million Tokens (USD → CNY @ {:.2})", exchange);
}

fn print_clients_view() {
    let home_dir = tokscale_core::get_home_dir_string(&None).unwrap_or_default();
    let mut builder = Builder::new();
    builder.push_record(["Client", "Path", "Exists"]);
    for client_id in ClientId::ALL.iter() {
        let def = client_id.data();
        let path = def.resolve_path(&home_dir);
        let exists = std::path::Path::new(&path).exists();
        builder.push_record([def.id, &path, if exists { "✓" } else { "✗" }]);
    }
    let mut table = builder.build();
    table.with(Style::rounded()).with(Padding::new(1, 1, 0, 0));

    println!();
    println!("  calctokens  --  Clients Report");
    println!();
    print!("{}", table);
}

// ── main ────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let range_key = if args.today { "today" }
        else if args.month { "month" }
        else if args.monthly { "monthly" }
        else if args.hourly { "hourly" }
        else { "all" };

    let conn = Connection::open(db_path())?;
    init_db(&conn)?;

    let exchange: f64 = if let Some(cached) = get_cached_exchange(&conn, "CNY")? {
        cached
    } else {
        let rate: f64 = Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()?
            .get(EXCH_API).send()?.json::<ExchangeResp>()?.rates.cny;
        save_exchange_cache(&conn, "CNY", rate)?;
        rate
    };

    if get_cached_exchange(&conn, "PRICING")?.is_some() {
        std::env::set_var("TOKSCALE_PRICING_CACHE_ONLY", "1");
    } else {
        std::env::set_var("TOKSCALE_PRICING_CACHE_ONLY", "0");
        save_exchange_cache(&conn, "PRICING", 1.0)?;
    }

    let rt = Runtime::new()?;

    // ── pricing lookup (independent of other modes) ─────────────────
    if let Some(ref model_id) = args.pricing {
        match fetch_pricing_lookup(&rt, model_id)? {
            Some(result) => {
                if args.json_output {
                    let p = &result.pricing;
                    let j = serde_json::json!({
                        "currency": "CNY",
                        "modelId": model_id,
                        "matchedKey": result.matched_key,
                        "source": result.source,
                        "inputCostPerMillion": p.input_cost_per_token.unwrap_or(0.0) * 1e6 * exchange,
                        "outputCostPerMillion": p.output_cost_per_token.unwrap_or(0.0) * 1e6 * exchange,
                        "cacheReadCostPerMillion": p.cache_read_input_token_cost.unwrap_or(0.0) * 1e6 * exchange,
                    });
                    println!("{}", serde_json::to_string_pretty(&j)?);
                } else {
                    print_pricing_view(model_id, &result, exchange);
                }
            }
            None => eprintln!("No pricing found for model: {}", model_id),
        }
        return Ok(());
    }

    // ── clients view ────────────────────────────────────────────────
    if args.clients {
        if args.json_output {
            let home_dir = tokscale_core::get_home_dir_string(&None).unwrap_or_default();
            let mut entries = Vec::new();
            for cid in ClientId::ALL.iter() {
                let def = cid.data();
                let path = def.resolve_path(&home_dir);
                let exists = std::path::Path::new(&path).exists();
                entries.push(serde_json::json!({ "client": def.id, "path": path, "exists": exists }));
            }
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "clients": entries }))?)
        } else {
            print_clients_view();
        }
        return Ok(());
    }

    // ── Sync messages from log files into SQLite ────────────────────
    // This is authoritative: once synced, reports read from SQLite
    // regardless of whether the original log files still exist.
    if let Err(e) = sync_messages(&conn, &rt) {
        // Sync failure is non-fatal — SQLite data from previous runs
        // can still generate reports. Only first-run + no-logs fails.
        eprintln!("Warning: message sync failed (data may be stale): {}", e);
    }
    // Rebuild pre-aggregated daily_summary so report queries hit ~5K rows
    // instead of scanning 100K+ raw messages.
    if let Err(e) = refresh_daily_summary(&conn) {
        eprintln!("Warning: daily_summary refresh failed: {}", e);
    }

    // ── monthly view ───────────────────────────────────────────────
    if args.monthly {
        let report = query_monthly_report(&conn, &args)?;
        if args.json_output {
            #[derive(serde::Serialize)] #[serde(rename_all = "camelCase")]
            struct E { month: String, models: Vec<String>, input: i64, output: i64, cache_read: i64, cache_write: i64, message_count: i32, cost: f64 }
            #[derive(serde::Serialize)] #[serde(rename_all = "camelCase")]
            struct R { currency: String, entries: Vec<E>, total_cost: f64 }
            let j = R { currency: "CNY".into(), total_cost: report.total_cost * exchange,
                entries: report.entries.iter().map(|e| E { month: e.month.clone(), models: e.models.clone(),
                    input: e.input, output: e.output, cache_read: e.cache_read, cache_write: e.cache_write,
                    message_count: e.message_count, cost: e.cost * exchange }).collect() };
            println!("{}", serde_json::to_string_pretty(&j)?);
        } else { print_monthly_view(&report, exchange); }
        return Ok(());
    }

    // ── hourly view ────────────────────────────────────────────────
    if args.hourly {
        let report = query_hourly_report(&conn, &args)?;
        if args.json_output {
            #[derive(serde::Serialize)] #[serde(rename_all = "camelCase")]
            struct E { hour: String, clients: Vec<String>, models: Vec<String>, input: i64, output: i64, cache_read: i64, cache_write: i64, reasoning: i64, message_count: i32, cost: f64 }
            #[derive(serde::Serialize)] #[serde(rename_all = "camelCase")]
            struct R { currency: String, entries: Vec<E>, total_cost: f64 }
            let j = R { currency: "CNY".into(), total_cost: report.total_cost * exchange,
                entries: report.entries.iter().map(|e| E { hour: e.hour.clone(), clients: e.clients.clone(), models: e.models.clone(),
                    input: e.input, output: e.output, cache_read: e.cache_read, cache_write: e.cache_write,
                    reasoning: e.reasoning, message_count: e.message_count, cost: e.cost * exchange }).collect() };
            println!("{}", serde_json::to_string_pretty(&j)?);
        } else { print_hourly_view(&report, exchange); }
        return Ok(());
    }

    // ── models view (default) ──────────────────────────────────────
    let client_tag = if args.client.is_empty() { "".to_string() } else { args.client.join(",") };
    let cache_key = if !client_tag.is_empty() {
        format!("{}_{}", range_key, client_tag)
    } else { range_key.to_string() };

    let last_record = get_last_record(&conn, &cache_key)?;
    let report = query_model_report(&conn, &args)?;

    let total_in = report.total_input;
    let total_out = report.total_output;
    let total_cache_read = report.total_cache_read;
    let total_cache_write = report.total_cache_write;
    let total_cost = report.total_cost;
    let total_rmb = total_cost * exchange;

    save_record(&conn, &cache_key, total_in, total_out, total_cache_read, total_cache_write, total_cost, total_rmb)?;

    if args.json_output {
        #[derive(serde::Serialize)] #[serde(rename_all = "camelCase")]
        struct JE { client: String, model: String, provider: String, input: i64, output: i64, cache_read: i64, cache_write: i64, reasoning: i64, message_count: i32, cost: f64 }
        #[derive(serde::Serialize)] #[serde(rename_all = "camelCase")]
        struct JR { currency: String, entries: Vec<JE>, total_input: i64, total_output: i64, total_cache_read: i64, total_cache_write: i64, total_cost: f64, processing_time_ms: u32 }
        let out = JR { currency: "CNY".into(),
            entries: report.entries.iter().map(|e| JE { client: e.client.clone(), model: e.model.clone(), provider: e.provider.clone(),
                input: e.input, output: e.output, cache_read: e.cache_read, cache_write: e.cache_write,
                reasoning: e.reasoning, message_count: e.message_count, cost: e.cost * exchange }).collect(),
            total_input: report.total_input, total_output: report.total_output,
            total_cache_read: report.total_cache_read, total_cache_write: report.total_cache_write,
            total_cost: report.total_cost * exchange, processing_time_ms: report.processing_time_ms };
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        print_models_view(&report, exchange, &last_record, range_key);
    }

    Ok(())
}

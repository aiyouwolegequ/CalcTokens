use calctokens_core::{
    pricing, ClientId, HourlyReport, HourlyUsage, LocalParseOptions, ModelReport, ModelUsage,
    MonthlyReport, MonthlyUsage,
};
use chrono::{Datelike, Local, Utc};
use clap::Parser;
use reqwest::blocking::Client;
use rusqlite::{params, Connection};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tabled::builder::Builder;
use tabled::settings::{object::Segment, Modify, Padding, Style, Width};
use tokio::runtime::Runtime;

mod antigravity;

const EXCH_API: &str = "https://api.exchangerate-api.com/v4/latest/USD";
const MIN_CNY_RATE: f64 = 0.01;
const MAX_CNY_RATE: f64 = 100.0;
fn db_path() -> String {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    format!("{}/.calctokens.db", home)
}

#[derive(Parser, Debug)]
#[command(
    name = "calctokens",
    bin_name = "calctokens",
    version,
    about = "AI coding assistant token usage tracker (CNY)"
)]
struct Args {
    /// Optional command or report type: today, month, all, monthly, hourly, clients, upgrade, sync
    #[arg(index = 1, conflicts_with_all = ["today", "month", "all", "monthly", "hourly", "pricing", "clients", "upgrade"])]
    command: Option<String>,
    /// Filter by client(s): claude, opencode, codex, gemini, openclaw, hermes, kimi, qwen, antigravity, etc.
    #[arg(long, short, num_args = 1..)]
    client: Vec<String>,
    /// Today's usage (since 00:00) vs yesterday's total, TOP 3 COST
    #[arg(long, conflicts_with_all = ["month", "all", "monthly", "hourly", "pricing", "clients", "upgrade", "command"])]
    today: bool,
    /// This month's usage vs last month's total, TOP 5 COST
    #[arg(long, conflicts_with_all = ["today", "all", "monthly", "hourly", "pricing", "clients", "upgrade", "command"])]
    month: bool,
    /// All time usage, no DELTA, TOP 10 COST
    #[arg(long, conflicts_with_all = ["today", "month", "monthly", "hourly", "pricing", "clients", "upgrade", "command"])]
    all: bool,
    #[arg(long, conflicts_with_all = ["today", "month", "all", "hourly", "pricing", "clients", "upgrade", "command"])]
    monthly: bool,
    #[arg(long, conflicts_with_all = ["today", "month", "all", "monthly", "pricing", "clients", "upgrade", "command"])]
    hourly: bool,
    /// Look up pricing for a model (e.g. claude-sonnet-4-20250514)
    #[arg(long, value_name = "MODEL_ID")]
    pricing: Option<String>,
    #[arg(long, conflicts_with_all = ["today", "month", "all", "monthly", "hourly", "upgrade", "command"])]
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
    /// Sync OpenRouter model metadata and exchange rates to local database
    #[arg(long, conflicts_with_all = ["today", "month", "all", "monthly", "hourly", "pricing", "clients", "command"])]
    upgrade: bool,
    /// Skip message sync and daily_summary refresh (read-only historical queries)
    #[arg(long, conflicts_with = "sync")]
    no_sync: bool,
    /// Force message sync before running the selected report
    #[arg(long)]
    sync: bool,
}

#[derive(Deserialize, Debug)]
struct ExchangeResp {
    rates: Rates,
}
#[derive(Deserialize, Debug)]
struct Rates {
    #[serde(rename = "CNY")]
    cny: f64,
}

fn validate_cny_rate(rate: f64) -> Option<f64> {
    (rate.is_finite() && (MIN_CNY_RATE..=MAX_CNY_RATE).contains(&rate)).then_some(rate)
}

fn fetch_cny_rate() -> Result<f64, Box<dyn std::error::Error>> {
    let rate = Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()?
        .get(EXCH_API)
        .send()?
        .json::<ExchangeResp>()?
        .rates
        .cny;
    validate_cny_rate(rate).ok_or_else(|| format!("invalid USD/CNY exchange rate: {rate}").into())
}

#[derive(Debug, Clone, Default)]
struct Stats {
    input: i64,
    output: i64,
    cache_read: i64,
    cache_write: i64,
    cost: f64,
}

#[derive(Debug, Clone, Copy, Default)]
struct SyncStats {
    changed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceSnapshot {
    source_count: i64,
    total_size: i64,
    max_modified_ns: i64,
    fingerprint: String,
}

struct HistoryTotals {
    input: i64,
    output: i64,
    cache_read: i64,
    cache_write: i64,
    cost: f64,
    rmb: f64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct HistoryRecord {
    id: i64,
    timestamp: String,
    range: String,
    total_input: i64,
    total_output: i64,
    total_cache_read: i64,
    total_cache_write: i64,
    total_cost: f64,
    total_rmb: f64,
}

// ── DB helpers ──────────────────────────────────────────────────────────

fn init_db(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA temp_store = MEMORY;
         PRAGMA cache_size = -64000;
         PRAGMA busy_timeout = 5000;",
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL, range TEXT NOT NULL,
            total_input REAL NOT NULL, total_output REAL NOT NULL,
            total_cache_read REAL NOT NULL, total_cache_write REAL NOT NULL,
            total_cost REAL NOT NULL, total_rmb REAL NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_history_range_id ON history(range, id DESC)",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS exchange_cache (
            currency TEXT PRIMARY KEY, rate REAL NOT NULL, fetched_date TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS token_cache (
            range TEXT PRIMARY KEY, json_data TEXT NOT NULL, fetched_date TEXT NOT NULL
        )",
        [],
    )?;
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
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_date ON messages(date)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_client ON messages(client)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_date_client ON messages(date, client)",
        [],
    )?;

    // canonical_id: normalized key for aggregation across raw model_id variants.
    // Added via migration so existing databases don't need full rebuild.
    conn.execute("ALTER TABLE messages ADD COLUMN canonical_id TEXT", [])
        .ok(); // Ignore error if column already exists
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_canonical ON messages(canonical_id)",
        [],
    )?;

    // daily_summary: pre-aggregated with canonical_id as the aggregation key.
    // Create this before any migration cleanup that deletes or refreshes rows.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS daily_summary (
            date TEXT NOT NULL,
            client TEXT NOT NULL,
            canonical_id TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            input_tokens INTEGER NOT NULL,
            output_tokens INTEGER NOT NULL,
            cache_read INTEGER NOT NULL,
            cache_write INTEGER NOT NULL,
            reasoning INTEGER NOT NULL DEFAULT 0,
            cost REAL NOT NULL,
            message_count INTEGER NOT NULL,
            turn_count INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (date, client, canonical_id)
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_daily_summary_canonical ON daily_summary(canonical_id)",
        [],
    )?;

    // Snapshot of local source files used to skip expensive parsing when
    // the source tree has not changed since the last successful sync.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_source_snapshots (
            sync_key TEXT PRIMARY KEY,
            source_count INTEGER NOT NULL,
            total_size INTEGER NOT NULL,
            max_modified_ns INTEGER NOT NULL,
            fingerprint TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;

    // Backfill canonical_id for existing rows that don't have one yet.
    // Uses resolve_alias() to map raw/pretty model_ids to a stable canonical form.
    backfill_canonical_ids(conn)?;

    // v0.8.8: Fix Gemini Flash canonical split — all Antigravity Gemini Flash
    // messages use gemini-3.5-flash (distinct from Gemini CLI's gemini-3-flash-preview).
    conn.execute(
        "UPDATE messages SET canonical_id = 'gemini-3.5-flash'
         WHERE client = 'antigravity'
         AND canonical_id = 'gemini-3-flash-preview'",
        [],
    )?;

    // Fix Gemini-3.5-Flash capitalization in canonical_id.
    conn.execute(
        "UPDATE messages SET canonical_id = 'gemini-3.5-flash'
         WHERE canonical_id = 'Gemini-3.5-Flash'",
        [],
    )?;
    conn.execute(
        "DELETE FROM daily_summary WHERE canonical_id = 'Gemini-3.5-Flash'",
        [],
    )?;

    // v1.1.4: Clean up stale Kimi alias rows left behind by the time-based model split.
    // The Kimi parser now maps kimi-code/kimi-for-coding to kimi-k2.5/kimi-k2.6/kimi-k2.7-code
    // based on message timestamp. Old rows with the generic alias remain as duplicates unless
    // a newer split row exists for the same logical message. Delete those stale duplicates
    // and force daily_summary to be rebuilt so aggregates reflect the corrected model split.
    const KIMI_K25_RELEASE_MS: i64 = 1769472000000;
    let deleted: usize = conn.execute(
        "DELETE FROM messages
         WHERE id IN (
             SELECT old.id
             FROM messages old
             JOIN messages new ON old.client = new.client
                 AND old.session_id = new.session_id
                 AND old.timestamp = new.timestamp
                 AND old.input_tokens = new.input_tokens
                 AND old.output_tokens = new.output_tokens
                 AND old.cache_read = new.cache_read
                 AND old.cache_write = new.cache_write
             WHERE old.client = 'kimi'
                 AND old.model_id IN ('kimi-code/kimi-for-coding', 'kimi-for-coding')
                 AND new.model_id IN ('kimi-k2.5', 'kimi-k2.6', 'kimi-k2.7-code')
                 AND old.timestamp >= ?1
         )",
        params![KIMI_K25_RELEASE_MS],
    )?;
    if deleted > 0 {
        conn.execute("DELETE FROM daily_summary WHERE client = 'kimi'", [])?;
        refresh_daily_summary(conn)?;
    }

    // OpenRouter model metadata — upserted by 'calctokens upgrade'.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS openrouter_models (
            model_id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            input_cost REAL,
            output_cost REAL,
            cache_read_cost REAL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;

    // Exchange rate history — appended by 'calctokens upgrade'.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS exchange_rates (
            date TEXT PRIMARY KEY,
            rate REAL NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;

    Ok(())
}
/// One-time backfill: compute canonical_id for messages that don't have one.
/// resolve_alias() maps raw/pretty display names to the canonical pricing key.
fn backfill_canonical_ids(conn: &Connection) -> rusqlite::Result<()> {
    let null_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE canonical_id IS NULL",
        [],
        |row| row.get(0),
    )?;
    if null_count == 0 {
        return Ok(());
    }

    let mut stmt = conn.prepare("SELECT id, model_id FROM messages WHERE canonical_id IS NULL")?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    let mut update = conn.prepare("UPDATE messages SET canonical_id = ?1 WHERE id = ?2")?;
    for (id, model_id) in &rows {
        let canonical = pricing::aliases::resolve_alias(model_id).unwrap_or(model_id);
        update.execute(params![canonical, id])?;
    }

    Ok(())
}

fn get_cached_exchange(conn: &Connection, currency: &str) -> rusqlite::Result<Option<f64>> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let mut stmt =
        conn.prepare("SELECT rate FROM exchange_cache WHERE currency = ? AND fetched_date = ?")?;
    let mut rows = stmt.query(params![currency, today])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

fn get_cached_exchange_any_age(conn: &Connection, currency: &str) -> rusqlite::Result<Option<f64>> {
    let mut stmt = conn.prepare("SELECT rate FROM exchange_cache WHERE currency = ?")?;
    let mut rows = stmt.query(params![currency])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

fn save_exchange_cache(conn: &Connection, currency: &str, rate: f64) -> rusqlite::Result<()> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    conn.execute(
        "INSERT OR REPLACE INTO exchange_cache (currency, rate, fetched_date) VALUES (?1, ?2, ?3)",
        params![currency, rate, today],
    )?;
    Ok(())
}

fn load_exchange_rate_with<F>(
    conn: &Connection,
    currency: &str,
    fetch: F,
) -> Result<f64, Box<dyn std::error::Error>>
where
    F: FnOnce() -> Result<f64, Box<dyn std::error::Error>>,
{
    if let Some(cached) = get_cached_exchange(conn, currency)? {
        if let Some(rate) = validate_cny_rate(cached) {
            return Ok(rate);
        }
    }

    match fetch() {
        Ok(rate) => {
            save_exchange_cache(conn, currency, rate)?;
            Ok(rate)
        }
        Err(err) => {
            if let Some(cached) = get_cached_exchange_any_age(conn, currency)? {
                if let Some(rate) = validate_cny_rate(cached) {
                    eprintln!(
                        "Warning: failed to refresh {currency} exchange rate; using stale cached rate"
                    );
                    return Ok(rate);
                }
            }
            Err(err)
        }
    }
}

fn get_last_record(conn: &Connection, range: &str) -> rusqlite::Result<Option<HistoryRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, range, total_input, total_output, total_cache_read,
                total_cache_write, total_cost, total_rmb
         FROM history WHERE range = ? ORDER BY id DESC LIMIT 1",
    )?;
    let mut rows = stmt.query(params![range])?;
    if let Some(row) = rows.next()? {
        Ok(Some(HistoryRecord {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            range: row.get(2)?,
            total_input: row.get::<_, f64>(3)? as i64,
            total_output: row.get::<_, f64>(4)? as i64,
            total_cache_read: row.get::<_, f64>(5)? as i64,
            total_cache_write: row.get::<_, f64>(6)? as i64,
            total_cost: row.get(7)?,
            total_rmb: row.get(8)?,
        }))
    } else {
        Ok(None)
    }
}

fn save_record(conn: &Connection, range: &str, totals: &HistoryTotals) -> rusqlite::Result<()> {
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    conn.execute(
        "INSERT INTO history (timestamp, range, total_input, total_output, total_cache_read,
                              total_cache_write, total_cost, total_rmb)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            timestamp,
            range,
            totals.input as f64,
            totals.output as f64,
            totals.cache_read as f64,
            totals.cache_write as f64,
            totals.cost,
            totals.rmb
        ],
    )?;
    Ok(())
}

// ── Message syncing ─────────────────────────────────────────────────────
// Parse all client log files, store every message in SQLite.
// Dedup by message_key so repeated runs only add new messages.

fn resolved_sync_clients(clients: Option<&Vec<String>>) -> Vec<String> {
    clients.cloned().unwrap_or_else(|| {
        let mut clients: Vec<String> = ClientId::iter()
            .filter(|c| c.parse_local())
            .map(|c| c.as_str().to_string())
            .collect();
        clients.push("synthetic".to_string());
        clients
    })
}

fn sync_snapshot_key(clients: Option<&Vec<String>>) -> String {
    let mut clients = resolved_sync_clients(clients);
    clients.sort();
    clients.dedup();
    format!("clients:{}", clients.join(","))
}

fn modified_time_ns(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn add_source_path(paths: &mut Vec<PathBuf>, path: Option<PathBuf>) {
    if let Some(path) = path {
        paths.push(path);
    }
}

fn source_snapshot_for_clients(
    clients: Option<&Vec<String>>,
) -> Result<SourceSnapshot, Box<dyn std::error::Error>> {
    let home_dir = calctokens_core::get_home_dir_string(&None)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::NotFound, err))?;
    let clients = resolved_sync_clients(clients);
    let scan_result = calctokens_core::scan_all_clients_with_scanner_settings(
        &home_dir,
        &clients,
        true,
        &Default::default(),
    );

    let mut paths: Vec<PathBuf> = scan_result
        .all_files()
        .into_iter()
        .map(|(_, path)| path)
        .collect();
    paths.extend(scan_result.opencode_dbs);
    add_source_path(&mut paths, scan_result.synthetic_db);
    add_source_path(&mut paths, scan_result.kilo_db);
    add_source_path(&mut paths, scan_result.hermes_db);
    add_source_path(&mut paths, scan_result.goose_db);
    add_source_path(&mut paths, scan_result.zed_db);
    add_source_path(&mut paths, scan_result.kiro_db);
    add_source_path(&mut paths, scan_result.mimocode_db);
    paths.extend(
        scan_result
            .crush_dbs
            .into_iter()
            .map(|source| source.db_path),
    );
    add_source_path(&mut paths, scan_result.opencode_json_dir);

    paths.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    paths.dedup();

    let mut hasher = Sha256::new();
    let mut source_count = 0_i64;
    let mut total_size = 0_i64;
    let mut max_modified_ns = 0_i64;

    for path in paths {
        let Ok(metadata) = std::fs::metadata(&path) else {
            continue;
        };
        let modified_ns = modified_time_ns(&metadata);
        let size = metadata.len().min(i64::MAX as u64) as i64;
        let path_text = path.to_string_lossy();

        source_count += 1;
        total_size = total_size.saturating_add(size);
        max_modified_ns = max_modified_ns.max(modified_ns);

        hasher.update(path_text.as_bytes());
        hasher.update([0]);
        hasher.update(size.to_le_bytes());
        hasher.update(modified_ns.to_le_bytes());
    }

    Ok(SourceSnapshot {
        source_count,
        total_size,
        max_modified_ns,
        fingerprint: format!("{:x}", hasher.finalize()),
    })
}

fn load_source_snapshot(conn: &Connection, key: &str) -> rusqlite::Result<Option<SourceSnapshot>> {
    let mut stmt = conn.prepare(
        "SELECT source_count, total_size, max_modified_ns, fingerprint
         FROM sync_source_snapshots WHERE sync_key = ?1",
    )?;
    let mut rows = stmt.query(params![key])?;
    if let Some(row) = rows.next()? {
        Ok(Some(SourceSnapshot {
            source_count: row.get(0)?,
            total_size: row.get(1)?,
            max_modified_ns: row.get(2)?,
            fingerprint: row.get(3)?,
        }))
    } else {
        Ok(None)
    }
}

fn save_source_snapshot(
    conn: &Connection,
    key: &str,
    snapshot: &SourceSnapshot,
) -> rusqlite::Result<()> {
    let updated_at = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    conn.execute(
        "INSERT INTO sync_source_snapshots
         (sync_key, source_count, total_size, max_modified_ns, fingerprint, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(sync_key) DO UPDATE SET
            source_count = excluded.source_count,
            total_size = excluded.total_size,
            max_modified_ns = excluded.max_modified_ns,
            fingerprint = excluded.fingerprint,
            updated_at = excluded.updated_at",
        params![
            key,
            snapshot.source_count,
            snapshot.total_size,
            snapshot.max_modified_ns,
            snapshot.fingerprint,
            updated_at
        ],
    )?;
    Ok(())
}

fn source_snapshot_changed(
    conn: &Connection,
    clients: Option<&Vec<String>>,
) -> Result<(String, SourceSnapshot, bool), Box<dyn std::error::Error>> {
    let key = sync_snapshot_key(clients);
    let current = source_snapshot_for_clients(clients)?;
    let previous = load_source_snapshot(conn, &key)?;
    Ok((key, current.clone(), previous.as_ref() != Some(&current)))
}

fn sync_messages(
    conn: &Connection,
    rt: &Runtime,
    clients: Option<Vec<String>>,
) -> Result<SyncStats, Box<dyn std::error::Error>> {
    let has_agy = antigravity::has_active_agy_process();

    // ── Antigravity auto sync hook ──
    if let Err(e) = antigravity::sync_antigravity() {
        eprintln!("Warning: Antigravity sync failed: {}", e);
    }

    let opts = LocalParseOptions {
        home_dir: None,
        use_env_roots: true,
        clients: clients.clone(),
        since: None,
        until: None,
        year: None,
        scanner_settings: Default::default(),
    };

    let messages = rt.block_on(calctokens_core::parse_local_unified_messages(opts))?;

    let before_count: usize = conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))?;

    let tx = conn.unchecked_transaction()?;

    // Historical messages must never be deleted just because their original
    // source files have been rotated. The conflict update has a null-safe
    // WHERE guard so parser fixes still update existing rows by message_key
    // without rewriting identical history on every run.
    //
    // NOTE: daily_summary is intentionally NOT deleted here. It is refreshed
    // after this function returns so that a sync that discovers zero new
    // messages does not leave the pre-aggregated table empty.
    {
        let mut stmt = tx.prepare(
            "INSERT INTO messages
             (client, model_id, canonical_id, provider_id, session_id,
              workspace_key, workspace_label,
              timestamp, date,
              input_tokens, output_tokens, cache_read, cache_write, reasoning,
              cost, message_count, agent, is_turn_start, message_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                     ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18, ?19)
             ON CONFLICT(message_key) DO UPDATE SET
                client = excluded.client,
                model_id = excluded.model_id,
                canonical_id = excluded.canonical_id,
                provider_id = excluded.provider_id,
                session_id = excluded.session_id,
                workspace_key = excluded.workspace_key,
                workspace_label = excluded.workspace_label,
                timestamp = excluded.timestamp,
                date = excluded.date,
                input_tokens = excluded.input_tokens,
                output_tokens = excluded.output_tokens,
                cache_read = excluded.cache_read,
                cache_write = excluded.cache_write,
                reasoning = excluded.reasoning,
                cost = excluded.cost,
                message_count = excluded.message_count,
                agent = excluded.agent,
                is_turn_start = excluded.is_turn_start
             WHERE messages.client IS NOT excluded.client
                OR messages.model_id IS NOT excluded.model_id
                OR messages.canonical_id IS NOT excluded.canonical_id
                OR messages.provider_id IS NOT excluded.provider_id
                OR messages.session_id IS NOT excluded.session_id
                OR messages.workspace_key IS NOT excluded.workspace_key
                OR messages.workspace_label IS NOT excluded.workspace_label
                OR messages.timestamp IS NOT excluded.timestamp
                OR messages.date IS NOT excluded.date
                OR messages.input_tokens IS NOT excluded.input_tokens
                OR messages.output_tokens IS NOT excluded.output_tokens
                OR messages.cache_read IS NOT excluded.cache_read
                OR messages.cache_write IS NOT excluded.cache_write
                OR messages.reasoning IS NOT excluded.reasoning
                OR messages.cost IS NOT excluded.cost
                OR messages.message_count IS NOT excluded.message_count
                OR messages.agent IS NOT excluded.agent
                OR messages.is_turn_start IS NOT excluded.is_turn_start",
        )?;
        let mut changed = 0usize;
        for msg in &messages {
            let key = msg.dedup_key.clone().unwrap_or_else(|| {
                format!(
                    "{}:{}:{}:{}:{}:{}",
                    msg.client,
                    msg.session_id,
                    msg.timestamp,
                    msg.model_id,
                    msg.tokens.input,
                    msg.tokens.output
                )
            });
            let canonical_id = pricing::aliases::resolve_alias(&msg.model_id)
                .unwrap_or(&msg.model_id)
                .to_string();
            changed += stmt.execute(params![
                msg.client,
                msg.model_id,
                canonical_id,
                msg.provider_id,
                msg.session_id,
                msg.workspace_key,
                msg.workspace_label,
                msg.timestamp,
                msg.date,
                msg.tokens.input,
                msg.tokens.output,
                msg.tokens.cache_read,
                msg.tokens.cache_write,
                msg.tokens.reasoning,
                msg.cost,
                msg.message_count,
                msg.agent,
                msg.is_turn_start as i32,
                key,
            ])?;
        }
        drop(stmt);
        tx.commit()?;

        let after_count: usize =
            conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))?;
        let inserted = after_count.saturating_sub(before_count);
        if inserted == 0 && has_agy {
            eprintln!("Warning: agy process is running, but 0 new messages were synced. If you recently used agy, this might indicate a sync issue.");
        }
        Ok(SyncStats { changed })
    }
}

/// Rebuild daily_summary from raw messages, grouped by canonical_id.
/// Called only after sync actually changes stored messages, or when the
/// summary table is empty. canonical_id is never NULL after backfill —
/// COALESCE is defensive.
fn refresh_daily_summary(conn: &Connection) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM daily_summary", [])?;
    tx.execute(
        "INSERT OR REPLACE INTO daily_summary
         (date, client, canonical_id, provider_id,
          input_tokens, output_tokens, cache_read, cache_write, reasoning,
          cost, message_count, turn_count)
         SELECT date, client,
                COALESCE(canonical_id, model_id),
                MAX(provider_id),
                SUM(input_tokens), SUM(output_tokens),
                SUM(cache_read), SUM(cache_write), SUM(reasoning),
                SUM(cost), SUM(message_count), SUM(is_turn_start)
         FROM messages
         GROUP BY date, client, COALESCE(canonical_id, model_id)",
        [],
    )?;
    tx.commit()?;
    Ok(())
}

fn daily_summary_is_empty(conn: &Connection) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT NOT EXISTS (SELECT 1 FROM daily_summary LIMIT 1)",
        [],
        |row| row.get(0),
    )
}

// ── SQLite-based report queries ─────────────────────────────────────────

fn build_date_filter(args: &Args) -> (Option<String>, Option<String>) {
    if args.today {
        let today = Local::now().format("%Y-%m-%d").to_string();
        (Some(today.clone()), Some(today))
    } else if args.month {
        let now = Local::now();
        let start = now.format("%Y-%m-01").to_string();
        let end = now.format("%Y-%m-%d").to_string();
        (Some(start), Some(end))
    } else if args.all {
        (None, None)
    } else if args.since.is_none() && args.until.is_none() && args.year.is_none() {
        // Default to today if no other filters
        let today = Local::now().format("%Y-%m-%d").to_string();
        (Some(today.clone()), Some(today))
    } else {
        (args.since.clone(), args.until.clone())
    }
}

fn validate_client_filters(clients: &[String]) -> Result<(), String> {
    let invalid: Vec<&str> = clients
        .iter()
        .map(|client| client.as_str())
        .filter(|client| *client != "synthetic" && ClientId::from_str(client).is_none())
        .collect();

    if invalid.is_empty() {
        return Ok(());
    }

    let mut valid: Vec<&str> = ClientId::ALL.iter().map(|client| client.as_str()).collect();
    valid.push("synthetic");
    valid.sort_unstable();

    Err(format!(
        "unknown client filter(s): {}. Valid clients: {}",
        invalid.join(", "),
        valid.join(", ")
    ))
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
            .map(|i| format!("?{}", params.len() + i + 1))
            .collect();
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

fn query_model_report(
    conn: &Connection,
    args: &Args,
) -> Result<ModelReport, Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    let (where_clause, params) = build_where_clause(args);

    let sql = format!(
        "SELECT client, canonical_id, MAX(provider_id),
                SUM(input_tokens), SUM(output_tokens),
                SUM(cache_read), SUM(cache_write),
                SUM(reasoning), SUM(message_count),
                SUM(cost)
         FROM daily_summary
         WHERE {}
         GROUP BY client, canonical_id
         ORDER BY SUM(cost) DESC",
        where_clause
    );

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
        .iter()
        .map(|p| p as &dyn rusqlite::types::ToSql)
        .collect();
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

fn query_top_models(
    conn: &Connection,
    args: &Args,
    top_n: usize,
) -> Result<Vec<ModelUsage>, Box<dyn std::error::Error>> {
    let (where_clause, params) = build_where_clause(args);

    let sql = format!(
        "SELECT canonical_id, MAX(provider_id),
                SUM(input_tokens), SUM(output_tokens),
                SUM(cache_read), SUM(cache_write),
                SUM(reasoning), SUM(message_count),
                SUM(cost)
         FROM daily_summary
         WHERE {}
         GROUP BY canonical_id
         ORDER BY SUM(cost) DESC
         LIMIT {}",
        where_clause, top_n
    );

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
        .iter()
        .map(|p| p as &dyn rusqlite::types::ToSql)
        .collect();
    let mut rows = stmt.query(param_refs.as_slice())?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next()? {
        entries.push(ModelUsage {
            client: "".to_string(),
            model: row.get(0)?,
            provider: row.get(1)?,
            merged_clients: None,
            workspace_key: None,
            workspace_label: None,
            input: row.get(2)?,
            output: row.get(3)?,
            cache_read: row.get(4)?,
            cache_write: row.get(5)?,
            reasoning: row.get(6)?,
            message_count: row.get(7)?,
            cost: row.get(8)?,
        });
    }

    Ok(entries)
}

fn query_top_usage_models(
    conn: &Connection,
    args: &Args,
    top_n: usize,
) -> Result<Vec<ModelUsage>, Box<dyn std::error::Error>> {
    let (where_clause, params) = build_where_clause(args);

    let sql = format!(
        "SELECT canonical_id, MAX(provider_id),
                SUM(input_tokens), SUM(output_tokens),
                SUM(cache_read), SUM(cache_write),
                SUM(reasoning), SUM(message_count),
                SUM(cost)
         FROM daily_summary
         WHERE {}
         GROUP BY canonical_id
         ORDER BY (SUM(input_tokens) + SUM(output_tokens) + SUM(cache_read) + SUM(cache_write)) DESC
         LIMIT {}",
        where_clause, top_n
    );

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
        .iter()
        .map(|p| p as &dyn rusqlite::types::ToSql)
        .collect();
    let mut rows = stmt.query(param_refs.as_slice())?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next()? {
        entries.push(ModelUsage {
            client: "".to_string(),
            model: row.get(0)?,
            provider: row.get(1)?,
            merged_clients: None,
            workspace_key: None,
            workspace_label: None,
            input: row.get(2)?,
            output: row.get(3)?,
            cache_read: row.get(4)?,
            cache_write: row.get(5)?,
            reasoning: row.get(6)?,
            message_count: row.get(7)?,
            cost: row.get(8)?,
        });
    }

    Ok(entries)
}

fn query_monthly_report(
    conn: &Connection,
    args: &Args,
) -> Result<MonthlyReport, Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    let (where_clause, params) = build_where_clause(args);

    let sql = format!(
        "SELECT substr(date, 1, 7) as month,
                GROUP_CONCAT(DISTINCT canonical_id),
                SUM(input_tokens), SUM(output_tokens),
                SUM(cache_read), SUM(cache_write),
                SUM(message_count), SUM(cost)
         FROM daily_summary
         WHERE {}
         GROUP BY month
         ORDER BY month",
        where_clause
    );

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
        .iter()
        .map(|p| p as &dyn rusqlite::types::ToSql)
        .collect();
    let mut rows = stmt.query(param_refs.as_slice())?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next()? {
        let models_str: Option<String> = row.get(1)?;
        entries.push(MonthlyUsage {
            month: row.get(0)?,
            models: models_str
                .map(|s| s.split(',').map(String::from).collect())
                .unwrap_or_default(),
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

fn query_hourly_report(
    conn: &Connection,
    args: &Args,
) -> Result<HourlyReport, Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    let (where_clause, params) = build_where_clause(args);

    // Build SQL without format! %% escaping — use concat to avoid confusion.
    let sql = [
        "SELECT strftime('%Y-%m-%d %H:00', datetime(timestamp/1000, 'unixepoch', 'localtime')) as hour,",
        "       GROUP_CONCAT(DISTINCT client),",
        "       GROUP_CONCAT(DISTINCT COALESCE(canonical_id, model_id)),",
        "       SUM(input_tokens), SUM(output_tokens),",
        "       SUM(cache_read), SUM(cache_write),",
        "       SUM(reasoning), SUM(message_count),",
        "       SUM(is_turn_start), SUM(cost)",
        " FROM messages",
        " WHERE ",
        &where_clause,
        " GROUP BY hour",
        " ORDER BY hour",
    ]
    .join("\n");
    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
        .iter()
        .map(|p| p as &dyn rusqlite::types::ToSql)
        .collect();
    let mut rows = stmt.query(param_refs.as_slice())?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next()? {
        let clients_str: Option<String> = row.get(1)?;
        let models_str: Option<String> = row.get(2)?;
        entries.push(HourlyUsage {
            hour: row.get(0)?,
            clients: clients_str
                .map(|s| s.split(',').map(String::from).collect())
                .unwrap_or_default(),
            models: models_str
                .map(|s| s.split(',').map(String::from).collect())
                .unwrap_or_default(),
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

fn get_stats_for_range(
    conn: &Connection,
    since: Option<String>,
    until: Option<String>,
    clients: &[String],
) -> rusqlite::Result<Stats> {
    let mut clauses = vec!["1=1".to_string()];
    let mut params: Vec<String> = vec![];

    if let Some(s) = since {
        clauses.push(format!("date >= ?{}", params.len() + 1));
        params.push(s);
    }
    if let Some(u) = until {
        clauses.push(format!("date <= ?{}", params.len() + 1));
        params.push(u);
    }
    if !clients.is_empty() {
        let placeholders: Vec<String> = (0..clients.len())
            .map(|i| format!("?{}", params.len() + i + 1))
            .collect();
        clauses.push(format!("client IN ({})", placeholders.join(",")));
        params.extend(clients.iter().cloned());
    }

    let sql = format!(
        "SELECT SUM(input_tokens), SUM(output_tokens), SUM(cache_read), SUM(cache_write), SUM(cost)
         FROM daily_summary WHERE {}",
        clauses.join(" AND ")
    );

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
        .iter()
        .map(|p| p as &dyn rusqlite::types::ToSql)
        .collect();
    let mut rows = stmt.query(param_refs.as_slice())?;

    if let Some(row) = rows.next()? {
        Ok(Stats {
            input: row.get::<_, Option<i64>>(0)?.unwrap_or(0),
            output: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
            cache_read: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
            cache_write: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            cost: row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
        })
    } else {
        Ok(Stats::default())
    }
}

// ── calctokens-core helpers ───────────────────────────────────────────────

fn fetch_pricing_lookup(
    rt: &Runtime,
    model_id: &str,
) -> Result<Option<pricing::lookup::LookupResult>, Box<dyn std::error::Error>> {
    let svc = rt
        .block_on(pricing::PricingService::get_or_init())
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    Ok(svc.lookup_with_source(model_id, None))
}

// ── Formatting helpers ──────────────────────────────────────────────────

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

fn fmt_diff(n: f64) -> String {
    if n == 0.0 {
        String::from("0")
    } else if n.abs() >= 1_000_000_000_000.0 {
        format!("{:+.2}T", n / 1_000_000_000_000.0)
    } else if n.abs() >= 1_000_000_000.0 {
        format!("{:+.2}B", n / 1_000_000_000.0)
    } else if n.abs() >= 1_000_000.0 {
        format!("{:+.2}M", n / 1_000_000.0)
    } else if n.abs() >= 1_000.0 {
        format!("{:+.2}K", n / 1_000.0)
    } else {
        format!("{:+.0}", n)
    }
}

fn share_pct(cost: f64, total_cost: f64) -> String {
    if total_cost > 0.0 && cost > 0.0 {
        format!("{:.1}%", cost / total_cost * 100.0)
    } else {
        "0.0%".to_string()
    }
}

fn cache_total(cache_read: i64, cache_write: i64) -> i64 {
    cache_read + cache_write
}

fn cache_pct(input: i64, output: i64, cache_read: i64, cache_write: i64) -> f64 {
    let total = input + output + cache_read + cache_write;
    if total > 0 {
        cache_total(cache_read, cache_write) as f64 / total as f64 * 100.0
    } else {
        0.0
    }
}

fn fmt_pct(rate: f64) -> String {
    format!("{:.1}%", rate)
}

fn fmt_pct_diff(rate: f64) -> String {
    if rate == 0.0 {
        String::from("0.0pp")
    } else {
        format!("{:+.1}pp", rate)
    }
}

// ── View printers ───────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn print_models_view(
    report: &ModelReport,
    top_models: &[ModelUsage],
    top_usage_models: &[ModelUsage],
    exchange: f64,
    delta_stats: Option<Stats>,
    range_flag: &str,
    delta_label: &str,
    since_date: Option<&str>,
    today_date: Option<&str>,
) {
    let total_in = report.total_input;
    let total_out = report.total_output;
    let total_cache_read = report.total_cache_read;
    let total_cache_write = report.total_cache_write;
    let total_cost = report.total_cost;
    let total_rmb = total_cost * exchange;
    let total_tokens: f64 = report
        .entries
        .iter()
        .map(|e| (e.input + e.output + e.cache_write + e.cache_read) as f64)
        .sum();

    let metric_label = match range_flag {
        "today" => "TODAY",
        "month" => "MONTH",
        "all" => "ALL",
        _ => "RANGE",
    };

    let (delta_in, delta_out, delta_cache_read, delta_cache_write, delta_cache_rate, delta_rmb) =
        if let Some(ref ds) = delta_stats {
            (
                total_in - ds.input,
                total_out - ds.output,
                total_cache_read - ds.cache_read,
                total_cache_write - ds.cache_write,
                cache_pct(total_in, total_out, total_cache_read, total_cache_write)
                    - cache_pct(ds.input, ds.output, ds.cache_read, ds.cache_write),
                total_rmb - (ds.cost * exchange),
            )
        } else {
            (0, 0, 0, 0, 0.0, 0.0)
        };

    let mut sum_builder = Builder::new();
    let (headers, values): (Vec<&str>, Vec<String>) = if let Some(since) = since_date {
        let today = today_date.unwrap_or("-");
        (
            vec![
                "Today", "Since", "Input", "Output", "Cache", "Cache%", "Total", "CNY",
            ],
            vec![
                today.to_string(),
                since.to_string(),
                fmt_num(total_in as f64),
                fmt_num(total_out as f64),
                fmt_num(cache_total(total_cache_read, total_cache_write) as f64),
                fmt_pct(cache_pct(
                    total_in,
                    total_out,
                    total_cache_read,
                    total_cache_write,
                )),
                fmt_num((total_in + total_out + total_cache_write + total_cache_read) as f64),
                format!("¥{:.2}", total_rmb),
            ],
        )
    } else {
        (
            vec![
                "Metric", "Input", "Output", "Cache", "Cache%", "Total", "CNY",
            ],
            vec![
                metric_label.to_string(),
                fmt_num(total_in as f64),
                fmt_num(total_out as f64),
                fmt_num(cache_total(total_cache_read, total_cache_write) as f64),
                fmt_pct(cache_pct(
                    total_in,
                    total_out,
                    total_cache_read,
                    total_cache_write,
                )),
                fmt_num((total_in + total_out + total_cache_write + total_cache_read) as f64),
                format!("¥{:.2}", total_rmb),
            ],
        )
    };
    sum_builder.push_record(headers);
    sum_builder.push_record(values);
    let mut sum_table = sum_builder.build();
    sum_table
        .with(Style::rounded())
        .with(Padding::new(1, 1, 0, 0));

    let delta_table = if delta_stats.is_some() {
        let mut delta_builder = Builder::new();
        delta_builder.push_record([
            "Δ Metric",
            "Δ Input",
            "Δ Output",
            "Δ Cache",
            "Δ Cache%",
            "Δ Total",
            "Δ CNY",
        ]);
        delta_builder.push_record([
            delta_label,
            &fmt_diff(delta_in as f64),
            &fmt_diff(delta_out as f64),
            &fmt_diff(cache_total(delta_cache_read, delta_cache_write) as f64),
            &fmt_pct_diff(delta_cache_rate),
            &fmt_diff((delta_in + delta_out + delta_cache_read + delta_cache_write) as f64),
            &format!("¥{:+.2}", delta_rmb),
        ]);
        let mut dt = delta_builder.build();
        dt.with(Style::rounded()).with(Padding::new(1, 1, 0, 0));
        Some(dt)
    } else {
        None
    };

    let mut entries: Vec<_> = report.entries.iter().collect();
    entries.sort_by(|a, b| {
        let ta = (a.input + a.output + a.cache_write + a.cache_read) as f64;
        let tb = (b.input + b.output + b.cache_write + b.cache_read) as f64;
        tb.partial_cmp(&ta).unwrap()
    });

    let mut detail_builder = Builder::new();
    detail_builder.push_record([
        "Client", "Model", "CNY", "Input", "Output", "Cache", "Cache%", "Total", "Share",
    ]);
    for entry in &entries {
        let (inp, out, cw, cr) = (
            entry.input as f64,
            entry.output as f64,
            entry.cache_write as f64,
            entry.cache_read as f64,
        );
        if inp == 0.0 && out == 0.0 && cw == 0.0 && cr == 0.0 {
            continue;
        }
        let total = inp + out + cw + cr;
        let share_str = share_pct(total, total_tokens);
        let display_model = pricing::aliases::resolve_pretty_name(&entry.model)
            .unwrap_or(&entry.model)
            .to_string();
        detail_builder.push_record([
            &entry.client,
            &display_model,
            &format!("¥{:.2}", entry.cost * exchange),
            &fmt_num(inp),
            &fmt_num(out),
            &fmt_num(cw + cr),
            &fmt_pct(cache_pct(
                entry.input,
                entry.output,
                entry.cache_read,
                entry.cache_write,
            )),
            &fmt_num(total),
            &share_str,
        ]);
    }
    let mut detail_table = detail_builder.build();
    detail_table
        .with(Style::rounded())
        .with(Padding::new(0, 1, 0, 0));

    let mut top_builder = Builder::new();
    top_builder.push_record(["#", "Model", "Total", "CNY", "Share"]);
    for (i, entry) in top_models.iter().filter(|e| e.cost > 0.0).enumerate() {
        let total = (entry.input + entry.output + entry.cache_write + entry.cache_read) as f64;
        let display_model = pricing::aliases::resolve_pretty_name(&entry.model)
            .unwrap_or(&entry.model)
            .to_string();
        top_builder.push_record([
            &format!("{}", i + 1),
            &display_model,
            &fmt_num(total),
            &format!("¥{:.2}", entry.cost * exchange),
            &share_pct(entry.cost, total_cost),
        ]);
    }
    let mut top_table = top_builder.build();
    top_table.with(Style::rounded());

    let mut top_usage_builder = Builder::new();
    top_usage_builder.push_record(["#", "Model", "Total", "CNY", "Share"]);
    for (i, entry) in top_usage_models
        .iter()
        .filter(|e| (e.input + e.output + e.cache_write + e.cache_read) > 0)
        .enumerate()
    {
        let total = (entry.input + entry.output + entry.cache_write + entry.cache_read) as f64;
        let display_model = pricing::aliases::resolve_pretty_name(&entry.model)
            .unwrap_or(&entry.model)
            .to_string();
        top_usage_builder.push_record([
            &format!("{}", i + 1),
            &display_model,
            &fmt_num(total),
            &format!("¥{:.2}", entry.cost * exchange),
            &share_pct(total, total_tokens),
        ]);
    }
    let mut top_usage_table = top_usage_builder.build();
    top_usage_table.with(Style::rounded());

    println!();
    println!(
        "  calctokens  --  Token Usage Report   [ {} ]",
        metric_label
    );
    println!();
    println!("  SUMMARY");
    print!("{}", sum_table);
    println!();
    if let Some(dt) = delta_table {
        println!("  DELTA ({})", delta_label);
        print!("{}", dt);
        println!();
    }
    println!("  DETAIL");
    print!("{}", detail_table);
    println!();
    let display_count = top_models.iter().filter(|e| e.cost > 0.0).count();
    println!("  TOP {} COST", display_count);
    println!("{}", top_table);

    let display_usage_count = top_usage_models
        .iter()
        .filter(|e| (e.input + e.output + e.cache_write + e.cache_read) > 0)
        .count();
    println!();
    println!("  TOP {} USAGE", display_usage_count);
    println!("{}", top_usage_table);
}

fn print_monthly_view(report: &MonthlyReport, exchange: f64) {
    let total_cost = report.total_cost;
    let total_rmb = total_cost * exchange;
    let total_tokens: f64 = report
        .entries
        .iter()
        .map(|e| (e.input + e.output + e.cache_write + e.cache_read) as f64)
        .sum();

    let mut sum_builder = Builder::new();
    sum_builder.push_record([
        "Period", "Input", "Output", "Cache", "Cache%", "Total", "CNY", "Msgs",
    ]);
    for entry in &report.entries {
        let (inp, out, cw, cr) = (
            entry.input as f64,
            entry.output as f64,
            entry.cache_write as f64,
            entry.cache_read as f64,
        );
        sum_builder.push_record([
            &entry.month,
            &fmt_num(inp),
            &fmt_num(out),
            &fmt_num(cw + cr),
            &fmt_pct(cache_pct(
                entry.input,
                entry.output,
                entry.cache_read,
                entry.cache_write,
            )),
            &fmt_num(inp + out + cw + cr),
            &format!("¥{:.2}", entry.cost * exchange),
            &entry.message_count.to_string(),
        ]);
    }
    let mut sum_table = sum_builder.build();
    sum_table
        .with(Style::rounded())
        .with(Padding::new(1, 1, 0, 0));

    let mut detail_builder = Builder::new();
    detail_builder.push_record(["Month", "Total Tokens", "CNY", "Share"]);
    for entry in &report.entries {
        let total = (entry.input + entry.output + entry.cache_write + entry.cache_read) as f64;
        detail_builder.push_record([
            &entry.month,
            &fmt_num(total),
            &format!("¥{:.2}", entry.cost * exchange),
            &share_pct(total, total_tokens),
        ]);
    }
    let mut detail_table = detail_builder.build();
    detail_table
        .with(Style::rounded())
        .with(Padding::new(0, 1, 0, 0));

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
    let total_tokens: f64 = report
        .entries
        .iter()
        .map(|e| (e.input + e.output + e.cache_write + e.cache_read) as f64)
        .sum();

    let mut detail_builder = Builder::new();
    detail_builder.push_record([
        "Hour", "Clients", "Models", "Input", "Output", "Cache", "Total", "CNY", "Share",
    ]);
    for entry in &report.entries {
        let (inp, out, cw, cr) = (
            entry.input as f64,
            entry.output as f64,
            entry.cache_write as f64,
            entry.cache_read as f64,
        );
        let total = inp + out + cw + cr;
        let clients = entry
            .clients
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        let models = entry
            .models
            .iter()
            .take(2)
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        detail_builder.push_record([
            &entry.hour,
            &clients,
            &models,
            &fmt_num(inp),
            &fmt_num(out),
            &fmt_num(cw + cr),
            &fmt_num(total),
            &format!("¥{:.2}", entry.cost * exchange),
            &share_pct(total, total_tokens),
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

fn print_pricing_view(model_id: &str, result: &pricing::lookup::LookupResult, exchange: f64) {
    let p = &result.pricing;
    let input_rmb = p.input_cost_per_token.unwrap_or(0.0) * 1_000_000.0 * exchange;
    let output_rmb = p.output_cost_per_token.unwrap_or(0.0) * 1_000_000.0 * exchange;
    let cache_rmb = p.cache_read_input_token_cost.unwrap_or(0.0) * 1_000_000.0 * exchange;

    let mut builder = Builder::new();
    builder.push_record(["Model", "Source", "Input/M", "Output/M", "Cache Read/M"]);
    builder.push_record([
        model_id,
        &result.source,
        &format!("¥{:.4}", input_rmb),
        &format!("¥{:.4}", output_rmb),
        &format!("¥{:.4}", cache_rmb),
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
    let home_dir = calctokens_core::get_home_dir_string(&None).unwrap_or_default();
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

// ── upgrade command ──────────────────────────────────────────────────────

fn do_upgrade(conn: &Connection, rt: &Runtime) -> Result<(), Box<dyn std::error::Error>> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let now_iso = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    // 1. Fetch and store exchange rate
    println!("[upgrade] Fetching USD/CNY exchange rate...");
    let rate = fetch_cny_rate()?;
    save_exchange_cache(conn, "CNY", rate)?;
    conn.execute(
        "INSERT OR REPLACE INTO exchange_rates (date, rate, updated_at) VALUES (?1, ?2, ?3)",
        params![today, rate, now_iso],
    )?;
    println!("[upgrade] USD/CNY rate: {:.4}", rate);

    // 2. Fetch fresh OpenRouter model data. Keep the old pricing cache intact
    // unless the fresh fetch succeeds and the cache layer atomically replaces it.
    println!("[upgrade] Fetching OpenRouter model metadata...");
    let models = rt
        .block_on(calctokens_core::pricing::openrouter::fetch_all_models_fresh())
        .map_err(|e| {
            std::io::Error::other(format!("OpenRouter model metadata sync failed: {e}"))
        })?;

    // 3. Upsert into openrouter_models table
    let mut inserted = 0usize;
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO openrouter_models
             (model_id, display_name, input_cost, output_cost, cache_read_cost, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for (model_id, pricing) in &models {
            // Derive a display name from the model ID (strip provider prefix)
            let display_name = model_id.split('/').next_back().unwrap_or(model_id);
            stmt.execute(params![
                model_id,
                display_name,
                pricing.input_cost_per_token,
                pricing.output_cost_per_token,
                pricing.cache_read_input_token_cost,
                now_iso,
            ])?;
            inserted += 1;
        }
    }
    tx.commit()?;

    // Refresh daily_summary with the new canonical data
    refresh_daily_summary(conn)?;

    println!("[upgrade] Stored {} models in openrouter_models", inserted);
    println!("[upgrade] Done. Database is up to date.");
    Ok(())
}

// ── main ────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = Args::parse();
    let mut sync_only = false;

    if let Some(ref cmd) = args.command {
        match cmd.as_str() {
            "today" => args.today = true,
            "month" => args.month = true,
            "all" => args.all = true,
            "monthly" => args.monthly = true,
            "hourly" => args.hourly = true,
            "clients" => args.clients = true,
            "upgrade" => args.upgrade = true,
            "sync" => {
                args.sync = true;
                sync_only = true;
            }
            _ => {
                eprintln!("Error: Unknown command or argument '{}'", cmd);
                std::process::exit(1);
            }
        }
    }

    if args.sync && args.no_sync {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "--sync and --no-sync cannot be used together",
        )
        .into());
    }

    if let Err(e) = validate_client_filters(&args.client) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, e).into());
    }

    // ── clients view ────────────────────────────────────────────────
    if args.clients {
        if args.json_output {
            let home_dir = calctokens_core::get_home_dir_string(&None).unwrap_or_default();
            let mut entries = Vec::new();
            for cid in ClientId::ALL.iter() {
                let def = cid.data();
                let path = def.resolve_path(&home_dir);
                let exists = std::path::Path::new(&path).exists();
                entries
                    .push(serde_json::json!({ "client": def.id, "path": path, "exists": exists }));
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({ "clients": entries }))?
            )
        } else {
            print_clients_view();
        }
        return Ok(());
    }

    let conn = Connection::open(db_path())?;
    init_db(&conn)?;
    let rt = Runtime::new()?;

    // ── upgrade: sync OpenRouter metadata + exchange rate ──────────────
    if args.upgrade {
        do_upgrade(&conn, &rt)?;
        let _ = conn.execute("PRAGMA optimize;", []);
        return Ok(());
    }

    let range_key = if args.today {
        "today"
    } else if args.month {
        "month"
    } else if args.all {
        "all"
    } else if args.monthly {
        "monthly"
    } else if args.hourly {
        "hourly"
    } else {
        "default"
    };

    let exchange = load_exchange_rate_with(&conn, "CNY", fetch_cny_rate)?;

    if get_cached_exchange(&conn, "PRICING")?.is_some() {
        std::env::set_var("CALCTOKENS_PRICING_CACHE_ONLY", "1");
    } else {
        std::env::set_var("CALCTOKENS_PRICING_CACHE_ONLY", "0");
        save_exchange_cache(&conn, "PRICING", 1.0)?;
    }

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

    // ── Sync messages from log files into SQLite ────────────────────
    let sync_clients = if args.client.is_empty() {
        None
    } else {
        Some(args.client.clone())
    };
    let sync_stats = if args.no_sync {
        SyncStats::default()
    } else {
        let mut snapshot_to_save = None;
        let should_sync = if args.sync || daily_summary_is_empty(&conn).unwrap_or(true) {
            true
        } else {
            match source_snapshot_changed(&conn, sync_clients.as_ref()) {
                Ok((key, snapshot, changed)) => {
                    if changed {
                        snapshot_to_save = Some((key, snapshot));
                    }
                    changed
                }
                Err(e) => {
                    eprintln!("Warning: source snapshot check failed: {}", e);
                    true
                }
            }
        };

        if should_sync {
            match sync_messages(&conn, &rt, sync_clients.clone()) {
                Ok(stats) => {
                    match source_snapshot_changed(&conn, sync_clients.as_ref()) {
                        Ok((key, snapshot, _)) => snapshot_to_save = Some((key, snapshot)),
                        Err(e) => eprintln!("Warning: source snapshot save skipped: {}", e),
                    }
                    if let Some((key, snapshot)) = snapshot_to_save {
                        if let Err(e) = save_source_snapshot(&conn, &key, &snapshot) {
                            eprintln!("Warning: source snapshot save failed: {}", e);
                        }
                    }
                    stats
                }
                Err(e) => {
                    eprintln!("Warning: message sync failed (data may be stale): {}", e);
                    SyncStats::default()
                }
            }
        } else {
            SyncStats::default()
        }
    };
    if !args.no_sync {
        let should_refresh_summary =
            sync_stats.changed > 0 || daily_summary_is_empty(&conn).unwrap_or(true);
        if should_refresh_summary {
            if let Err(e) = refresh_daily_summary(&conn) {
                eprintln!("Warning: daily_summary refresh failed: {}", e);
            }
        }
    }
    if sync_only {
        println!(
            "Synced local messages: {} changed row(s).",
            sync_stats.changed
        );
        let _ = conn.execute("PRAGMA optimize;", []);
        return Ok(());
    }

    // ── monthly view ───────────────────────────────────────────────
    if args.monthly {
        let report = query_monthly_report(&conn, &args)?;
        if args.json_output {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct E {
                month: String,
                models: Vec<String>,
                input: i64,
                output: i64,
                cache_read: i64,
                cache_write: i64,
                message_count: i32,
                cost: f64,
            }
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct R {
                currency: String,
                entries: Vec<E>,
                total_cost: f64,
            }
            let j = R {
                currency: "CNY".into(),
                total_cost: report.total_cost * exchange,
                entries: report
                    .entries
                    .iter()
                    .map(|e| E {
                        month: e.month.clone(),
                        models: e.models.clone(),
                        input: e.input,
                        output: e.output,
                        cache_read: e.cache_read,
                        cache_write: e.cache_write,
                        message_count: e.message_count,
                        cost: e.cost * exchange,
                    })
                    .collect(),
            };
            println!("{}", serde_json::to_string_pretty(&j)?);
        } else {
            print_monthly_view(&report, exchange);
        }
        return Ok(());
    }

    // ── hourly view ────────────────────────────────────────────────
    if args.hourly {
        let report = query_hourly_report(&conn, &args)?;
        if args.json_output {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct E {
                hour: String,
                clients: Vec<String>,
                models: Vec<String>,
                input: i64,
                output: i64,
                cache_read: i64,
                cache_write: i64,
                reasoning: i64,
                message_count: i32,
                cost: f64,
            }
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct R {
                currency: String,
                entries: Vec<E>,
                total_cost: f64,
            }
            let j = R {
                currency: "CNY".into(),
                total_cost: report.total_cost * exchange,
                entries: report
                    .entries
                    .iter()
                    .map(|e| E {
                        hour: e.hour.clone(),
                        clients: e.clients.clone(),
                        models: e.models.clone(),
                        input: e.input,
                        output: e.output,
                        cache_read: e.cache_read,
                        cache_write: e.cache_write,
                        reasoning: e.reasoning,
                        message_count: e.message_count,
                        cost: e.cost * exchange,
                    })
                    .collect(),
            };
            println!("{}", serde_json::to_string_pretty(&j)?);
        } else {
            print_hourly_view(&report, exchange);
        }
        return Ok(());
    }

    // ── models view (default / today / month / all) ────────────────
    let (delta_stats, top_n, delta_label) = if args.today {
        let yesterday = (Local::now() - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        let stats = get_stats_for_range(
            &conn,
            Some(yesterday.clone()),
            Some(yesterday),
            &args.client,
        )?;
        (Some(stats), 3, "vs yesterday")
    } else if args.month {
        let now = Local::now();
        let year = now.year();
        let month = now.month();
        let (ly, lm) = if month == 1 {
            (year - 1, 12)
        } else {
            (year, month - 1)
        };
        let last_month_start = format!("{:04}-{:02}-01", ly, lm);
        let last_month_end = format!(
            "{:04}-{:02}-{:02}",
            ly,
            lm,
            if [1, 3, 5, 7, 8, 10, 12].contains(&lm) {
                31
            } else if [4, 6, 9, 11].contains(&lm) {
                30
            } else {
                if (ly % 4 == 0 && ly % 100 != 0) || ly % 400 == 0 {
                    29
                } else {
                    28
                }
            }
        );
        let stats = get_stats_for_range(
            &conn,
            Some(last_month_start),
            Some(last_month_end),
            &args.client,
        )?;
        (Some(stats), 5, "vs last month")
    } else if args.all {
        (None, 10, "")
    } else if args.since.is_some() || args.until.is_some() || args.year.is_some() {
        (None, 3, "")
    } else {
        // Default: Delta vs last check
        let client_tag = if args.client.is_empty() {
            "".to_string()
        } else {
            args.client.join(",")
        };
        let cache_key = format!("default_{}", client_tag);
        let last = get_last_record(&conn, &cache_key)?;
        let stats = last.map(|r| Stats {
            input: r.total_input,
            output: r.total_output,
            cache_read: r.total_cache_read,
            cache_write: r.total_cache_write,
            cost: r.total_cost,
        });
        (stats, 3, "since last check")
    };

    let report = query_model_report(&conn, &args)?;
    let top_models = query_top_models(&conn, &args, top_n)?;
    let top_usage_models = query_top_usage_models(&conn, &args, top_n)?;

    // For the "all" view, show today's date and the earliest recorded date.
    let today_date: Option<String> = if range_key == "all" {
        Some(Local::now().format("%Y-%m-%d").to_string())
    } else {
        None
    };
    let since_date: Option<String> = if range_key == "all" {
        conn.query_row(
            "SELECT MIN(date) FROM messages WHERE date IS NOT NULL AND date != ''",
            [],
            |row| row.get(0),
        )
        .ok()
    } else {
        None
    };

    // Save record for "since last check" if in default mode
    if !args.today
        && !args.month
        && !args.all
        && args.since.is_none()
        && args.until.is_none()
        && args.year.is_none()
    {
        let client_tag = if args.client.is_empty() {
            "".to_string()
        } else {
            args.client.join(",")
        };
        let cache_key = format!("default_{}", client_tag);
        save_record(
            &conn,
            &cache_key,
            &HistoryTotals {
                input: report.total_input,
                output: report.total_output,
                cache_read: report.total_cache_read,
                cache_write: report.total_cache_write,
                cost: report.total_cost,
                rmb: report.total_cost * exchange,
            },
        )?;
    }

    if args.json_output {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct JE {
            client: String,
            model: String,
            provider: String,
            input: i64,
            output: i64,
            cache_read: i64,
            cache_write: i64,
            reasoning: i64,
            message_count: i32,
            cost: f64,
        }
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct JR {
            currency: String,
            entries: Vec<JE>,
            total_input: i64,
            total_output: i64,
            total_cache_read: i64,
            total_cache_write: i64,
            total_cost: f64,
            processing_time_ms: u32,
            #[serde(skip_serializing_if = "Option::is_none")]
            since_date: Option<String>,
        }
        let out = JR {
            currency: "CNY".into(),
            entries: report
                .entries
                .iter()
                .map(|e| JE {
                    client: e.client.clone(),
                    model: pricing::aliases::resolve_pretty_name(&e.model)
                        .unwrap_or(&e.model)
                        .to_string(),
                    provider: e.provider.clone(),
                    input: e.input,
                    output: e.output,
                    cache_read: e.cache_read,
                    cache_write: e.cache_write,
                    reasoning: e.reasoning,
                    message_count: e.message_count,
                    cost: e.cost * exchange,
                })
                .collect(),
            total_input: report.total_input,
            total_output: report.total_output,
            total_cache_read: report.total_cache_read,
            total_cache_write: report.total_cache_write,
            total_cost: report.total_cost * exchange,
            processing_time_ms: report.processing_time_ms,
            since_date: since_date.clone(),
        };
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        print_models_view(
            &report,
            &top_models,
            &top_usage_models,
            exchange,
            delta_stats,
            range_key,
            delta_label,
            since_date.as_deref(),
            today_date.as_deref(),
        );
    }

    let _ = conn.execute("PRAGMA optimize;", []);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_cny_rate_accepts_plausible_rate() {
        assert_eq!(validate_cny_rate(7.2), Some(7.2));
    }

    #[test]
    fn validate_cny_rate_rejects_zero_negative_and_extreme_rates() {
        assert!(validate_cny_rate(0.0).is_none());
        assert!(validate_cny_rate(-1.0).is_none());
        assert!(validate_cny_rate(1000.0).is_none());
    }

    #[test]
    fn cache_pct_uses_cache_tokens_over_total_tokens() {
        assert_eq!(cache_pct(100, 50, 25, 25), 25.0);
        assert_eq!(cache_pct(100, 50, 50, 0), 25.0);
        assert_eq!(cache_pct(0, 0, 0, 0), 0.0);
    }

    #[test]
    fn init_db_succeeds_on_fresh_database() {
        let conn = Connection::open_in_memory().unwrap();

        init_db(&conn).unwrap();

        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'daily_summary'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 1);
    }

    #[test]
    fn init_db_creates_sync_source_snapshots() {
        let conn = Connection::open_in_memory().unwrap();

        init_db(&conn).unwrap();

        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'sync_source_snapshots'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 1);
    }

    #[test]
    fn sync_snapshot_key_is_order_independent() {
        let first = vec!["codex".to_string(), "claude".to_string()];
        let second = vec!["claude".to_string(), "codex".to_string()];

        assert_eq!(
            sync_snapshot_key(Some(&first)),
            sync_snapshot_key(Some(&second))
        );
    }

    #[test]
    fn save_and_load_source_snapshot_round_trips() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        let snapshot = SourceSnapshot {
            source_count: 2,
            total_size: 128,
            max_modified_ns: 42,
            fingerprint: "abc123".to_string(),
        };

        save_source_snapshot(&conn, "clients:codex", &snapshot).unwrap();

        assert_eq!(
            load_source_snapshot(&conn, "clients:codex").unwrap(),
            Some(snapshot)
        );
    }

    #[test]
    fn load_exchange_rate_falls_back_to_stale_cached_rate() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO exchange_cache (currency, rate, fetched_date)
             VALUES ('CNY', 7.1, '2000-01-01')",
            [],
        )
        .unwrap();

        let rate = load_exchange_rate_with(&conn, "CNY", || {
            Err(std::io::Error::other("network down").into())
        })
        .unwrap();

        assert_eq!(rate, 7.1);
    }

    #[test]
    fn load_exchange_rate_rejects_invalid_stale_cached_rate() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO exchange_cache (currency, rate, fetched_date)
             VALUES ('CNY', 1000.0, '2000-01-01')",
            [],
        )
        .unwrap();

        let err = load_exchange_rate_with(&conn, "CNY", || {
            Err(std::io::Error::other("network down").into())
        })
        .unwrap_err();

        assert!(err.to_string().contains("network down"));
    }

    #[test]
    fn validate_client_filters_accepts_known_clients_and_synthetic() {
        let clients = vec!["claude".to_string(), "synthetic".to_string()];

        assert!(validate_client_filters(&clients).is_ok());
    }

    #[test]
    fn validate_client_filters_rejects_unknown_clients() {
        let clients = vec!["claud".to_string()];

        let err = validate_client_filters(&clients).unwrap_err();

        assert!(err.contains("unknown client filter(s): claud"));
        assert!(err.contains("claude"));
    }
}

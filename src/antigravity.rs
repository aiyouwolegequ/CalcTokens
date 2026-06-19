use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

const MAX_HTTP_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_HEARTBEAT_RESPONSE_BYTES: u64 = 64 * 1024;
const MAX_TRAJECTORIES_PER_ENDPOINT: usize = 4096;
const MAX_AGY_CLI_SESSIONS: usize = 4096;
const MAX_METADATA_ITEMS_PER_SESSION: usize = 2048;
const MAX_RETRY_INFOS_PER_METADATA: usize = 4096;
const MAX_JSONL_LINES_PER_ARTIFACT: usize = 20_000;
const MAX_JSONL_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
struct ProcessCandidate {
    pid: u32,
    csrf_token: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SessionManifestEntry {
    session_id: String,
    artifact_path: String,
    last_modified_ms: i64,
    step_count: i64,
    connection_fingerprint: String,
    artifact_hash: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ConnectionEntry {
    fingerprint: String,
    pid: u32,
    port: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Manifest {
    version: i32,
    synced_at: String,
    connections: Vec<ConnectionEntry>,
    sessions: Vec<SessionManifestEntry>,
}

fn get_cache_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    Path::new(&home)
        .join(".config")
        .join("calctokens")
        .join("antigravity-cache")
}

fn get_agy_data_dirs() -> Vec<PathBuf> {
    let home = match std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        Ok(h) => h,
        Err(_) => return vec![],
    };
    let mut dirs = Vec::new();
    let gemini_dir = Path::new(&home).join(".gemini");
    for sub in &["antigravity-cli", "antigravity", "antigravity-ide"] {
        let dir = gemini_dir.join(sub);
        if dir.exists() {
            dirs.push(dir);
        }
    }
    dirs
}

#[derive(Debug, Clone)]
struct AgySessionInfo {
    session_id: String,
    pb_mtime_ms: i64,
    pb_size: i64,
}

/// Discover sessions from all agy/Antigravity conversations directories.
/// Used as a fallback when GetAllCascadeTrajectories returns empty.
fn get_agy_cli_sessions() -> Vec<AgySessionInfo> {
    let data_dirs = get_agy_data_dirs();
    let mut sessions_map: std::collections::HashMap<String, AgySessionInfo> =
        std::collections::HashMap::new();

    for data_dir in data_dirs {
        let conversations_dir = data_dir.join("conversations");
        if !conversations_dir.exists() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&conversations_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "pb" || e == "db") {
                    if let Some(session_id) = path.file_stem().and_then(|s| s.to_str()) {
                        let (mtime_ms, size) = std::fs::metadata(&path)
                            .ok()
                            .map(|m| {
                                let mt = m
                                    .modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                                    .map(|d| d.as_millis() as i64)
                                    .unwrap_or(0);
                                let sz = m.len() as i64;
                                (mt, sz)
                            })
                            .unwrap_or((0, 0));

                        let session_id_str = session_id.to_string();
                        // De-duplicate: keep the one with the latest mtime_ms
                        if let Some(existing) = sessions_map.get(&session_id_str) {
                            if mtime_ms > existing.pb_mtime_ms {
                                sessions_map.insert(
                                    session_id_str.clone(),
                                    AgySessionInfo {
                                        session_id: session_id_str,
                                        pb_mtime_ms: mtime_ms,
                                        pb_size: size,
                                    },
                                );
                            }
                        } else {
                            sessions_map.insert(
                                session_id_str.clone(),
                                AgySessionInfo {
                                    session_id: session_id_str,
                                    pb_mtime_ms: mtime_ms,
                                    pb_size: size,
                                },
                            );
                        }
                    }
                }
            }
        }
    }
    sessions_map.into_values().collect()
}

fn get_active_processes() -> Vec<ProcessCandidate> {
    let mut candidates = Vec::new();
    let output = match Command::new("ps")
        .args(["-ww", "-eo", "pid,ppid,args"])
        .output()
    {
        Ok(out) => out,
        Err(_) => return candidates,
    };

    let output_str = String::from_utf8_lossy(&output.stdout);
    let my_pid = std::process::id();

    for line in output_str.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parts: Vec<&str> = trimmed.splitn(3, |c: char| c.is_whitespace()).collect();
        if parts.len() < 3 {
            continue;
        }

        let pid_str = parts[0];
        let _ppid_str = parts[1];
        let args = parts[2];

        let pid = match pid_str.parse::<u32>() {
            Ok(p) => p,
            Err(_) => continue,
        };

        if pid == my_pid {
            continue;
        }

        let lower_args = args.to_lowercase();
        let args_split: Vec<&str> = args.split_whitespace().collect();
        let is_agy = args_split.contains(&"agy")
            || args.contains("/agy")
            || args.ends_with("agy")
            || (lower_args.contains("language_server")
                && (lower_args.contains("antigravity") || lower_args.contains("gemini")));

        if is_agy {
            let csrf_token = extract_csrf_token(args);
            candidates.push(ProcessCandidate { pid, csrf_token });
        }
    }
    candidates
}

fn extract_csrf_token(args: &str) -> String {
    let lower = args.to_lowercase();
    if let Some(idx) = lower.find("--csrf_token") {
        let tail = &args[idx + "--csrf_token".len()..];
        let trimmed_tail = tail.trim_start();
        if let Some(stripped) = trimmed_tail.strip_prefix('=') {
            let val = stripped.trim_start();
            val.split_whitespace().next().unwrap_or("").to_string()
        } else {
            trimmed_tail
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string()
        }
    } else {
        "".to_string()
    }
}

fn read_limited_response(mut response: Response, max_bytes: u64) -> Result<Vec<u8>, String> {
    let mut limited = response.by_ref().take(max_bytes.saturating_add(1));
    let mut body = Vec::new();
    limited
        .read_to_end(&mut body)
        .map_err(|err| format!("failed to read response body: {}", err))?;
    if body.len() as u64 > max_bytes {
        return Err(format!("response body exceeded {} bytes", max_bytes));
    }
    Ok(body)
}

fn parse_json_response_limited(response: Response) -> Result<Value, String> {
    parse_json_bytes_limited(
        &read_limited_response(response, MAX_HTTP_RESPONSE_BYTES)?,
        MAX_HTTP_RESPONSE_BYTES,
    )
}

fn parse_json_bytes_limited(body: &[u8], max_bytes: u64) -> Result<Value, String> {
    if body.len() as u64 > max_bytes {
        return Err(format!("response body exceeded {} bytes", max_bytes));
    }
    serde_json::from_slice(body).map_err(|err| format!("invalid JSON response: {}", err))
}

fn get_trajectory_summaries(
    data: &Value,
) -> Result<Option<&serde_json::Map<String, Value>>, String> {
    match data.get("trajectorySummaries") {
        Some(Value::Object(obj)) => Ok(Some(obj)),
        Some(_) => Err("'trajectorySummaries' key is not an object".to_string()),
        None if data.as_object().is_some_and(|obj| obj.is_empty()) => Ok(None),
        None => Err("missing 'trajectorySummaries' key".to_string()),
    }
}

fn append_json_line_bounded(
    lines: &mut Vec<String>,
    total_bytes: &mut usize,
    value: Value,
) -> bool {
    append_json_line_bounded_with_limits(
        lines,
        total_bytes,
        value,
        MAX_JSONL_LINES_PER_ARTIFACT,
        MAX_JSONL_ARTIFACT_BYTES,
    )
}

fn append_json_line_bounded_with_limits(
    lines: &mut Vec<String>,
    total_bytes: &mut usize,
    value: Value,
    max_lines: usize,
    max_bytes: usize,
) -> bool {
    let Ok(line) = serde_json::to_string(&value) else {
        return false;
    };
    let next_bytes = match total_bytes
        .checked_add(line.len())
        .and_then(|bytes| bytes.checked_add(1))
    {
        Some(bytes) => bytes,
        None => return false,
    };
    if lines.len() >= max_lines || next_bytes > max_bytes {
        return false;
    }
    lines.push(line);
    *total_bytes = next_bytes;
    true
}

/// Single lsof call to discover all TCP LISTEN ports with their PIDs.
/// Returns Vec of (pid, port) pairs for all listening TCP sockets.
fn get_all_listening_ports() -> Vec<(u32, u16)> {
    let mut result = Vec::new();
    let output = match Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-Pan"])
        .output()
    {
        Ok(out) => out,
        Err(_) => return result,
    };
    let output_str = String::from_utf8_lossy(&output.stdout);
    for line in output_str.lines().skip(1) {
        if !line.contains("LISTEN") || !line.contains("TCP") {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 2 {
            continue;
        }
        let pid: u32 = match fields[1].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if let Some(port) = extract_port_from_lsof_line(line) {
            result.push((pid, port));
        }
    }
    result
}

fn get_listening_ports(pid: u32, all_ports: &[(u32, u16)]) -> Vec<u16> {
    all_ports
        .iter()
        .filter(|(p, _)| *p == pid)
        .map(|(_, port)| *port)
        .collect()
}

fn extract_port_from_lsof_line(line: &str) -> Option<u16> {
    if !line.contains("LISTEN") || !line.contains("TCP") {
        return None;
    }
    if let Some(tcp_idx) = line.find("TCP ") {
        let after_tcp = line[tcp_idx + 4..].trim_start();
        if let Some(colon_idx) = after_tcp.find(':') {
            let port_part = &after_tcp[colon_idx + 1..];
            let port_str = port_part.split_whitespace().next().unwrap_or("");
            let clean_port = port_str.trim_end_matches(|c: char| !c.is_numeric());
            if let Ok(port) = clean_port.parse::<u16>() {
                return Some(port);
            }
        }
    }
    None
}

fn probe_heartbeat(client: &Client, port: u16, csrf_token: &str) -> Option<(String, HeaderMap)> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert("Connect-Protocol-Version", HeaderValue::from_static("1"));
    if !csrf_token.is_empty() {
        if let Ok(val) = HeaderValue::from_str(csrf_token) {
            headers.insert("X-Codeium-Csrf-Token", val);
        }
    }

    let payload = json!({"uuid": "00000000-0000-0000-0000-000000000000"});
    let http_url = format!(
        "http://127.0.0.1:{}/exa.language_server_pb.LanguageServerService/Heartbeat",
        port
    );
    let https_url = format!(
        "https://127.0.0.1:{}/exa.language_server_pb.LanguageServerService/Heartbeat",
        port
    );

    // Probe HTTP and HTTPS in parallel — the agy daemon listens on one protocol per port.
    let (tx, rx) = std::sync::mpsc::channel();
    for (url, protocol, hdrs) in [
        (http_url, "http", headers.clone()),
        (https_url, "https", headers),
    ] {
        let tx = tx.clone();
        let cl = client.clone();
        let pld = payload.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let res = cl.post(&url).headers(hdrs).json(&pld).send().ok()?;
                if !res.status().is_success() {
                    return None;
                }
                let body = read_limited_response(res, MAX_HEARTBEAT_RESPONSE_BYTES).ok()?;
                let text = String::from_utf8_lossy(&body);
                if text.contains("lastExtensionHeartbeat") {
                    Some(protocol.to_string())
                } else {
                    None
                }
            })();
            let _ = tx.send(result);
        });
    }
    drop(tx);

    // Return the first successful protocol
    for _ in 0..2 {
        if let Ok(Some(protocol)) = rx.recv() {
            // Rebuild headers for the winning protocol
            let mut h = HeaderMap::new();
            h.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            h.insert("Connect-Protocol-Version", HeaderValue::from_static("1"));
            if !csrf_token.is_empty() {
                if let Ok(val) = HeaderValue::from_str(csrf_token) {
                    h.insert("X-Codeium-Csrf-Token", val);
                }
            }
            return Some((protocol, h));
        }
    }
    None
}

fn sanitize_session_id(session_id: &str) -> String {
    let mut sanitized = String::new();
    for c in session_id.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
            sanitized.push(c);
        } else {
            sanitized.push('-');
        }
    }
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "session".to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_timestamp(val: Option<&Value>) -> i64 {
    let val = match val {
        Some(v) => v,
        None => return 0,
    };
    match val {
        Value::Number(num) => num.as_i64().unwrap_or(0),
        Value::String(s) => {
            if let Ok(i) = s.parse::<i64>() {
                return i;
            }
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
                return dt.timestamp_millis();
            }
            if let Ok(dt) = s.parse::<chrono::DateTime<chrono::Utc>>() {
                return dt.timestamp_millis();
            }
            0
        }
        _ => 0,
    }
}

fn resolve_model_id(chat_model: &Value) -> String {
    let mut model_id = chat_model
        .get("responseModel")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if model_id.is_empty() {
        model_id = chat_model
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
    }
    if model_id.is_empty() {
        model_id = "unknown";
    }
    model_id.to_string()
}

fn to_safe_i64(value: Option<&Value>) -> i64 {
    let value = match value {
        Some(v) => v,
        None => return 0,
    };
    match value {
        Value::Number(num) => num.as_i64().unwrap_or(0).max(0),
        Value::String(s) => {
            if let Ok(i) = s.parse::<i64>() {
                return i.max(0);
            }
            if let Ok(f) = s.parse::<f64>() {
                return (f as i64).max(0);
            }
            0
        }
        Value::Bool(true) => 1,
        Value::Bool(false) => 0,
        _ => 0,
    }
}

/// agy CLI v1.0.1+: outputTokens may include thinking tokens.
/// responseOutputTokens is the visible output when present.
fn resolve_output_and_reasoning(raw_out: i64, raw_reasoning: i64, response_out: i64) -> (i64, i64) {
    if response_out > 0 {
        (response_out, raw_reasoning)
    } else if raw_reasoning > 0 && raw_reasoning < raw_out {
        (raw_out - raw_reasoning, raw_reasoning)
    } else {
        (raw_out, raw_reasoning)
    }
}

fn sha256_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

fn process_trajectory(
    client: &Client,
    session_id: &str,
    protocol: &str,
    port: u16,
    headers: &HeaderMap,
    sessions_dir: &Path,
) -> Option<(String, String)> {
    let url = format!(
        "{}://127.0.0.1:{}/exa.language_server_pb.LanguageServerService/GetCascadeTrajectoryGeneratorMetadata",
        protocol, port
    );
    let payload = json!({ "cascadeId": session_id });

    let res = match client
        .post(&url)
        .headers(headers.clone())
        .json(&payload)
        .send()
    {
        Ok(r) => r,
        Err(_) => return None,
    };

    if !res.status().is_success() {
        return None;
    }

    let data: Value = match parse_json_response_limited(res) {
        Ok(d) => d,
        Err(_) => return None,
    };

    let metadata = data.get("generatorMetadata").and_then(|v| v.as_array())?;

    if metadata.len() > MAX_METADATA_ITEMS_PER_SESSION {
        return None;
    }

    let mut jsonl_lines = Vec::new();
    let mut jsonl_bytes = 0usize;

    for meta in metadata {
        let chat_model = meta.get("chatModel").unwrap_or(meta);
        let model_id = resolve_model_id(chat_model);

        let mut created_at_ms = parse_timestamp(
            chat_model
                .get("chatStartMetadata")
                .and_then(|m| m.get("createdAt")),
        );
        if created_at_ms == 0 {
            created_at_ms = parse_timestamp(chat_model.get("createdAt"));
        }

        let meta_line = json!({
            "type": "session_meta",
            "sessionId": session_id,
            "modelId": model_id,
            "timestamp": created_at_ms
        });
        if !append_json_line_bounded(&mut jsonl_lines, &mut jsonl_bytes, meta_line) {
            return None;
        }

        let retry_infos = chat_model
            .get("retryInfos")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if retry_infos.len() > MAX_RETRY_INFOS_PER_METADATA {
            return None;
        }

        let mut has_retry_usage = false;

        for retry in &retry_infos {
            let usage = retry.get("usage").unwrap_or(retry);
            let inp = to_safe_i64(usage.get("inputTokens"));
            let raw_out = to_safe_i64(usage.get("outputTokens"));
            let raw_reasoning = to_safe_i64(usage.get("thinkingOutputTokens"));
            let response_out = to_safe_i64(usage.get("responseOutputTokens"));
            let cache_read = to_safe_i64(usage.get("cacheReadTokens"));

            let (out, reasoning) =
                resolve_output_and_reasoning(raw_out, raw_reasoning, response_out);

            let mut timestamp_ms =
                parse_timestamp(usage.get("createdAt").or_else(|| usage.get("timestamp")));
            if timestamp_ms == 0 {
                timestamp_ms = created_at_ms;
            }

            if inp == 0 && out == 0 && cache_read == 0 && reasoning == 0 {
                continue;
            }

            has_retry_usage = true;

            let usage_line = json!({
                "type": "usage",
                "sessionId": session_id,
                "modelId": model_id,
                "timestamp": timestamp_ms,
                "input": inp,
                "output": out,
                "cacheRead": cache_read,
                "cacheWrite": 0,
                "reasoning": reasoning,
                "responseId": usage.get("responseId")
            });
            if !append_json_line_bounded(&mut jsonl_lines, &mut jsonl_bytes, usage_line) {
                return None;
            }
        }

        // Fallback: if retryInfos is empty, extract usage directly from chatModel.usage
        // (agy CLI v1.0.1+ may store usage at the top level instead of inside retryInfos)
        if !has_retry_usage {
            if let Some(usage) = chat_model.get("usage") {
                let inp = to_safe_i64(usage.get("inputTokens"));
                let raw_out = to_safe_i64(usage.get("outputTokens"));
                let raw_reasoning = to_safe_i64(usage.get("thinkingOutputTokens"));
                let response_out = to_safe_i64(usage.get("responseOutputTokens"));
                let cache_read = to_safe_i64(usage.get("cacheReadTokens"));
                let cache_write = to_safe_i64(usage.get("cacheWriteTokens"));

                let (out, reasoning) =
                    resolve_output_and_reasoning(raw_out, raw_reasoning, response_out);

                if !(inp == 0 && out == 0 && cache_read == 0 && cache_write == 0 && reasoning == 0)
                {
                    let usage_line = json!({
                        "type": "usage",
                        "sessionId": session_id,
                        "modelId": model_id,
                        "timestamp": created_at_ms,
                        "input": inp,
                        "output": out,
                        "cacheRead": cache_read,
                        "cacheWrite": cache_write,
                        "reasoning": reasoning,
                        "responseId": usage.get("responseId")
                    });
                    if !append_json_line_bounded(&mut jsonl_lines, &mut jsonl_bytes, usage_line) {
                        return None;
                    }
                }
            }
        }
    }

    if jsonl_lines.is_empty() {
        return None;
    }

    let content = jsonl_lines.join("\n") + "\n";

    let sanitized = sanitize_session_id(session_id);
    let session_hash = sha256_hash(session_id);
    let filename = format!("{}-{}.jsonl", sanitized, &session_hash[..16]);
    let filepath = sessions_dir.join(&filename);

    if let Ok(mut f) = File::create(&filepath) {
        if f.write_all(content.as_bytes()).is_ok() {
            let file_hash = sha256_hash(&content);
            return Some((
                format!("sessions/{}", filename),
                format!("sha256:{}", file_hash),
            ));
        }
    }

    None
}

pub fn sync_antigravity() -> Result<(), Box<dyn std::error::Error>> {
    let candidates = get_active_processes();
    let all_ports = get_all_listening_ports();

    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_millis(2000))
        .build()?;

    let mut active_endpoints = Vec::new();
    let mut connections = Vec::new();

    let has_candidates = !candidates.is_empty();
    let mut probe_handles = Vec::new();
    for cand in candidates {
        let ports = get_listening_ports(cand.pid, &all_ports);
        for port in ports {
            let client = client.clone();
            let csrf_token = cand.csrf_token.clone();
            let pid = cand.pid;

            let handle = std::thread::spawn(move || {
                probe_heartbeat(&client, port, &csrf_token)
                    .map(|(protocol, headers)| (protocol, port, headers, pid))
            });
            probe_handles.push(handle);
        }
    }

    for handle in probe_handles {
        if let Ok(Some((protocol, port, headers, pid))) = handle.join() {
            let fingerprint = format!("pid:{}:port:{}", pid, port);
            connections.push(ConnectionEntry {
                fingerprint: fingerprint.clone(),
                pid,
                port,
            });
            active_endpoints.push((protocol, port, headers, fingerprint));
        }
    }

    if active_endpoints.is_empty() {
        if has_candidates {
            eprintln!("Warning: agy/Antigravity process is running, but no active ports could be probed. Heartbeat verification failed.");
        }
        return Ok(());
    }

    let cache_dir = get_cache_dir();
    let sessions_dir = cache_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir)?;

    let manifest_path = cache_dir.join("manifest.json");

    let mut old_sessions = std::collections::HashMap::new();
    if manifest_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
            if let Ok(manifest) = serde_json::from_str::<Manifest>(&content) {
                for sess in manifest.sessions {
                    old_sessions.insert(sess.session_id.clone(), sess);
                }
            }
        }
    }

    let mut new_sessions = std::collections::HashMap::new();

    for (protocol, port, headers, fingerprint) in &active_endpoints {
        let url = format!("{}://127.0.0.1:{}/exa.language_server_pb.LanguageServerService/GetAllCascadeTrajectories", protocol, port);
        let res = match client
            .post(&url)
            .headers(headers.clone())
            .json(&json!({}))
            .send()
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "Warning: Failed to connect to GetAllCascadeTrajectories on port {}: {}",
                    port, e
                );
                continue;
            }
        };

        if !res.status().is_success() {
            eprintln!(
                "Warning: GetAllCascadeTrajectories returned HTTP status {} on port {}",
                res.status(),
                port
            );
            continue;
        }

        let data: Value = match parse_json_response_limited(res) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Warning: Failed to parse GetAllCascadeTrajectories JSON response on port {}: {}", port, e);
                continue;
            }
        };

        let trajectories = match get_trajectory_summaries(&data) {
            Ok(Some(obj)) => obj,
            Ok(None) => continue,
            Err(e) => {
                eprintln!(
                    "Warning: GetAllCascadeTrajectories response is {} on port {}",
                    e, port
                );
                continue;
            }
        };

        if trajectories.len() > MAX_TRAJECTORIES_PER_ENDPOINT {
            eprintln!(
                "Warning: GetAllCascadeTrajectories returned {} trajectories on port {}, exceeding the limit of {}; skipping endpoint",
                trajectories.len(),
                port,
                MAX_TRAJECTORIES_PER_ENDPOINT
            );
            continue;
        }

        for (session_id, summary) in trajectories {
            let last_modified_ms = parse_timestamp(summary.get("lastModifiedTime"));
            let step_count = summary
                .get("stepCount")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            if let Some(old_sess) = old_sessions.get(session_id) {
                if old_sess.step_count == step_count
                    && old_sess.last_modified_ms == last_modified_ms
                    && cache_dir.join(&old_sess.artifact_path).exists()
                {
                    new_sessions.insert(session_id.clone(), old_sess.clone());
                    continue;
                }
            }

            if let Some((artifact_path, artifact_hash)) =
                process_trajectory(&client, session_id, protocol, *port, headers, &sessions_dir)
            {
                new_sessions.insert(
                    session_id.clone(),
                    SessionManifestEntry {
                        session_id: session_id.clone(),
                        artifact_path,
                        last_modified_ms,
                        step_count,
                        connection_fingerprint: fingerprint.clone(),
                        artifact_hash,
                    },
                );
            }
        }
    }

    // ── agy CLI fallback: when GetAllCascadeTrajectories returns empty ──
    // agy CLI (v1.0.1+) stores conversations as .pb files but the list endpoint
    // returns {}. Discover sessions from the filesystem and fetch them directly
    // via GetCascadeTrajectoryGeneratorMetadata.
    let agy_sessions = get_agy_cli_sessions();
    if !agy_sessions.is_empty() {
        if agy_sessions.len() > MAX_AGY_CLI_SESSIONS {
            eprintln!(
                "Warning: discovered {} agy CLI sessions, exceeding the limit of {}; skipping agy CLI fallback",
                agy_sessions.len(),
                MAX_AGY_CLI_SESSIONS
            );
            return Ok(());
        }
        // Prefer an agy-specific endpoint (no CSRF token), fall back to any active endpoint
        let fallback_endpoint = active_endpoints.first().cloned();
        let agy_endpoint = active_endpoints
            .iter()
            .find(|(_, _, headers, _)| !headers.contains_key("X-Codeium-Csrf-Token"))
            .cloned()
            .or(fallback_endpoint);

        if let Some((protocol, port, headers, _fingerprint)) = agy_endpoint {
            for agy_sess in &agy_sessions {
                if new_sessions.contains_key(&agy_sess.session_id) {
                    continue;
                }
                if let Some(old_sess) = old_sessions.get(&agy_sess.session_id) {
                    if old_sess.last_modified_ms == agy_sess.pb_mtime_ms
                        && old_sess.step_count == agy_sess.pb_size
                        && cache_dir.join(&old_sess.artifact_path).exists()
                    {
                        new_sessions.insert(agy_sess.session_id.clone(), old_sess.clone());
                        continue;
                    }
                }
                if let Some((artifact_path, artifact_hash)) = process_trajectory(
                    &client,
                    &agy_sess.session_id,
                    &protocol,
                    port,
                    &headers,
                    &sessions_dir,
                ) {
                    new_sessions.insert(
                        agy_sess.session_id.clone(),
                        SessionManifestEntry {
                            session_id: agy_sess.session_id.clone(),
                            artifact_path,
                            last_modified_ms: agy_sess.pb_mtime_ms,
                            step_count: agy_sess.pb_size,
                            connection_fingerprint: format!("agy-cli:{}", agy_sess.session_id),
                            artifact_hash,
                        },
                    );
                }
            }
        }
    }

    for (session_id, old_sess) in old_sessions {
        if let std::collections::hash_map::Entry::Vacant(entry) = new_sessions.entry(session_id) {
            let filepath = cache_dir.join(&old_sess.artifact_path);
            if filepath.exists() {
                entry.insert(old_sess);
            }
        }
    }

    let now_utc = chrono::Utc::now().to_rfc3339();
    let manifest = Manifest {
        version: 1,
        synced_at: now_utc,
        connections,
        sessions: new_sessions.into_values().collect(),
    };

    if let Ok(mut f) = File::create(&manifest_path) {
        if let Ok(json_str) = serde_json::to_string_pretty(&manifest) {
            let _ = f.write_all(json_str.as_bytes());
        }
    }

    Ok(())
}

pub fn has_active_agy_process() -> bool {
    !get_active_processes().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_bytes_limited_rejects_oversized_body() {
        let body = br#"{"trajectorySummaries":{}}"#;

        let err = parse_json_bytes_limited(body, 4).unwrap_err();

        assert!(err.contains("response body exceeded"));
    }

    #[test]
    fn parse_json_bytes_limited_accepts_body_within_limit() {
        let body = br#"{"trajectorySummaries":{}}"#;

        let value = parse_json_bytes_limited(body, body.len() as u64).unwrap();

        assert!(value.get("trajectorySummaries").is_some());
    }

    #[test]
    fn get_trajectory_summaries_treats_empty_response_as_no_sessions() {
        let value = json!({});

        let summaries = get_trajectory_summaries(&value).unwrap();

        assert!(summaries.is_none());
    }

    #[test]
    fn get_trajectory_summaries_rejects_non_empty_response_without_key() {
        let value = json!({"status": "ok"});

        let err = get_trajectory_summaries(&value).unwrap_err();

        assert!(err.contains("missing 'trajectorySummaries' key"));
    }

    #[test]
    fn append_json_line_bounded_rejects_line_count_over_limit() {
        let mut lines = Vec::new();
        let mut total_bytes = 0usize;

        assert!(append_json_line_bounded_with_limits(
            &mut lines,
            &mut total_bytes,
            json!({"type":"session_meta"}),
            1,
            1024
        ));
        assert!(!append_json_line_bounded_with_limits(
            &mut lines,
            &mut total_bytes,
            json!({"type":"usage"}),
            1,
            1024
        ));
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn append_json_line_bounded_rejects_byte_count_over_limit() {
        let mut lines = Vec::new();
        let mut total_bytes = 0usize;

        assert!(!append_json_line_bounded_with_limits(
            &mut lines,
            &mut total_bytes,
            json!({"type":"session_meta"}),
            10,
            4
        ));
        assert!(lines.is_empty());
    }
}

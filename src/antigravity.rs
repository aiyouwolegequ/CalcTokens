use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};

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

fn get_active_processes() -> Vec<ProcessCandidate> {
    let mut candidates = Vec::new();
    let output = match Command::new("ps").args(&["-ww", "-eo", "pid,ppid,args"]).output() {
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
        let mut is_agy = false;
        
        let args_split: Vec<&str> = args.split_whitespace().collect();
        if args_split.contains(&"agy") || args.contains("/agy") || args.ends_with("agy") {
            is_agy = true;
        } else if lower_args.contains("language_server") && (lower_args.contains("antigravity") || lower_args.contains("gemini")) {
            is_agy = true;
        }
        
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
        if trimmed_tail.starts_with('=') {
            let val = trimmed_tail[1..].trim_start();
            val.split_whitespace().next().unwrap_or("").to_string()
        } else {
            trimmed_tail.split_whitespace().next().unwrap_or("").to_string()
        }
    } else {
        "".to_string()
    }
}

fn get_listening_ports(pid: u32) -> Vec<u16> {
    let mut ports = Vec::new();
    let output = match Command::new("lsof").args(&["-Pan", "-p", &pid.to_string(), "-i"]).output() {
        Ok(out) => out,
        Err(_) => return ports,
    };
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    for line in output_str.lines() {
        if let Some(port) = extract_port_from_lsof_line(line) {
            if !ports.contains(&port) {
                ports.push(port);
            }
        }
    }
    ports
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
    
    // Try HTTP
    let http_url = format!("http://127.0.0.1:{}/exa.language_server_pb.LanguageServerService/Heartbeat", port);
    if let Ok(res) = client.post(&http_url).headers(headers.clone()).json(&payload).send() {
        if res.status().is_success() {
            if let Ok(text) = res.text() {
                if text.contains("lastExtensionHeartbeat") {
                    return Some(("http".to_string(), headers));
                }
            }
        }
    }
    
    // Try HTTPS
    let https_url = format!("https://127.0.0.1:{}/exa.language_server_pb.LanguageServerService/Heartbeat", port);
    if let Ok(res) = client.post(&https_url).headers(headers.clone()).json(&payload).send() {
        if res.status().is_success() {
            if let Ok(text) = res.text() {
                if text.contains("lastExtensionHeartbeat") {
                    return Some(("https".to_string(), headers));
                }
            }
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
    let mut model_id = chat_model.get("responseModel")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if model_id.is_empty() {
        model_id = chat_model.get("model")
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
        Value::Bool(b) => if *b { 1 } else { 0 },
        _ => 0,
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
    
    let res = match client.post(&url).headers(headers.clone()).json(&payload).send() {
        Ok(r) => r,
        Err(_) => return None,
    };
    
    if !res.status().is_success() {
        return None;
    }
    
    let data: Value = match res.json() {
        Ok(d) => d,
        Err(_) => return None,
    };
    
    let metadata = match data.get("generatorMetadata").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return None,
    };
    
    let mut jsonl_lines = Vec::new();
    
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
        if let Ok(line_str) = serde_json::to_string(&meta_line) {
            jsonl_lines.push(line_str);
        }
        
        let retry_infos = chat_model
            .get("retryInfos")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
            
        for retry in retry_infos {
            let usage = retry.get("usage").unwrap_or(&retry);
            let inp = to_safe_i64(usage.get("inputTokens"));
            let out = to_safe_i64(usage.get("outputTokens"));
            let cache_read = to_safe_i64(usage.get("cacheReadTokens"));
            let reasoning = to_safe_i64(usage.get("thinkingOutputTokens"));
            
            let mut timestamp_ms = parse_timestamp(
                usage
                    .get("createdAt")
                    .or_else(|| usage.get("timestamp")),
            );
            if timestamp_ms == 0 {
                timestamp_ms = created_at_ms;
            }
            
            if inp == 0 && out == 0 && cache_read == 0 && reasoning == 0 {
                continue;
            }
            
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
            if let Ok(line_str) = serde_json::to_string(&usage_line) {
                jsonl_lines.push(line_str);
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
            return Some((format!("sessions/{}", filename), format!("sha256:{}", file_hash)));
        }
    }
    
    None
}

pub fn sync_antigravity() -> Result<(), Box<dyn std::error::Error>> {
    let candidates = get_active_processes();
    
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_millis(1500))
        .build()?;
        
    let mut active_endpoints = Vec::new();
    let mut connections = Vec::new();
    
    for cand in candidates {
        let pid = cand.pid;
        let csrf_token = cand.csrf_token;
        let ports = get_listening_ports(pid);
        
        for port in ports {
            if let Some((protocol, headers)) = probe_heartbeat(&client, port, &csrf_token) {
                let fingerprint = format!("pid:{}:port:{}", pid, port);
                connections.push(ConnectionEntry {
                    fingerprint: fingerprint.clone(),
                    pid,
                    port,
                });
                
                active_endpoints.push((protocol, port, headers, fingerprint));
            }
        }
    }
    
    if active_endpoints.is_empty() {
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
    
    for (protocol, port, headers, fingerprint) in active_endpoints {
        let url = format!("{}://127.0.0.1:{}/exa.language_server_pb.LanguageServerService/GetAllCascadeTrajectories", protocol, port);
        let res = match client.post(&url).headers(headers.clone()).json(&json!({})).send() {
            Ok(r) => r,
            Err(_) => continue,
        };
        
        if !res.status().is_success() {
            continue;
        }
        
        let data: Value = match res.json() {
            Ok(d) => d,
            Err(_) => continue,
        };
        
        let trajectories = match data.get("trajectorySummaries").and_then(|v| v.as_object()) {
            Some(obj) => obj,
            None => continue,
        };
        
        for (session_id, summary) in trajectories {
            let last_modified_ms = parse_timestamp(summary.get("lastModifiedTime"));
            let step_count = summary.get("stepCount").and_then(|v| v.as_i64()).unwrap_or(0);
            
            if let Some(old_sess) = old_sessions.get(session_id) {
                if old_sess.step_count == step_count
                    && old_sess.last_modified_ms == last_modified_ms
                    && cache_dir.join(&old_sess.artifact_path).exists()
                {
                    new_sessions.insert(session_id.clone(), old_sess.clone());
                    continue;
                }
            }
            
            if let Some((artifact_path, artifact_hash)) = process_trajectory(
                &client,
                session_id,
                &protocol,
                port,
                &headers,
                &sessions_dir,
            ) {
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
    
    for (session_id, old_sess) in old_sessions {
        if !new_sessions.contains_key(&session_id) {
            let filepath = cache_dir.join(&old_sess.artifact_path);
            if filepath.exists() {
                new_sessions.insert(session_id, old_sess);
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

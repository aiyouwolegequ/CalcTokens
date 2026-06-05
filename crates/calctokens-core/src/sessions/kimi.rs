//! Kimi CLI session parser
//!
//! Parses wire.jsonl files from ~/.kimi/sessions/[GROUP_ID]/[SESSION_UUID]/wire.jsonl
//! Token data comes from StatusUpdate messages in the wire protocol.

use super::utils::file_modified_timestamp_ms;
use super::UnifiedMessage;
use crate::{provider_identity, TokenBreakdown};
use serde::Deserialize;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Top-level wire.jsonl line: either metadata, a timestamped message, or a usage record
#[derive(Debug, Deserialize)]
struct WireLine {
    // Shared / Old format fields
    timestamp: Option<f64>,
    message: Option<WireMessage>,
    #[serde(rename = "type")]
    line_type: Option<String>,

    // New format (usage.record) fields
    model: Option<String>,
    usage: Option<NewTokenUsage>,
    #[serde(rename = "usageScope")]
    usage_scope: Option<String>,
    time: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct NewTokenUsage {
    #[serde(rename = "inputOther")]
    input_other: Option<i64>,
    output: Option<i64>,
    #[serde(rename = "inputCacheRead")]
    input_cache_read: Option<i64>,
    #[serde(rename = "inputCacheCreation")]
    input_cache_creation: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct WireMessage {
    #[serde(rename = "type")]
    msg_type: String,
    payload: Option<StatusPayload>,
}

#[derive(Debug, Deserialize)]
struct StatusPayload {
    token_usage: Option<TokenUsage>,
    #[allow(dead_code)]
    message_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenUsage {
    input_other: Option<i64>,
    output: Option<i64>,
    input_cache_read: Option<i64>,
    input_cache_creation: Option<i64>,
}

/// Default model name when config is not available
const DEFAULT_MODEL: &str = "kimi-for-coding";
const DEFAULT_PROVIDER: &str = "moonshotai";

/// Read model name from config files (.kimi/config.json or .kimi-code/config.toml)
fn read_model_from_config(wire_path: &Path) -> String {
    for ancestor in wire_path.ancestors() {
        // Check for .kimi-code config.toml
        let config_toml = ancestor.join("config.toml");
        if config_toml.is_file() {
            if let Ok(content) = std::fs::read_to_string(&config_toml) {
                for line in content.lines() {
                    if let Some(rest) = line.trim().strip_prefix("default_model") {
                        if let Some(eq_val) = rest.trim().strip_prefix('=') {
                            let val = eq_val.trim().trim_matches('"').trim_matches('\'').trim();
                            if !val.is_empty() {
                                return val.to_string();
                            }
                        }
                    }
                }
            }
        }

        // Check for .kimi config.json
        let config_json = ancestor.join("config.json");
        if config_json.is_file() {
            if let Ok(content) = std::fs::read_to_string(&config_json) {
                if let Ok(bytes) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(model) = bytes.get("model").and_then(|v| v.as_str()) {
                        if !model.is_empty() {
                            return model.to_string();
                        }
                    }
                }
            }
        }
    }
    DEFAULT_MODEL.to_string()
}

/// Extract session ID from the wire.jsonl path.
/// Handles both path structures:
/// - Old: ~/.kimi/sessions/GROUP_ID/SESSION_UUID/wire.jsonl
/// - New: ~/.kimi-code/sessions/GROUP_ID/SESSION_UUID/agents/main/wire.jsonl
fn extract_session_id(path: &Path) -> String {
    let mut current = path.parent();
    while let Some(p) = current {
        if p.file_name().and_then(|n| n.to_str()) == Some("agents") {
            if let Some(parent) = p.parent() {
                if let Some(name) = parent.file_name().and_then(|n| n.to_str()) {
                    return name.to_string();
                }
            }
        }
        current = p.parent();
    }

    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Parse a Kimi CLI wire.jsonl file (supporting old Kimi CLI and Kimi Code formats)
pub fn parse_kimi_file(path: &Path) -> Vec<UnifiedMessage> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let model_from_config = read_model_from_config(path);
    let session_id = extract_session_id(path);

    let reader = BufReader::new(file);
    let mut messages: Vec<UnifiedMessage> = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut bytes = trimmed.as_bytes().to_vec();
        let wire_line = match simd_json::from_slice::<WireLine>(&mut bytes) {
            Ok(wl) => wl,
            Err(_) => continue,
        };

        // Check for new format: usage.record
        if wire_line.line_type.as_deref() == Some("usage.record") {
            if wire_line.usage_scope.as_deref() != Some("turn") {
                continue;
            }

            let usage = match wire_line.usage {
                Some(u) => u,
                None => continue,
            };

            let input = usage.input_other.unwrap_or(0).max(0);
            let output = usage.output.unwrap_or(0).max(0);
            let cache_read = usage.input_cache_read.unwrap_or(0).max(0);
            let cache_write = usage.input_cache_creation.unwrap_or(0).max(0);

            if input + output + cache_read + cache_write == 0 {
                continue;
            }

            let model = wire_line.model.unwrap_or_else(|| model_from_config.clone());
            let timestamp_ms = wire_line
                .time
                .map(|t| t as i64)
                .unwrap_or_else(|| file_modified_timestamp_ms(path));

            messages.push(UnifiedMessage::new_with_dedup(
                "kimi",
                model.clone(),
                provider_for_kimi_model(&model),
                session_id.clone(),
                timestamp_ms,
                TokenBreakdown {
                    input,
                    output,
                    cache_read,
                    cache_write,
                    reasoning: 0,
                },
                0.0,
                None,
            ));
            continue;
        }

        // Skip metadata lines (first line: {"type": "metadata", ...})
        if wire_line.line_type.as_deref() == Some("metadata") {
            continue;
        }

        let message = match wire_line.message {
            Some(m) => m,
            None => continue,
        };

        // Only process StatusUpdate messages
        if message.msg_type != "StatusUpdate" {
            continue;
        }

        let payload = match message.payload {
            Some(p) => p,
            None => continue,
        };

        let token_usage = match payload.token_usage {
            Some(u) => u,
            None => continue,
        };

        // Convert Unix seconds (float) to milliseconds, fallback to file mtime
        let timestamp_ms = wire_line
            .timestamp
            .map(|ts| (ts * 1000.0) as i64)
            .unwrap_or_else(|| file_modified_timestamp_ms(path));

        let input = token_usage.input_other.unwrap_or(0).max(0);
        let output = token_usage.output.unwrap_or(0).max(0);
        let cache_read = token_usage.input_cache_read.unwrap_or(0).max(0);
        let cache_write = token_usage.input_cache_creation.unwrap_or(0).max(0);

        // Skip entries with zero tokens
        if input + output + cache_read + cache_write == 0 {
            continue;
        }

        let dedup_key = payload.message_id;

        messages.push(UnifiedMessage::new_with_dedup(
            "kimi",
            model_from_config.clone(),
            provider_for_kimi_model(&model_from_config),
            session_id.clone(),
            timestamp_ms,
            TokenBreakdown {
                input,
                output,
                cache_read,
                cache_write,
                // Kimi wire protocol does not expose reasoning tokens; all reasoning included in output
                reasoning: 0,
            },
            0.0,
            dedup_key,
        ));
    }

    messages
}

fn provider_for_kimi_model(model: &str) -> String {
    provider_identity::inferred_provider_from_model(model)
        .unwrap_or(DEFAULT_PROVIDER)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_parse_kimi_valid_status_update() {
        let content = r#"{"type": "metadata", "protocol_version": "1.3"}
{"timestamp": 1770983426.420942, "message": {"type": "StatusUpdate", "payload": {"token_usage": {"input_other": 1562, "output": 2463, "input_cache_read": 0, "input_cache_creation": 0}, "message_id": "chatcmpl-xxx"}}}"#;
        let file = create_test_file(content);

        let messages = parse_kimi_file(file.path());

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].client, "kimi");
        assert_eq!(messages[0].model_id, "kimi-for-coding");
        assert_eq!(messages[0].provider_id, "moonshotai");
        assert_eq!(messages[0].tokens.input, 1562);
        assert_eq!(messages[0].tokens.output, 2463);
        assert_eq!(messages[0].tokens.cache_read, 0);
        assert_eq!(messages[0].tokens.cache_write, 0);
        // Timestamp: 1770983426.420942 * 1000 = 1770983426420
        assert_eq!(messages[0].timestamp, 1770983426420);
    }

    #[test]
    fn test_parse_kimi_multi_turn() {
        let content = r#"{"type": "metadata", "protocol_version": "1.3"}
{"timestamp": 1770983400.0, "message": {"type": "TurnBegin", "payload": {"user_input": "hello"}}}
{"timestamp": 1770983410.0, "message": {"type": "StatusUpdate", "payload": {"token_usage": {"input_other": 100, "output": 200, "input_cache_read": 0, "input_cache_creation": 0}, "message_id": "msg-1"}}}
{"timestamp": 1770983420.0, "message": {"type": "TurnBegin", "payload": {"user_input": "world"}}}
{"timestamp": 1770983430.0, "message": {"type": "StatusUpdate", "payload": {"token_usage": {"input_other": 300, "output": 400, "input_cache_read": 50, "input_cache_creation": 0}, "message_id": "msg-2"}}}"#;
        let file = create_test_file(content);

        let messages = parse_kimi_file(file.path());

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].tokens.input, 100);
        assert_eq!(messages[0].tokens.output, 200);
        assert_eq!(messages[1].tokens.input, 300);
        assert_eq!(messages[1].tokens.output, 400);
        assert_eq!(messages[1].tokens.cache_read, 50);
    }

    #[test]
    fn test_parse_kimi_uses_configured_model_for_provider_inference() {
        let dir = tempfile::TempDir::new().unwrap();
        let kimi_dir = dir.path().join(".kimi");
        let session_dir = kimi_dir.join("sessions/group-1/session-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(kimi_dir.join("config.json"), r#"{"model":"MiniMax-M2.7"}"#).unwrap();
        let wire_path = session_dir.join("wire.jsonl");
        std::fs::write(
            &wire_path,
            r#"{"timestamp": 1770983426.420942, "message": {"type": "StatusUpdate", "payload": {"token_usage": {"input_other": 100, "output": 50, "input_cache_read": 0, "input_cache_creation": 0}, "message_id": "msg-1"}}}"#,
        )
        .unwrap();

        let messages = parse_kimi_file(&wire_path);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].model_id, "MiniMax-M2.7");
        assert_eq!(messages[0].provider_id, "minimax");
    }

    #[test]
    fn test_parse_kimi_code_usage_record() {
        let content = r#"{"type":"usage.record","model":"kimi-code/kimi-for-coding","usage":{"inputOther":3032,"output":121,"inputCacheRead":13312,"inputCacheCreation":100},"usageScope":"turn","time":1780631837221}
{"type":"usage.record","model":"kimi-code/kimi-for-coding","usage":{"inputOther":100,"output":200,"inputCacheRead":300,"inputCacheCreation":0},"usageScope":"session","time":1780631840000}"#;
        let file = create_test_file(content);

        let messages = parse_kimi_file(file.path());

        assert_eq!(messages.len(), 1); // Only usageScope "turn" is parsed
        assert_eq!(messages[0].client, "kimi");
        assert_eq!(messages[0].model_id, "kimi-code/kimi-for-coding");
        assert_eq!(messages[0].provider_id, "moonshotai");
        assert_eq!(messages[0].tokens.input, 3032);
        assert_eq!(messages[0].tokens.output, 121);
        assert_eq!(messages[0].tokens.cache_read, 13312);
        assert_eq!(messages[0].tokens.cache_write, 100);
        assert_eq!(messages[0].timestamp, 1780631837221);
    }

    #[test]
    fn test_parse_kimi_code_uses_configured_model_toml() {
        let dir = tempfile::TempDir::new().unwrap();
        let kimi_dir = dir.path().join(".kimi-code");
        let session_dir = kimi_dir.join("sessions/group-1/session-1/agents/main");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            kimi_dir.join("config.toml"),
            "default_model = \"custom-model-abc\"",
        )
        .unwrap();
        let wire_path = session_dir.join("wire.jsonl");
        std::fs::write(
            &wire_path,
            r#"{"type":"usage.record","usage":{"inputOther":100,"output":50,"inputCacheRead":0,"inputCacheCreation":0},"usageScope":"turn","time":1780631837221}"#,
        )
        .unwrap();

        let messages = parse_kimi_file(&wire_path);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].model_id, "custom-model-abc");
    }

    #[test]
    fn test_parse_kimi_skip_non_status_update() {
        let content = r#"{"type": "metadata", "protocol_version": "1.3"}
{"timestamp": 1770983400.0, "message": {"type": "TurnBegin", "payload": {"user_input": "hello"}}}
{"timestamp": 1770983410.0, "message": {"type": "ContentPart", "payload": {"type": "text", "text": "response"}}}
{"timestamp": 1770983420.0, "message": {"type": "ToolCall", "payload": {"type": "function", "id": "tool_1", "function": {"name": "ReadFile", "arguments": "{}"}}}}"#;
        let file = create_test_file(content);

        let messages = parse_kimi_file(file.path());

        assert!(messages.is_empty());
    }

    #[test]
    fn test_parse_kimi_empty_file() {
        let file = create_test_file("");

        let messages = parse_kimi_file(file.path());

        assert!(messages.is_empty());
    }

    #[test]
    fn test_parse_kimi_tool_call_multi_step() {
        // Simulates a tool-call scenario with multiple StatusUpdate messages in one turn
        let content = r#"{"type": "metadata", "protocol_version": "1.3"}
{"timestamp": 1770983400.0, "message": {"type": "TurnBegin", "payload": {"user_input": "read file"}}}
{"timestamp": 1770983405.0, "message": {"type": "StepBegin", "payload": {"n": 1}}}
{"timestamp": 1770983410.0, "message": {"type": "StatusUpdate", "payload": {"token_usage": {"input_other": 500, "output": 100, "input_cache_read": 200, "input_cache_creation": 0}, "message_id": "msg-step1"}}}
{"timestamp": 1770983415.0, "message": {"type": "ToolCall", "payload": {"type": "function", "id": "tool_1", "function": {"name": "ReadFile", "arguments": "{}"}}}}
{"timestamp": 1770983420.0, "message": {"type": "StepBegin", "payload": {"n": 2}}}
{"timestamp": 1770983425.0, "message": {"type": "StatusUpdate", "payload": {"token_usage": {"input_other": 800, "output": 300, "input_cache_read": 400, "input_cache_creation": 100}, "message_id": "msg-step2"}}}"#;
        let file = create_test_file(content);

        let messages = parse_kimi_file(file.path());

        assert_eq!(messages.len(), 2);
        // Step 1
        assert_eq!(messages[0].tokens.input, 500);
        assert_eq!(messages[0].tokens.output, 100);
        assert_eq!(messages[0].tokens.cache_read, 200);
        assert_eq!(messages[0].tokens.cache_write, 0);
        // Step 2
        assert_eq!(messages[1].tokens.input, 800);
        assert_eq!(messages[1].tokens.output, 300);
        assert_eq!(messages[1].tokens.cache_read, 400);
        assert_eq!(messages[1].tokens.cache_write, 100);
    }

    #[test]
    fn test_parse_kimi_with_cache_tokens() {
        let content = r#"{"type": "metadata", "protocol_version": "1.3"}
{"timestamp": 1771123711.615454, "message": {"type": "StatusUpdate", "payload": {"context_usage": 0.024, "token_usage": {"input_other": 1508, "output": 205, "input_cache_read": 4864, "input_cache_creation": 0}, "message_id": "chatcmpl-2tNw2mhUNfdPMP0Jyie7gDhD"}}}"#;
        let file = create_test_file(content);

        let messages = parse_kimi_file(file.path());

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].tokens.input, 1508);
        assert_eq!(messages[0].tokens.output, 205);
        assert_eq!(messages[0].tokens.cache_read, 4864);
        assert_eq!(messages[0].tokens.cache_write, 0);
    }

    #[test]
    fn test_parse_kimi_skips_zero_token_entries() {
        let content = r#"{"type": "metadata", "protocol_version": "1.3"}
{"timestamp": 1770983410.0, "message": {"type": "StatusUpdate", "payload": {"token_usage": {"input_other": 0, "output": 0, "input_cache_read": 0, "input_cache_creation": 0}, "message_id": "msg-empty"}}}"#;
        let file = create_test_file(content);

        let messages = parse_kimi_file(file.path());

        assert!(messages.is_empty());
    }

    #[test]
    fn test_parse_kimi_malformed_lines() {
        let content = r#"{"type": "metadata", "protocol_version": "1.3"}
not valid json at all
{"timestamp": 1770983410.0, "message": {"type": "StatusUpdate", "payload": {"token_usage": {"input_other": 100, "output": 200, "input_cache_read": 0, "input_cache_creation": 0}, "message_id": "msg-1"}}}"#;
        let file = create_test_file(content);

        let messages = parse_kimi_file(file.path());

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].tokens.input, 100);
    }
}

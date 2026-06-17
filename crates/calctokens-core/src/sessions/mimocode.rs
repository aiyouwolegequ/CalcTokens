use super::utils::open_readonly_sqlite;
use super::UnifiedMessage;
use crate::{provider_identity, TokenBreakdown};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct MimoMessage {
    #[serde(default)]
    pub id: Option<String>,
    pub session_id: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    pub role: String,
    #[serde(rename = "modelID", default)]
    pub model_id: Option<String>,
    #[serde(rename = "providerID", default)]
    pub provider_id: Option<String>,
    pub cost: Option<f64>,
    pub tokens: Option<MimoTokens>,
    pub time: Option<MimoTime>,
    pub agent: Option<String>,
    pub mode: Option<String>,
    #[serde(skip)]
    pub time_created: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MimoTokens {
    pub input: i64,
    pub output: i64,
    #[serde(default)]
    pub reasoning: Option<i64>,
    pub cache: MimoCache,
}

#[derive(Debug, Deserialize)]
pub struct MimoCache {
    pub read: i64,
    pub write: i64,
}

#[derive(Debug, Deserialize)]
pub struct MimoTime {
    pub created: f64,
    pub completed: Option<f64>,
}

fn is_mimo_model(model_id: &str) -> bool {
    model_id.to_lowercase().starts_with("mimo-")
}

pub fn parse_mimocode_sqlite(db_path: &Path) -> Vec<UnifiedMessage> {
    let Some(conn) = open_readonly_sqlite(db_path) else {
        return Vec::new();
    };

    let query = r#"
        SELECT m.id, m.session_id, m.agent_id, m.time_created, m.data,
               p.worktree,
               CASE
                   WHEN ci.session_id IS NOT NULL THEN 'claude_import'
                   WHEN ei.session_id IS NOT NULL THEN COALESCE(ei.source, 'external_import')
                   ELSE 'native'
               END AS source_kind
        FROM message m
        JOIN session s ON m.session_id = s.id
        JOIN project p ON s.project_id = p.id
        LEFT JOIN claude_import ci ON s.id = ci.session_id
        LEFT JOIN external_import ei ON s.id = ei.session_id
        WHERE m.data LIKE '%"role"%"assistant"%'
          AND m.data LIKE '%"tokens"%'
          AND json_valid(m.data)
          AND json_extract(m.data, '$.role') = 'assistant'
          AND json_extract(m.data, '$.tokens') IS NOT NULL
    "#;

    let mut stmt = match conn.prepare(query) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let rows = match stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let session_id: String = row.get(1)?;
        let agent_id: Option<String> = row.get(2)?;
        let time_created: Option<i64> = row.get(3)?;
        let data_json: String = row.get(4)?;
        let worktree: Option<String> = row.get(5)?;
        let source_kind: String = row.get(6)?;
        Ok((
            id,
            session_id,
            agent_id,
            time_created,
            data_json,
            worktree,
            source_kind,
        ))
    }) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut messages = Vec::new();

    for row_result in rows {
        let (row_id, row_session_id, row_agent_id, time_created, data_json, worktree, source_kind) =
            match row_result {
                Ok(r) => r,
                Err(_) => continue,
            };

        let mut bytes = data_json.into_bytes();
        let mut msg: MimoMessage = match simd_json::from_slice(&mut bytes) {
            Ok(m) => m,
            Err(_) => continue,
        };
        msg.time_created = time_created;

        if msg.role != "assistant" {
            continue;
        }

        let tokens = match msg.tokens {
            Some(t) => t,
            None => continue,
        };

        let model_id = match msg.model_id {
            Some(m) => m,
            None => continue,
        };

        // MiMo Code can import sessions from other clients (e.g. Claude Code).
        // Attribute imported sessions to their original client; keep only native
        // MiMo model usage under the mimocode client.
        let client = match source_kind.as_str() {
            "claude_import" | "cc" => "claude",
            _ => {
                if !is_mimo_model(&model_id) {
                    continue;
                }
                "mimocode"
            }
        };

        let dedup_key = msg.id.or(Some(row_id));

        let agent = msg.agent.or(msg.mode).or(row_agent_id);

        let session_id = msg.session_id.unwrap_or(row_session_id);
        let timestamp = msg
            .time
            .map(|t| t.created as i64)
            .unwrap_or_else(|| msg.time_created.unwrap_or(0));

        let provider = provider_identity::inferred_provider_from_model(&model_id)
            .unwrap_or(client)
            .to_string();

        let mut unified = UnifiedMessage::new_with_agent(
            client,
            model_id,
            provider,
            session_id,
            timestamp,
            TokenBreakdown {
                input: tokens.input.max(0),
                output: tokens.output.max(0),
                cache_read: tokens.cache.read.max(0),
                cache_write: tokens.cache.write.max(0),
                reasoning: tokens.reasoning.unwrap_or(0).max(0),
            },
            msg.cost.unwrap_or(0.0).max(0.0),
            agent,
        );
        unified.dedup_key = dedup_key;

        if let Some(wt) = worktree {
            let key = super::normalize_workspace_key(&wt);
            let label = key.as_deref().and_then(super::workspace_label_from_key);
            unified.set_workspace(key, label);
        }

        messages.push(unified);
    }

    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};
    use tempfile::TempDir;

    fn create_mimocode_sqlite_db(dir: &TempDir) -> std::path::PathBuf {
        let db_path = dir.path().join("mimocode.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE project (
                id TEXT PRIMARY KEY,
                worktree TEXT NOT NULL
            );
            CREATE TABLE session (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                title TEXT NOT NULL DEFAULT '',
                directory TEXT NOT NULL DEFAULT '',
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL
            );
            CREATE TABLE message (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                agent_id TEXT NOT NULL DEFAULT 'main',
                time_created INTEGER NOT NULL DEFAULT 0,
                time_updated INTEGER NOT NULL DEFAULT 0,
                data TEXT NOT NULL
            );
            CREATE TABLE claude_import (
                source_uuid TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                source_path TEXT NOT NULL,
                source_mtime INTEGER NOT NULL,
                time_imported INTEGER NOT NULL,
                message_ids TEXT
            );
            CREATE TABLE external_import (
                source TEXT NOT NULL,
                source_key TEXT NOT NULL,
                session_id TEXT NOT NULL,
                source_path TEXT NOT NULL,
                source_mtime INTEGER NOT NULL,
                time_imported INTEGER NOT NULL,
                message_ids TEXT,
                PRIMARY KEY (source, source_key)
            );
            "#,
        )
        .unwrap();

        conn.execute(
            "INSERT INTO project (id, worktree) VALUES (?1, ?2)",
            params!["proj-1", "/Users/test/project"],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO session (id, project_id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params!["sess-1", "proj-1", "Test Session", "/Users/test/project", 1700000000000i64, 1700000000000i64],
        )
        .unwrap();

        db_path
    }

    fn insert_mimo_message(
        conn: &Connection,
        row_id: &str,
        session_id: &str,
        agent_id: &str,
        data_json: &str,
    ) {
        conn.execute(
            "INSERT INTO message (id, session_id, agent_id, time_created, time_updated, data) VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
            params![row_id, session_id, agent_id, 1700000000000i64, data_json],
        )
        .unwrap();
    }

    #[test]
    fn test_parse_mimocode_message_structure() {
        let json = r#"{
            "id": "msg-123",
            "role": "assistant",
            "modelID": "mimo-auto",
            "providerID": "mimo",
            "cost": 0.0,
            "tokens": {
                "input": 1000,
                "output": 200,
                "reasoning": 50,
                "cache": {"read": 500, "write": 100}
            },
            "time": {"created": 1700000000000}
        }"#;

        let mut bytes = json.as_bytes().to_vec();
        let msg: MimoMessage = simd_json::from_slice(&mut bytes).unwrap();
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.model_id, Some("mimo-auto".to_string()));
        assert_eq!(msg.provider_id, Some("mimo".to_string()));
        assert_eq!(msg.tokens.unwrap().input, 1000);
    }

    #[test]
    fn test_parse_mimocode_sqlite_reads_assistant_rows() {
        let dir = TempDir::new().unwrap();
        let db_path = create_mimocode_sqlite_db(&dir);
        let conn = Connection::open(&db_path).unwrap();

        let data_json = r#"{
            "id": "embedded-msg-1",
            "role": "assistant",
            "modelID": "mimo-auto",
            "providerID": "mimo",
            "cost": 0.0,
            "agent": "explore",
            "tokens": {
                "input": 1200,
                "output": 300,
                "reasoning": 40,
                "cache": {"read": 75, "write": 25}
            },
            "time": {"created": 1700000000123.0}
        }"#;
        insert_mimo_message(&conn, "row-msg-1", "sess-1", "main", data_json);
        drop(conn);

        let messages = parse_mimocode_sqlite(&db_path);
        assert_eq!(messages.len(), 1);

        let msg = &messages[0];
        assert_eq!(msg.client, "mimocode");
        assert_eq!(msg.session_id, "sess-1");
        assert_eq!(msg.model_id, "mimo-auto");
        assert_eq!(msg.provider_id, "mimo");
        assert_eq!(msg.timestamp, 1_700_000_000_123);
        assert_eq!(msg.tokens.input, 1200);
        assert_eq!(msg.tokens.output, 300);
        assert_eq!(msg.tokens.reasoning, 40);
        assert_eq!(msg.tokens.cache_read, 75);
        assert_eq!(msg.tokens.cache_write, 25);
        assert_eq!(msg.cost, 0.0);
        assert_eq!(msg.agent.as_deref(), Some("explore"));
        assert_eq!(msg.dedup_key.as_deref(), Some("embedded-msg-1"));
    }

    #[test]
    fn test_parse_mimocode_sqlite_skips_invalid_rows_and_clamps_values() {
        let dir = TempDir::new().unwrap();
        let db_path = create_mimocode_sqlite_db(&dir);
        let conn = Connection::open(&db_path).unwrap();

        // Insert test sessions
        for session_id in &["sess-user", "sess-no-tokens", "sess-invalid", "sess-valid"] {
            conn.execute(
                "INSERT INTO session (id, project_id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![session_id, "proj-1", "Test Session", "/Users/test/project", 1700000000000i64, 1700000000000i64],
            )
            .unwrap();
        }

        insert_mimo_message(
            &conn,
            "row-user",
            "sess-user",
            "main",
            r#"{
                "role": "user",
                "modelID": "mimo-auto",
                "tokens": {"input": 1, "output": 1, "cache": {"read": 0, "write": 0}}
            }"#,
        );
        insert_mimo_message(
            &conn,
            "row-no-tokens",
            "sess-no-tokens",
            "main",
            r#"{
                "role": "assistant",
                "modelID": "mimo-auto"
            }"#,
        );
        insert_mimo_message(
            &conn,
            "row-invalid-json",
            "sess-invalid",
            "main",
            "{not-json",
        );
        insert_mimo_message(
            &conn,
            "row-valid",
            "sess-valid",
            "main",
            r#"{
                "role": "assistant",
                "modelID": "mimo-auto",
                "cost": -0.75,
                "mode": "plan",
                "tokens": {
                    "input": -100,
                    "output": -50,
                    "reasoning": -5,
                    "cache": {"read": -20, "write": -10}
                }
            }"#,
        );
        drop(conn);

        let messages = parse_mimocode_sqlite(&db_path);
        assert_eq!(messages.len(), 1);

        let msg = &messages[0];
        assert_eq!(msg.session_id, "sess-valid");
        assert_eq!(msg.model_id, "mimo-auto");
        assert_eq!(msg.provider_id, "mimo");
        assert_eq!(msg.tokens.input, 0);
        assert_eq!(msg.tokens.output, 0);
        assert_eq!(msg.tokens.reasoning, 0);
        assert_eq!(msg.tokens.cache_read, 0);
        assert_eq!(msg.tokens.cache_write, 0);
        assert_eq!(msg.cost, 0.0);
        assert_eq!(msg.agent.as_deref(), Some("plan"));
        assert_eq!(msg.dedup_key.as_deref(), Some("row-valid"));
    }

    #[test]
    fn test_parse_mimocode_sqlite_skips_non_mimo_models() {
        let dir = TempDir::new().unwrap();
        let db_path = create_mimocode_sqlite_db(&dir);
        let conn = Connection::open(&db_path).unwrap();

        insert_mimo_message(
            &conn,
            "row-mimo",
            "sess-1",
            "main",
            r#"{
                "role": "assistant",
                "modelID": "mimo-auto",
                "providerID": "mimo",
                "tokens": {"input": 100, "output": 50, "cache": {"read": 0, "write": 0}},
                "time": {"created": 1700000000000.0}
            }"#,
        );
        insert_mimo_message(
            &conn,
            "row-kimi",
            "sess-1",
            "main",
            r#"{
                "role": "assistant",
                "modelID": "kimi-k2.6",
                "providerID": "anthropic",
                "tokens": {"input": 9999, "output": 9999, "cache": {"read": 0, "write": 0}},
                "time": {"created": 1700000000000.0}
            }"#,
        );
        drop(conn);

        let messages = parse_mimocode_sqlite(&db_path);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].model_id, "mimo-auto");
    }

    #[test]
    fn test_parse_mimocode_sqlite_reattributes_imported_sessions_to_claude() {
        let dir = TempDir::new().unwrap();
        let db_path = create_mimocode_sqlite_db(&dir);
        let conn = Connection::open(&db_path).unwrap();

        conn.execute(
            "INSERT INTO session (id, project_id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params!["sess-native", "proj-1", "Native Session", "/Users/test/project", 1700000000000i64, 1700000000000i64],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO session (id, project_id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params!["sess-imported", "proj-1", "Imported Session", "/Users/test/project", 1700000000000i64, 1700000000000i64],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO claude_import (source_uuid, session_id, source_path, source_mtime, time_imported, message_ids) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params!["uuid-1", "sess-imported", "/path/to/claude.jsonl", 1700000000000i64, 1700000000000i64, "[]"],
        )
        .unwrap();

        let native_json = r#"{
            "role": "assistant",
            "modelID": "mimo-auto",
            "providerID": "mimo",
            "tokens": {"input": 100, "output": 50, "cache": {"read": 0, "write": 0}},
            "time": {"created": 1700000000000.0}
        }"#;
        let imported_json = r#"{
            "role": "assistant",
            "modelID": "claude-sonnet-4-5",
            "providerID": "anthropic",
            "tokens": {"input": 9999, "output": 9999, "cache": {"read": 0, "write": 0}},
            "time": {"created": 1700000000000.0}
        }"#;
        insert_mimo_message(&conn, "row-native", "sess-native", "main", native_json);
        insert_mimo_message(
            &conn,
            "row-imported",
            "sess-imported",
            "main",
            imported_json,
        );
        drop(conn);

        let messages = parse_mimocode_sqlite(&db_path);
        assert_eq!(messages.len(), 2);

        let native = messages
            .iter()
            .find(|m| m.session_id == "sess-native")
            .unwrap();
        assert_eq!(native.client, "mimocode");
        assert_eq!(native.model_id, "mimo-auto");
        assert_eq!(native.provider_id, "mimo");

        let imported = messages
            .iter()
            .find(|m| m.session_id == "sess-imported")
            .unwrap();
        assert_eq!(imported.client, "claude");
        assert_eq!(imported.model_id, "claude-sonnet-4-5");
        assert_eq!(imported.provider_id, "anthropic");
    }

    #[test]
    fn test_parse_mimocode_sqlite_reattributes_external_cc_imports_to_claude() {
        let dir = TempDir::new().unwrap();
        let db_path = create_mimocode_sqlite_db(&dir);
        let conn = Connection::open(&db_path).unwrap();

        conn.execute(
            "INSERT INTO session (id, project_id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params!["sess-cc", "proj-1", "CC Imported Session", "/Users/test/project", 1700000000000i64, 1700000000000i64],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO external_import (source, source_key, session_id, source_path, source_mtime, time_imported, message_ids) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params!["cc", "key-1", "sess-cc", "/path/to/cc.jsonl", 1700000000000i64, 1700000000000i64, "[]"],
        )
        .unwrap();

        let imported_json = r#"{
            "role": "assistant",
            "modelID": "kimi-k2.5",
            "providerID": "anthropic",
            "tokens": {"input": 500, "output": 100, "cache": {"read": 0, "write": 0}},
            "time": {"created": 1700000000000.0}
        }"#;
        insert_mimo_message(&conn, "row-cc", "sess-cc", "main", imported_json);
        drop(conn);

        let messages = parse_mimocode_sqlite(&db_path);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].client, "claude");
        assert_eq!(messages[0].model_id, "kimi-k2.5");
        assert_eq!(messages[0].provider_id, "moonshotai");
    }

    #[test]
    fn test_parse_mimocode_sqlite_returns_empty_for_missing_db() {
        let messages = parse_mimocode_sqlite(std::path::Path::new("/nonexistent/mimocode.db"));
        assert!(messages.is_empty());
    }

    #[test]
    fn test_parse_mimocode_sqlite_extracts_workspace_from_project() {
        let dir = TempDir::new().unwrap();
        let db_path = create_mimocode_sqlite_db(&dir);
        let conn = Connection::open(&db_path).unwrap();

        let data_json = r#"{
            "role": "assistant",
            "modelID": "mimo-auto",
            "providerID": "mimo",
            "tokens": {"input": 100, "output": 50, "cache": {"read": 0, "write": 0}},
            "time": {"created": 1700000000000.0}
        }"#;
        insert_mimo_message(&conn, "row-ws", "sess-1", "main", data_json);
        drop(conn);

        let messages = parse_mimocode_sqlite(&db_path);
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].workspace_key.as_deref(),
            Some("/Users/test/project")
        );
        assert_eq!(messages[0].workspace_label.as_deref(), Some("project"));
    }

    #[test]
    fn test_parse_mimocode_sqlite_uses_agent_id_as_fallback() {
        let dir = TempDir::new().unwrap();
        let db_path = create_mimocode_sqlite_db(&dir);
        let conn = Connection::open(&db_path).unwrap();

        let data_json = r#"{
            "role": "assistant",
            "modelID": "mimo-auto",
            "tokens": {"input": 100, "output": 50, "cache": {"read": 0, "write": 0}},
            "time": {"created": 1700000000000.0}
        }"#;
        insert_mimo_message(&conn, "row-agent", "sess-1", "plan", data_json);
        drop(conn);

        let messages = parse_mimocode_sqlite(&db_path);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].agent.as_deref(), Some("plan"));
    }
}

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

pub fn parse_mimocode_sqlite(db_path: &Path) -> Vec<UnifiedMessage> {
    let Some(conn) = open_readonly_sqlite(db_path) else {
        return Vec::new();
    };

    let query = r#"
        SELECT m.id, m.session_id, m.agent_id, m.data,
               p.worktree
        FROM message m
        JOIN session s ON m.session_id = s.id
        JOIN project p ON s.project_id = p.id
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
        let data_json: String = row.get(3)?;
        let worktree: Option<String> = row.get(4)?;
        Ok((id, session_id, agent_id, data_json, worktree))
    }) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut messages = Vec::new();

    for row_result in rows {
        let (row_id, row_session_id, row_agent_id, data_json, worktree) = match row_result {
            Ok(r) => r,
            Err(_) => continue,
        };

        let mut bytes = data_json.into_bytes();
        let msg: MimoMessage = match simd_json::from_slice(&mut bytes) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if msg.role != "assistant" {
            continue;
        }

        let tokens = match msg.tokens {
            Some(t) => t,
            None => continue,
        };

        let dedup_key = msg.id.or(Some(row_id));

        let model_id = match msg.model_id {
            Some(m) => m,
            None => continue,
        };

        let agent = msg.agent.or(msg.mode).or(row_agent_id);

        let session_id = msg.session_id.unwrap_or(row_session_id);
        let timestamp = msg.time.map(|t| t.created as i64).unwrap_or(0);

        let provider = msg
            .provider_id
            .as_deref()
            .or_else(|| provider_identity::inferred_provider_from_model(&model_id))
            .unwrap_or("mimocode")
            .to_string();

        let mut unified = UnifiedMessage::new_with_agent(
            "mimocode",
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
                data TEXT NOT NULL
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
            "INSERT INTO message (id, session_id, agent_id, data) VALUES (?1, ?2, ?3, ?4)",
            params![row_id, session_id, agent_id, data_json],
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

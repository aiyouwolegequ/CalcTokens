//! Shared parsing helpers for session logs.

use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::io::{self, BufRead};
use std::path::Path;
use std::time::SystemTime;

pub(crate) const MAX_SESSION_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_SESSION_LINE_BYTES: usize = 8 * 1024 * 1024;

pub(crate) fn extract_i64(value: Option<&Value>) -> Option<i64> {
    value.and_then(|val| {
        val.as_i64()
            .or_else(|| val.as_u64().map(|v| v as i64))
            .or_else(|| val.as_str().and_then(|s| s.parse::<i64>().ok()))
    })
}

pub(crate) fn extract_string(value: Option<&Value>) -> Option<String> {
    value.and_then(|val| val.as_str().map(|s| s.to_string()))
}

pub(crate) fn parse_timestamp_value(value: &Value) -> Option<i64> {
    if let Some(ts) = value.as_str() {
        return parse_timestamp_str(ts);
    }

    let numeric = value
        .as_i64()
        .or_else(|| value.as_u64().map(|v| v as i64))?;
    if numeric <= 0 {
        return None;
    }
    if numeric >= 1_000_000_000_000 {
        Some(numeric)
    } else {
        Some(numeric * 1000)
    }
}

pub(crate) fn parse_timestamp_str(value: &str) -> Option<i64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(value) {
        return Some(dt.timestamp_millis());
    }

    if let Ok(numeric) = value.parse::<i64>() {
        if numeric <= 0 {
            return None;
        }
        if numeric >= 1_000_000_000_000 {
            return Some(numeric);
        }
        return Some(numeric * 1000);
    }

    None
}

pub(crate) fn file_modified_timestamp_ms(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis())
}

/// Open a SQLite file for read-only access with no mutex (single-threaded parser use).
/// Returns `None` if the file cannot be opened — the caller treats that as "no sessions".
pub(crate) fn open_readonly_sqlite(path: &Path) -> Option<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()
}

/// Read a file into bytes, returning `None` on any I/O error instead of propagating.
/// Used by parsers that treat missing/unreadable session files as "no data".
pub(crate) fn read_file_or_none(path: &Path) -> Option<Vec<u8>> {
    if !file_within_size_limit(path) {
        return None;
    }
    std::fs::read(path).ok()
}

pub(crate) fn read_text_file_or_none(path: &Path) -> Option<String> {
    if !file_within_size_limit(path) {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

pub(crate) fn file_within_size_limit(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.len() <= MAX_SESSION_FILE_BYTES)
        .unwrap_or(false)
}

pub(crate) fn read_line_bytes_limited<R: BufRead>(
    reader: &mut R,
    buffer: &mut Vec<u8>,
) -> io::Result<usize> {
    buffer.clear();
    let mut total = 0usize;

    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(total);
        }

        let newline_index = available.iter().position(|byte| *byte == b'\n');
        let take = newline_index
            .map(|index| index + 1)
            .unwrap_or(available.len());
        if total.saturating_add(take) > MAX_SESSION_LINE_BYTES {
            let remaining = MAX_SESSION_LINE_BYTES.saturating_sub(total);
            if remaining > 0 {
                buffer.extend_from_slice(&available[..remaining]);
            }
            reader.consume(take);
            if newline_index.is_none() {
                discard_until_newline(reader)?;
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "session log line exceeds size limit",
            ));
        }

        buffer.extend_from_slice(&available[..take]);
        reader.consume(take);
        total += take;

        if newline_index.is_some() {
            return Ok(total);
        }
    }
}

fn discard_until_newline<R: BufRead>(reader: &mut R) -> io::Result<()> {
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(());
        }
        let newline_index = available.iter().position(|byte| *byte == b'\n');
        let take = newline_index
            .map(|index| index + 1)
            .unwrap_or(available.len());
        reader.consume(take);
        if newline_index.is_some() {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_timestamp_value_rejects_zero_and_negative_numbers() {
        assert!(parse_timestamp_value(&serde_json::json!(0)).is_none());
        assert!(parse_timestamp_value(&serde_json::json!(-1000)).is_none());
        assert!(parse_timestamp_value(&serde_json::json!(-1_700_000_000_000_i64)).is_none());
    }

    #[test]
    fn parse_timestamp_value_accepts_positive_numbers() {
        assert_eq!(
            parse_timestamp_value(&serde_json::json!(1_700_000_000_000_i64)),
            Some(1_700_000_000_000)
        );
        assert_eq!(
            parse_timestamp_value(&serde_json::json!(1_700_000_000_i64)),
            Some(1_700_000_000_000)
        );
    }

    #[test]
    fn parse_timestamp_str_rejects_zero_and_negative_strings() {
        assert!(parse_timestamp_str("0").is_none());
        assert!(parse_timestamp_str("-5").is_none());
    }

    #[test]
    fn read_file_or_none_rejects_oversized_file() {
        let file = tempfile::NamedTempFile::new().unwrap();
        file.as_file().set_len(MAX_SESSION_FILE_BYTES + 1).unwrap();

        assert!(read_file_or_none(file.path()).is_none());
    }

    #[test]
    fn read_line_bytes_limited_rejects_oversized_line() {
        let input = vec![b'a'; MAX_SESSION_LINE_BYTES + 1];
        let mut reader = std::io::Cursor::new(input);
        let mut buffer = Vec::new();

        assert!(read_line_bytes_limited(&mut reader, &mut buffer).is_err());
    }

    #[test]
    fn read_line_bytes_limited_accepts_bounded_line() {
        let mut reader = std::io::Cursor::new(b"{\"ok\":true}\n".to_vec());
        let mut buffer = Vec::new();

        let bytes = read_line_bytes_limited(&mut reader, &mut buffer).unwrap();

        assert_eq!(bytes, b"{\"ok\":true}\n".len());
        assert_eq!(buffer, b"{\"ok\":true}\n");
    }
}

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

const AUDIT_LOG_READ_CHUNK_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuditEventRecord {
    pub timestamp_utc_epoch_ms: u64,
    pub process_id: u32,
    pub session_id: Uuid,
    pub event: String,
    pub result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditEventSummary {
    pub total_events_considered: usize,
    pub counts_by_event: Value,
    pub counts_by_result: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newest_timestamp_utc_epoch_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_timestamp_utc_epoch_ms: Option<u64>,
}

pub fn default_mcp_audit_path(vault_path: &Path) -> PathBuf {
    vault_path.with_file_name("mcp-audit.jsonl")
}

pub fn append_jsonl_entry<T>(
    path: &Path,
    entry: &T,
    max_bytes_before_compaction: u64,
    retained_event_count: usize,
) -> Result<(), String>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("No se pudo crear el directorio de auditoría: {error}"))?;
    }

    let line = serde_json::to_string(entry)
        .map_err(|error| format!("No se pudo serializar la auditoría local: {error}"))?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("No se pudo abrir el log local de auditoría: {error}"))?;

    file.write_all(line.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.flush())
        .map_err(|error| format!("No se pudo escribir el log local de auditoría: {error}"))?;
    file.sync_data()
        .map_err(|error| format!("No se pudo sincronizar el log local de auditoría: {error}"))?;

    compact_if_needed(path, max_bytes_before_compaction, retained_event_count)
}

pub fn read_recent_events(path: &Path, limit: usize) -> Result<Vec<AuditEventRecord>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut events = read_recent_lines(path, limit)?
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<AuditEventRecord>(&line)
                .map_err(|error| format!("No se pudo interpretar una línea del audit log: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    events.sort_by_key(|entry| std::cmp::Reverse(entry.timestamp_utc_epoch_ms));
    events.truncate(limit);
    Ok(events)
}

pub fn summarize_recent_events(path: &Path, limit: usize) -> Result<AuditEventSummary, String> {
    let events = read_recent_events(path, limit)?;
    let mut counts_by_event = serde_json::Map::new();
    let mut counts_by_result = serde_json::Map::new();

    for event in &events {
        increment_count(&mut counts_by_event, &event.event);
        increment_count(&mut counts_by_result, &event.result);
    }

    Ok(AuditEventSummary {
        total_events_considered: events.len(),
        counts_by_event: Value::Object(counts_by_event),
        counts_by_result: Value::Object(counts_by_result),
        newest_timestamp_utc_epoch_ms: events.first().map(|event| event.timestamp_utc_epoch_ms),
        oldest_timestamp_utc_epoch_ms: events.last().map(|event| event.timestamp_utc_epoch_ms),
    })
}

fn increment_count(counts: &mut serde_json::Map<String, Value>, key: &str) {
    let next = counts
        .get(key)
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(1);
    counts.insert(key.to_owned(), Value::from(next));
}

fn read_recent_lines(path: &Path, limit: usize) -> Result<Vec<String>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let mut file = File::open(path)
        .map_err(|error| format!("No se pudo abrir el log local de auditoría: {error}"))?;
    let mut offset = file
        .metadata()
        .map_err(|error| format!("No se pudo inspeccionar el log local de auditoría: {error}"))?
        .len();
    let mut chunks = Vec::new();
    let mut newline_count = 0usize;

    while offset > 0 && newline_count <= limit {
        let read_len = AUDIT_LOG_READ_CHUNK_BYTES.min(offset as usize);
        offset -= read_len as u64;
        file.seek(SeekFrom::Start(offset)).map_err(|error| {
            format!("No se pudo reposicionar el log local de auditoría: {error}")
        })?;

        let mut chunk = vec![0; read_len];
        file.read_exact(&mut chunk)
            .map_err(|error| format!("No se pudo leer el log local de auditoría: {error}"))?;
        newline_count += chunk.iter().filter(|byte| **byte == b'\n').count();
        chunks.push(chunk);
    }

    chunks.reverse();
    let total_len = chunks.iter().map(Vec::len).sum();
    let mut bytes = Vec::with_capacity(total_len);
    for chunk in chunks {
        bytes.extend_from_slice(&chunk);
    }

    let start = if offset > 0 {
        bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|index| index + 1)
            .unwrap_or(0)
    } else {
        0
    };
    let contents = String::from_utf8(bytes[start..].to_vec()).map_err(|error| {
        format!("El log local de auditoría contiene bytes UTF-8 inválidos: {error}")
    })?;
    let mut lines = contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();

    if lines.len() > limit {
        lines = lines.split_off(lines.len() - limit);
    }

    Ok(lines)
}

fn compact_if_needed(
    path: &Path,
    max_bytes_before_compaction: u64,
    retained_event_count: usize,
) -> Result<(), String> {
    if retained_event_count == 0 || !path.exists() {
        return Ok(());
    }

    let file_size = fs::metadata(path)
        .map_err(|error| format!("No se pudo inspeccionar el log local de auditoría: {error}"))?
        .len();
    if file_size <= max_bytes_before_compaction {
        return Ok(());
    }

    let retained_lines = read_recent_lines(path, retained_event_count)?;
    let retained_contents = if retained_lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", retained_lines.join("\n"))
    };

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|error| format!("No se pudo compactar el log local de auditoría: {error}"))?;
    file.write_all(retained_contents.as_bytes())
        .and_then(|_| file.flush())
        .map_err(|error| format!("No se pudo compactar el log local de auditoría: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("No se pudo compactar el log local de auditoría: {error}"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[derive(Serialize)]
    struct TestEntry {
        timestamp_utc_epoch_ms: u64,
        process_id: u32,
        session_id: Uuid,
        event: &'static str,
        result: &'static str,
    }

    fn entry(timestamp: u64, event: &'static str, result: &'static str) -> TestEntry {
        TestEntry {
            timestamp_utc_epoch_ms: timestamp,
            process_id: 1,
            session_id: Uuid::from_u128(timestamp as u128),
            event,
            result,
        }
    }

    #[test]
    fn append_jsonl_entry_writes_valid_json_line() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let path = temp_dir.path().join("mcp-audit.jsonl");

        append_jsonl_entry(
            &path,
            &entry(1, "generate_token", "delivered"),
            512 * 1024,
            2_048,
        )
        .expect("audit entry should persist");

        let contents = fs::read_to_string(path).expect("audit file should be readable");
        let line = contents.lines().next().expect("audit line should exist");
        let value: Value = serde_json::from_str(line).expect("audit line should be valid json");
        assert_eq!(value["event"], "generate_token");
    }

    #[test]
    fn read_recent_events_returns_newest_entries_first() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let path = temp_dir.path().join("mcp-audit.jsonl");
        fs::write(
            &path,
            concat!(
                "{\"timestamp_utc_epoch_ms\":1,\"process_id\":1,\"session_id\":\"00000000-0000-0000-0000-000000000001\",\"event\":\"older\",\"result\":\"ok\"}\n",
                "{\"timestamp_utc_epoch_ms\":2,\"process_id\":1,\"session_id\":\"00000000-0000-0000-0000-000000000002\",\"event\":\"newer\",\"result\":\"ok\"}\n"
            ),
        )
        .expect("audit file should be written");

        let events = read_recent_events(&path, 1).expect("events should load");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "newer");
    }

    #[test]
    fn read_recent_events_only_parses_the_requested_tail_window() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let path = temp_dir.path().join("mcp-audit.jsonl");
        fs::write(
            &path,
            concat!(
                "this is not json\n",
                "{\"timestamp_utc_epoch_ms\":3,\"process_id\":1,\"session_id\":\"00000000-0000-0000-0000-000000000003\",\"event\":\"recent\",\"result\":\"ok\"}\n"
            ),
        )
        .expect("audit file should be written");

        let events = read_recent_events(&path, 1).expect("tail window should load");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "recent");
    }

    #[test]
    fn summarize_recent_events_counts_events_and_results() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let path = temp_dir.path().join("mcp-audit.jsonl");
        fs::write(
            &path,
            concat!(
                "{\"timestamp_utc_epoch_ms\":1,\"process_id\":1,\"session_id\":\"00000000-0000-0000-0000-000000000001\",\"event\":\"generate_token\",\"result\":\"granted\"}\n",
                "{\"timestamp_utc_epoch_ms\":2,\"process_id\":1,\"session_id\":\"00000000-0000-0000-0000-000000000002\",\"event\":\"generate_token\",\"result\":\"delivered\"}\n",
                "{\"timestamp_utc_epoch_ms\":3,\"process_id\":1,\"session_id\":\"00000000-0000-0000-0000-000000000003\",\"event\":\"session_open\",\"result\":\"granted\"}\n"
            ),
        )
        .expect("audit file should be written");

        let summary = summarize_recent_events(&path, 10).expect("summary should be generated");

        assert_eq!(summary.total_events_considered, 3);
        assert_eq!(summary.counts_by_event["generate_token"], 2);
        assert_eq!(summary.counts_by_result["granted"], 2);
        assert_eq!(summary.newest_timestamp_utc_epoch_ms, Some(3));
        assert_eq!(summary.oldest_timestamp_utc_epoch_ms, Some(1));
    }

    #[test]
    fn compact_keeps_only_recent_entries_when_threshold_is_exceeded() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let path = temp_dir.path().join("mcp-audit.jsonl");
        fs::write(
            &path,
            concat!(
                "{\"timestamp_utc_epoch_ms\":1,\"process_id\":1,\"session_id\":\"00000000-0000-0000-0000-000000000001\",\"event\":\"one\",\"result\":\"ok\"}\n",
                "{\"timestamp_utc_epoch_ms\":2,\"process_id\":1,\"session_id\":\"00000000-0000-0000-0000-000000000002\",\"event\":\"two\",\"result\":\"ok\"}\n",
                "{\"timestamp_utc_epoch_ms\":3,\"process_id\":1,\"session_id\":\"00000000-0000-0000-0000-000000000003\",\"event\":\"three\",\"result\":\"ok\"}\n"
            ),
        )
        .expect("audit file should be written");

        compact_if_needed(&path, 32, 2).expect("compaction should succeed");

        let contents = fs::read_to_string(&path).expect("compacted audit file should exist");
        let lines = contents.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"event\":\"two\""));
        assert!(lines[1].contains("\"event\":\"three\""));
    }
}

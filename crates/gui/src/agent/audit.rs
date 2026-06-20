use std::{
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use mfa_forge_core::AccountPublic;

pub use mfa_forge_storage::audit_log::{AuditEventRecord, AuditEventSummary};

const AUDIT_LOG_MAX_BYTES_BEFORE_COMPACTION: u64 = 512 * 1024;
const AUDIT_LOG_RETAINED_EVENT_COUNT: usize = 2_048;

#[derive(Debug, Clone)]
pub struct AuditLogger {
    path: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct AuditEntry {
    timestamp_utc_epoch_ms: u64,
    process_id: u32,
    session_id: Uuid,
    event: &'static str,
    result: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    operation: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    account_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    service: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

impl AuditLogger {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn record(&self, entry: AuditEntry) -> Result<(), String> {
        mfa_forge_storage::audit_log::append_jsonl_entry(
            &self.path,
            &entry,
            AUDIT_LOG_MAX_BYTES_BEFORE_COMPACTION,
            AUDIT_LOG_RETAINED_EVENT_COUNT,
        )
    }
}

impl AuditEntry {
    pub fn new(session_id: Uuid, event: &'static str, result: &'static str) -> Self {
        Self {
            timestamp_utc_epoch_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or(0),
            process_id: process::id(),
            session_id,
            event,
            result,
            operation: None,
            account_id: None,
            service: None,
            user: None,
            details: None,
        }
    }

    pub fn with_operation(mut self, operation: &'static str) -> Self {
        self.operation = Some(operation);
        self
    }

    pub fn with_account(mut self, account: &AccountPublic) -> Self {
        self.account_id = Some(account.id);
        self.service = Some(account.service.clone());
        self.user = Some(account.user.clone());
        self
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}

pub fn default_mcp_audit_path(vault_path: &Path) -> PathBuf {
    mfa_forge_storage::audit_log::default_mcp_audit_path(vault_path)
}

pub fn read_recent_audit_events(
    path: &Path,
    limit: usize,
) -> Result<Vec<AuditEventRecord>, String> {
    mfa_forge_storage::audit_log::read_recent_events(path, limit)
}

pub fn summarize_recent_audit_events(
    path: &Path,
    limit: usize,
) -> Result<AuditEventSummary, String> {
    mfa_forge_storage::audit_log::summarize_recent_events(path, limit)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn audit_logger_appends_jsonl_entries() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let path = temp_dir.path().join("mcp-audit.jsonl");
        let logger = AuditLogger::new(path.clone());
        let session_id = Uuid::new_v4();

        logger
            .record(
                AuditEntry::new(session_id, "generate_token", "delivered")
                    .with_details(serde_json::json!({ "note": "no secrets here" })),
            )
            .expect("audit record should persist");

        let contents = fs::read_to_string(path).expect("audit file should be readable");
        let line = contents.lines().next().expect("audit line should exist");
        let value: Value = serde_json::from_str(line).expect("audit line should be valid json");
        assert_eq!(value["event"], "generate_token");
        assert!(value.get("token").is_none());
    }

    #[test]
    fn read_recent_audit_events_returns_newest_entries_first() {
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

        let events = read_recent_audit_events(&path, 1).expect("events should load");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "newer");
    }

    #[test]
    fn read_recent_audit_events_only_parses_the_requested_tail_window() {
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

        let events = read_recent_audit_events(&path, 1).expect("tail window should load");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "recent");
    }

    #[test]
    fn summarize_recent_audit_events_counts_events_and_results() {
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

        let summary =
            summarize_recent_audit_events(&path, 10).expect("summary should be generated");
        assert_eq!(summary.total_events_considered, 3);
        assert_eq!(summary.counts_by_event["generate_token"], 2);
        assert_eq!(summary.counts_by_result["granted"], 2);
        assert_eq!(summary.newest_timestamp_utc_epoch_ms, Some(3));
        assert_eq!(summary.oldest_timestamp_utc_epoch_ms, Some(1));
    }
}

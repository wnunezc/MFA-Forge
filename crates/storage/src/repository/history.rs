use mfa_forge_core::{AccountHistoryEvent, AccountRecord};

use crate::types::AccountHistoryEntry;

pub(super) fn capture_history(
    history: &mut Vec<AccountHistoryEntry>,
    event: AccountHistoryEvent,
    account: AccountRecord,
) {
    history.push(AccountHistoryEntry::new(
        event,
        account,
        unix_timestamp_now(),
    ));
    history.sort_by_key(|entry| std::cmp::Reverse(entry.captured_at));
}

pub(super) fn unix_timestamp_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

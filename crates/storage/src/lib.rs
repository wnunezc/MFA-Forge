/// Local application data path helpers for non-vault storage files.
pub mod app_data;
/// Sanitized JSONL audit-log persistence helpers.
pub mod audit_log;
mod crypto;
mod error;
/// Generic JSON preference persistence helpers.
pub mod preferences;
mod repository;
mod types;

pub use error::StorageError;
pub use repository::{VaultRepository, default_vault_path};

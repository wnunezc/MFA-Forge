use std::{io, path::PathBuf};

use thiserror::Error;
use uuid::Uuid;

use mfa_forge_core::CoreError;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("master password cannot be empty")]
    EmptyMasterPassword,
    #[error("could not determine a default vault path")]
    DefaultPathUnavailable,
    #[error("vault is already initialized at {0}")]
    VaultAlreadyExists(PathBuf),
    #[error("vault is not initialized at {0}")]
    VaultNotInitialized(PathBuf),
    #[error("failed to create the vault directory")]
    CreateDir(#[source] io::Error),
    #[error("failed to read the vault file")]
    ReadFile(#[source] io::Error),
    #[error("failed to remove a stale vault file")]
    RemoveFile(#[source] io::Error),
    #[error("failed to write the vault file")]
    WriteFile(#[source] io::Error),
    #[error("failed to rotate the previous vault into backup")]
    BackupVault(#[source] io::Error),
    #[error("failed to promote the temporary vault file")]
    PersistVault(#[source] io::Error),
    #[error("failed to restore the previous vault backup")]
    RestoreBackup(#[source] io::Error),
    #[error("failed to serialize the vault")]
    Serialize(#[source] serde_json::Error),
    #[error("failed to deserialize the vault")]
    Deserialize(#[source] serde_json::Error),
    #[error("vault backup is not available at {0}")]
    VaultBackupNotFound(PathBuf),
    #[error("account history entry {0} was not found")]
    HistoryEntryNotFound(Uuid),
    #[error("workspace {0} was not found")]
    DirectoryNotFound(String),
    #[error("workspace {0} is not empty")]
    DirectoryNotEmpty(String),
    #[error("vault uses an unsupported format version")]
    UnsupportedVersion,
    #[error("failed to derive the encryption key")]
    KeyDerivation,
    #[error("failed to initialize the encryption cipher")]
    CipherInit,
    #[error("vault password is invalid or the vault file is corrupted")]
    UnlockFailed,
    #[error("failed to encrypt the vault")]
    EncryptFailed,
    #[error("vault file is malformed")]
    MalformedEnvelope,
    #[error("vault contents are inconsistent")]
    InvariantViolation,
    #[error(transparent)]
    Core(#[from] CoreError),
}

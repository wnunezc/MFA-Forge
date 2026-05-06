use aes_gcm::{
    Aes256Gcm,
    aead::{AeadCore, OsRng},
};
use argon2::password_hash::SaltString;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use mfa_forge_core::{
    AccountHistoryEntryPublic, AccountHistoryEvent, AccountRecord, ProjectDirectory,
};

pub const VAULT_FORMAT_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEnvelope {
    pub version: u32,
    pub kdf: KdfParameters,
    pub nonce_b64: String,
    pub ciphertext_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParameters {
    pub salt: String,
    pub memory_cost_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl KdfParameters {
    pub fn generated() -> Self {
        Self {
            salt: SaltString::generate(&mut OsRng).to_string(),
            memory_cost_kib: 65_536,
            iterations: 3,
            parallelism: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountHistoryEntry {
    pub entry_id: Uuid,
    pub event: AccountHistoryEvent,
    pub captured_at: u64,
    pub account: AccountRecord,
}

impl AccountHistoryEntry {
    pub fn new(event: AccountHistoryEvent, account: AccountRecord, captured_at: u64) -> Self {
        Self {
            entry_id: Uuid::new_v4(),
            event,
            captured_at,
            account,
        }
    }

    pub fn public_view(&self) -> AccountHistoryEntryPublic {
        AccountHistoryEntryPublic {
            entry_id: self.entry_id,
            event: self.event,
            captured_at: self.captured_at,
            account: self.account.public_view(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultData {
    pub version: u32,
    #[serde(default)]
    pub accounts: Vec<AccountRecord>,
    #[serde(default)]
    pub history: Vec<AccountHistoryEntry>,
    #[serde(default)]
    pub directories: Vec<ProjectDirectory>,
}

impl Default for VaultData {
    fn default() -> Self {
        Self {
            version: VAULT_FORMAT_VERSION,
            accounts: Vec::new(),
            history: Vec::new(),
            directories: Vec::new(),
        }
    }
}

pub fn generate_nonce() -> [u8; 12] {
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    nonce.into()
}

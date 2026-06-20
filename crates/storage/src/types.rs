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
        #[cfg(any(test, feature = "fast-test-kdf"))]
        {
            return Self {
                salt: SaltString::generate(&mut OsRng).to_string(),
                memory_cost_kib: 1_024,
                iterations: 1,
                parallelism: 1,
            };
        }

        #[cfg(not(any(test, feature = "fast-test-kdf")))]
        {
            Self::production_strength()
        }
    }

    #[cfg(any(test, not(feature = "fast-test-kdf")))]
    pub fn production_strength() -> Self {
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

#[cfg(test)]
mod tests {
    use super::KdfParameters;

    #[test]
    fn generated_uses_fast_parameters_in_unit_tests() {
        let kdf = KdfParameters::generated();

        assert_eq!(kdf.memory_cost_kib, 1_024);
        assert_eq!(kdf.iterations, 1);
        assert_eq!(kdf.parallelism, 1);
    }

    #[test]
    fn production_strength_parameters_remain_available_for_crypto_coverage() {
        let kdf = KdfParameters::production_strength();

        assert_eq!(kdf.memory_cost_kib, 65_536);
        assert_eq!(kdf.iterations, 3);
        assert_eq!(kdf.parallelism, 1);
    }
}

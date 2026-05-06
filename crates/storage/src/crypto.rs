use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use secrecy::{ExposeSecret, SecretString};
use zeroize::Zeroize;

use mfa_forge_core::ProjectDirectory;

use crate::{
    StorageError,
    types::{KdfParameters, VAULT_FORMAT_VERSION, VaultData, VaultEnvelope, generate_nonce},
};

const KEY_LENGTH: usize = 32;

pub struct DecryptedVault {
    pub data: VaultData,
    pub migrated: bool,
}

pub fn encrypt_vault(
    vault: &VaultData,
    password: &SecretString,
) -> Result<VaultEnvelope, StorageError> {
    let kdf = KdfParameters::generated();
    let mut key = derive_key(password, &kdf)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| StorageError::CipherInit)?;

    let nonce = generate_nonce();
    let mut plaintext = serde_json::to_vec(vault).map_err(StorageError::Serialize)?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
        .map_err(|_| StorageError::EncryptFailed)?;

    plaintext.zeroize();
    key.zeroize();

    Ok(VaultEnvelope {
        version: VAULT_FORMAT_VERSION,
        kdf,
        nonce_b64: STANDARD.encode(nonce),
        ciphertext_b64: STANDARD.encode(ciphertext),
    })
}

pub fn decrypt_vault(
    envelope: &VaultEnvelope,
    password: &SecretString,
) -> Result<DecryptedVault, StorageError> {
    if !matches!(envelope.version, 1 | 2 | VAULT_FORMAT_VERSION) {
        return Err(StorageError::UnsupportedVersion);
    }

    let mut key = derive_key(password, &envelope.kdf)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| StorageError::CipherInit)?;

    let nonce_bytes = STANDARD
        .decode(&envelope.nonce_b64)
        .map_err(|_| StorageError::MalformedEnvelope)?;
    let nonce: [u8; 12] = nonce_bytes
        .try_into()
        .map_err(|_| StorageError::MalformedEnvelope)?;

    let ciphertext = STANDARD
        .decode(&envelope.ciphertext_b64)
        .map_err(|_| StorageError::MalformedEnvelope)?;

    let mut plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| StorageError::UnlockFailed)?;
    let mut vault =
        serde_json::from_slice::<VaultData>(&plaintext).map_err(StorageError::Deserialize)?;

    let migrated =
        envelope.version != VAULT_FORMAT_VERSION || vault.version != VAULT_FORMAT_VERSION;
    if !matches!(vault.version, 1 | 2 | VAULT_FORMAT_VERSION) {
        plaintext.zeroize();
        key.zeroize();
        return Err(StorageError::UnsupportedVersion);
    }

    if migrated {
        vault = migrate_vault_data(vault)?;
    }

    plaintext.zeroize();
    key.zeroize();

    Ok(DecryptedVault {
        data: vault,
        migrated,
    })
}

fn migrate_vault_data(mut vault: VaultData) -> Result<VaultData, StorageError> {
    if vault.version < 3 {
        let mut derived_directories = vault.directories;

        for account in &vault.accounts {
            let Some(project_path) = account.public.metadata.project_path.as_deref() else {
                continue;
            };

            if derived_directories
                .iter()
                .any(|directory| directory.path == project_path)
            {
                continue;
            }

            derived_directories.push(ProjectDirectory::with_timestamps(
                project_path.to_owned(),
                account.public.created_at,
                account
                    .public
                    .metadata
                    .updated_at
                    .max(account.public.created_at),
            )?);
        }

        vault.directories = derived_directories;
    }

    vault.version = VAULT_FORMAT_VERSION;
    Ok(vault)
}

fn derive_key(
    password: &SecretString,
    kdf: &KdfParameters,
) -> Result<[u8; KEY_LENGTH], StorageError> {
    if password.expose_secret().trim().is_empty() {
        return Err(StorageError::EmptyMasterPassword);
    }

    let params = Params::new(
        kdf.memory_cost_kib,
        kdf.iterations,
        kdf.parallelism,
        Some(KEY_LENGTH),
    )
    .map_err(|_| StorageError::KeyDerivation)?;

    let mut key = [0u8; KEY_LENGTH];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(
            password.expose_secret().as_bytes(),
            kdf.salt.as_bytes(),
            &mut key,
        )
        .map_err(|_| StorageError::KeyDerivation)?;

    Ok(key)
}

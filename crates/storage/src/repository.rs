use std::{
    collections::{BTreeMap, HashSet},
    ffi::OsString,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use secrecy::SecretString;
use uuid::Uuid;

use mfa_forge_core::{
    AccountHistoryEntryPublic, AccountHistoryEvent, AccountPublic, AccountRecord, AccountSelector,
    CoreError, ProjectDirectory, normalize_project_path_value,
};

use crate::{
    StorageError,
    crypto::{DecryptedVault, decrypt_vault, encrypt_vault},
    types::{AccountHistoryEntry, VaultData, VaultEnvelope},
};

pub fn default_vault_path() -> Result<PathBuf, StorageError> {
    let project_dirs = ProjectDirs::from("dev", "OpsZone", "MFA-Forge")
        .ok_or(StorageError::DefaultPathUnavailable)?;
    Ok(project_dirs.data_local_dir().join("vault.json"))
}

#[derive(Debug, Clone)]
pub struct VaultRepository {
    path: PathBuf,
}

impl VaultRepository {
    pub fn with_default_path() -> Result<Self, StorageError> {
        Ok(Self {
            path: default_vault_path()?,
        })
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn initialize(&self, password: &SecretString) -> Result<(), StorageError> {
        if self.path.exists() {
            return Err(StorageError::VaultAlreadyExists(self.path.clone()));
        }
        self.save(password, &VaultData::default())
    }

    pub fn add_account(
        &self,
        password: &SecretString,
        account: AccountRecord,
    ) -> Result<AccountPublic, StorageError> {
        let mut vault = self.load_and_maybe_migrate(password)?;
        ensure_not_duplicate(&vault.accounts, account.public(), None)?;

        let public = account.public_view();
        vault.accounts.push(account);
        sort_accounts(&mut vault.accounts);
        normalize_directory_registry(&mut vault)?;
        self.save(password, &vault)?;

        Ok(public)
    }

    pub fn add_accounts(
        &self,
        password: &SecretString,
        accounts: Vec<AccountRecord>,
    ) -> Result<Vec<AccountPublic>, StorageError> {
        let mut vault = self.load_and_maybe_migrate(password)?;
        let mut imported = Vec::with_capacity(accounts.len());

        for account in accounts {
            ensure_not_duplicate(&vault.accounts, account.public(), None)?;
            imported.push(account.public_view());
            vault.accounts.push(account);
        }

        sort_accounts(&mut vault.accounts);
        normalize_directory_registry(&mut vault)?;
        self.save(password, &vault)?;
        imported.sort_by_key(AccountPublic::sort_key);
        Ok(imported)
    }

    pub fn update_account(
        &self,
        password: &SecretString,
        account: AccountRecord,
    ) -> Result<AccountPublic, StorageError> {
        let mut vault = self.load_and_maybe_migrate(password)?;
        let account_id = account.public().id;
        ensure_not_duplicate(&vault.accounts, account.public(), Some(account_id))?;

        let index = vault
            .accounts
            .iter()
            .position(|existing| existing.public().id == account_id)
            .ok_or(CoreError::AccountNotFound)?;

        capture_history(
            &mut vault.history,
            AccountHistoryEvent::Updated,
            vault.accounts[index].clone(),
        );

        let public = account.public_view();
        vault.accounts[index] = account;
        sort_accounts(&mut vault.accounts);
        normalize_directory_registry(&mut vault)?;
        self.save(password, &vault)?;

        Ok(public)
    }

    pub fn list_accounts(
        &self,
        password: &SecretString,
    ) -> Result<Vec<AccountPublic>, StorageError> {
        let mut accounts = self
            .load_and_maybe_migrate(password)?
            .accounts
            .into_iter()
            .map(|account| account.public_view())
            .collect::<Vec<_>>();

        accounts.sort_by_key(AccountPublic::sort_key);
        Ok(accounts)
    }

    pub fn export_metadata(
        &self,
        password: &SecretString,
    ) -> Result<Vec<AccountPublic>, StorageError> {
        self.list_accounts(password)
    }

    pub fn list_directories(
        &self,
        password: &SecretString,
    ) -> Result<Vec<ProjectDirectory>, StorageError> {
        let mut directories = self.load_and_maybe_migrate(password)?.directories;
        sort_directories(&mut directories);
        Ok(directories)
    }

    pub fn create_directory(
        &self,
        password: &SecretString,
        path: impl Into<String>,
    ) -> Result<ProjectDirectory, StorageError> {
        let mut vault = self.load_and_maybe_migrate(password)?;
        let normalized_path = normalize_project_path_value(path)?;

        if let Some(existing) = vault
            .directories
            .iter()
            .find(|directory| directory.path == normalized_path)
            .cloned()
        {
            return Ok(existing);
        }

        let directory = ProjectDirectory::new(normalized_path)?;
        vault.directories.push(directory.clone());
        sort_directories(&mut vault.directories);
        self.save(password, &vault)?;
        Ok(directory)
    }

    pub fn delete_directory(
        &self,
        password: &SecretString,
        path: impl Into<String>,
    ) -> Result<ProjectDirectory, StorageError> {
        let mut vault = self.load_and_maybe_migrate(password)?;
        let normalized_path = normalize_project_path_value(path)?;

        if vault.accounts.iter().any(|account| {
            account
                .public()
                .metadata
                .project_path
                .as_deref()
                .is_some_and(|project_path| {
                    path_contains_or_descends(project_path, &normalized_path)
                })
        }) {
            return Err(StorageError::DirectoryNotEmpty(normalized_path));
        }

        if vault.directories.iter().any(|directory| {
            directory.path != normalized_path
                && path_contains_or_descends(&directory.path, &normalized_path)
        }) {
            return Err(StorageError::DirectoryNotEmpty(normalized_path));
        }

        let index = vault
            .directories
            .iter()
            .position(|directory| directory.path == normalized_path)
            .ok_or_else(|| StorageError::DirectoryNotFound(normalized_path.clone()))?;
        let removed = vault.directories.remove(index);
        self.save(password, &vault)?;
        Ok(removed)
    }

    pub fn list_history(
        &self,
        password: &SecretString,
    ) -> Result<Vec<AccountHistoryEntryPublic>, StorageError> {
        let mut history = self
            .load_and_maybe_migrate(password)?
            .history
            .into_iter()
            .map(|entry| entry.public_view())
            .collect::<Vec<_>>();

        history.sort_by_key(|entry| std::cmp::Reverse(entry.captured_at));
        Ok(history)
    }

    pub fn list_restore_candidates(
        &self,
        password: &SecretString,
    ) -> Result<Vec<AccountHistoryEntryPublic>, StorageError> {
        let mut vault = self.load_and_maybe_migrate(password)?;
        vault
            .history
            .sort_by_key(|entry| std::cmp::Reverse(entry.captured_at));

        let active_account_ids = vault
            .accounts
            .iter()
            .map(|account| account.public().id)
            .collect::<HashSet<_>>();
        let mut removed_account_ids = HashSet::new();

        Ok(vault
            .history
            .into_iter()
            .filter(|entry| match entry.event {
                AccountHistoryEvent::Updated => true,
                AccountHistoryEvent::Removed => {
                    let account_id = entry.account.public().id;
                    !active_account_ids.contains(&account_id)
                        && removed_account_ids.insert(account_id)
                }
                AccountHistoryEvent::Restored => false,
            })
            .map(|entry| entry.public_view())
            .collect())
    }

    pub fn find_account(
        &self,
        password: &SecretString,
        selector: &AccountSelector,
    ) -> Result<AccountRecord, StorageError> {
        let vault = self.load_and_maybe_migrate(password)?;
        select_account(&vault.accounts, selector).cloned()
    }

    pub fn remove_account(
        &self,
        password: &SecretString,
        selector: &AccountSelector,
    ) -> Result<AccountPublic, StorageError> {
        let mut vault = self.load_and_maybe_migrate(password)?;
        let selected_id = select_account(&vault.accounts, selector)?.public().id;

        let index = vault
            .accounts
            .iter()
            .position(|account| account.public().id == selected_id)
            .ok_or(StorageError::InvariantViolation)?;

        let removed = vault.accounts.remove(index);
        capture_history(
            &mut vault.history,
            AccountHistoryEvent::Removed,
            removed.clone(),
        );
        self.save(password, &vault)?;

        Ok(removed.public_view())
    }

    pub fn remove_accounts_by_ids(
        &self,
        password: &SecretString,
        account_ids: &[Uuid],
    ) -> Result<Vec<AccountPublic>, StorageError> {
        let mut vault = self.load_and_maybe_migrate(password)?;
        let targets = account_ids.iter().copied().collect::<HashSet<_>>();
        let existing = vault
            .accounts
            .iter()
            .map(|account| account.public().id)
            .collect::<HashSet<_>>();

        if !targets.is_subset(&existing) {
            return Err(StorageError::InvariantViolation);
        }

        let mut removed = Vec::new();
        let mut kept_accounts = Vec::with_capacity(vault.accounts.len());

        for account in std::mem::take(&mut vault.accounts) {
            if targets.contains(&account.public().id) {
                capture_history(
                    &mut vault.history,
                    AccountHistoryEvent::Removed,
                    account.clone(),
                );
                removed.push(account.public_view());
            } else {
                kept_accounts.push(account);
            }
        }

        vault.accounts = kept_accounts;
        self.save(password, &vault)?;
        removed.sort_by_key(AccountPublic::sort_key);
        Ok(removed)
    }

    pub fn restore_history_entry(
        &self,
        password: &SecretString,
        entry_id: Uuid,
    ) -> Result<AccountPublic, StorageError> {
        let mut vault = self.load_and_maybe_migrate(password)?;
        let history_entry = vault
            .history
            .iter()
            .find(|entry| entry.entry_id == entry_id)
            .cloned()
            .ok_or(StorageError::HistoryEntryNotFound(entry_id))?;

        let restore_at = unix_timestamp_now();
        let account_id = history_entry.account.public().id;
        let mut restored = history_entry.account.clone();
        restored.public.metadata.updated_at = restore_at;

        if let Some(index) = vault
            .accounts
            .iter()
            .position(|account| account.public().id == account_id)
        {
            ensure_not_duplicate(&vault.accounts, restored.public(), Some(account_id))?;
            capture_history(
                &mut vault.history,
                AccountHistoryEvent::Restored,
                vault.accounts[index].clone(),
            );
            vault.accounts[index] = restored.clone();
        } else {
            ensure_not_duplicate(&vault.accounts, restored.public(), None)?;
            vault.accounts.push(restored.clone());
        }

        sort_accounts(&mut vault.accounts);
        normalize_directory_registry(&mut vault)?;
        self.save(password, &vault)?;
        Ok(restored.public_view())
    }

    pub fn change_master_password(
        &self,
        current_password: &SecretString,
        new_password: &SecretString,
    ) -> Result<(), StorageError> {
        let vault = self.load_and_maybe_migrate(current_password)?;
        self.save(new_password, &vault)
    }

    /// Restores the primary vault file from the last backup snapshot.
    pub fn restore_from_backup(&self) -> Result<(), StorageError> {
        let backup_path = self.backup_path();

        if !backup_path.exists() {
            return Err(StorageError::VaultBackupNotFound(backup_path));
        }

        let bytes = fs::read(&backup_path).map_err(StorageError::ReadFile)?;
        let temp_path = self.temp_path();
        remove_if_exists(&temp_path)?;
        write_bytes(&temp_path, &bytes)?;

        if self.path.exists() {
            fs::remove_file(&self.path).map_err(StorageError::RemoveFile)?;
        }

        fs::rename(&temp_path, &self.path).map_err(StorageError::PersistVault)?;
        Ok(())
    }

    pub fn export_vault_file(&self, destination: &Path) -> Result<(), StorageError> {
        if !self.path.exists() {
            return Err(StorageError::VaultNotInitialized(self.path.clone()));
        }

        let bytes = fs::read(&self.path).map_err(StorageError::ReadFile)?;
        write_bytes_at(destination, &bytes)
    }

    pub fn import_vault_file(
        &self,
        password: &SecretString,
        source: &Path,
    ) -> Result<usize, StorageError> {
        let bytes = fs::read(source).map_err(StorageError::ReadFile)?;
        let envelope =
            serde_json::from_slice::<VaultEnvelope>(&bytes).map_err(StorageError::Deserialize)?;
        let mut decrypted = decrypt_vault(&envelope, password)?;
        let _ = normalize_directory_registry(&mut decrypted.data)?;
        let account_count = decrypted.data.accounts.len();
        self.save(password, &decrypted.data)?;
        Ok(account_count)
    }

    fn load_and_maybe_migrate(&self, password: &SecretString) -> Result<VaultData, StorageError> {
        let mut decrypted = self.load_state(password)?;
        let directories_changed = normalize_directory_registry(&mut decrypted.data)?;
        if decrypted.migrated || directories_changed {
            self.save(password, &decrypted.data)?;
        }

        Ok(decrypted.data)
    }

    fn load_state(&self, password: &SecretString) -> Result<DecryptedVault, StorageError> {
        if !self.path.exists() {
            return Err(StorageError::VaultNotInitialized(self.path.clone()));
        }

        let bytes = fs::read(&self.path).map_err(StorageError::ReadFile)?;
        let envelope =
            serde_json::from_slice::<VaultEnvelope>(&bytes).map_err(StorageError::Deserialize)?;
        decrypt_vault(&envelope, password)
    }

    fn save(&self, password: &SecretString, vault: &VaultData) -> Result<(), StorageError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(StorageError::CreateDir)?;
        }

        let envelope = encrypt_vault(vault, password)?;
        let bytes = serde_json::to_vec_pretty(&envelope).map_err(StorageError::Serialize)?;
        let temp_path = self.temp_path();
        let backup_path = self.backup_path();

        remove_if_exists(&temp_path)?;
        write_bytes(&temp_path, &bytes)?;

        if self.path.exists() {
            remove_if_exists(&backup_path)?;
            fs::rename(&self.path, &backup_path).map_err(StorageError::BackupVault)?;
        }

        if let Err(error) = fs::rename(&temp_path, &self.path) {
            let _ = remove_if_exists(&temp_path);

            if backup_path.exists() {
                fs::rename(&backup_path, &self.path).map_err(StorageError::RestoreBackup)?;
            }

            return Err(StorageError::PersistVault(error));
        }

        Ok(())
    }

    fn temp_path(&self) -> PathBuf {
        sibling_path_with_suffix(&self.path, "tmp")
    }

    fn backup_path(&self) -> PathBuf {
        sibling_path_with_suffix(&self.path, "bak")
    }
}

fn capture_history(
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

fn ensure_not_duplicate(
    accounts: &[AccountRecord],
    candidate: &AccountPublic,
    ignored_id: Option<Uuid>,
) -> Result<(), CoreError> {
    if accounts
        .iter()
        .filter(|existing| Some(existing.public().id) != ignored_id)
        .any(|existing| existing.shares_identity_with(candidate))
    {
        return Err(CoreError::DuplicateAccount);
    }
    Ok(())
}

fn select_account<'a>(
    accounts: &'a [AccountRecord],
    selector: &AccountSelector,
) -> Result<&'a AccountRecord, StorageError> {
    let mut matches = accounts
        .iter()
        .filter(|account| selector.matches(account.public()))
        .collect::<Vec<_>>();

    match matches.len() {
        0 => Err(CoreError::AccountNotFound.into()),
        1 => Ok(matches.remove(0)),
        _ => Err(CoreError::AmbiguousAccount.into()),
    }
}

fn sort_accounts(accounts: &mut [AccountRecord]) {
    accounts.sort_by_key(|account| account.public().sort_key());
}

fn sort_directories(directories: &mut [ProjectDirectory]) {
    directories.sort_by_key(|directory| directory.path.to_ascii_lowercase());
}

fn path_contains_or_descends(candidate: &str, root: &str) -> bool {
    candidate == root
        || candidate
            .strip_prefix(root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn normalize_directory_registry(vault: &mut VaultData) -> Result<bool, StorageError> {
    let mut directories_by_path = BTreeMap::<String, (u64, u64)>::new();

    for directory in &vault.directories {
        let normalized_path = normalize_project_path_value(directory.path.clone())?;
        let entry = directories_by_path
            .entry(normalized_path)
            .or_insert((directory.created_at, directory.updated_at));
        entry.0 = entry.0.min(directory.created_at);
        entry.1 = entry.1.max(directory.updated_at.max(directory.created_at));
    }

    for account in &vault.accounts {
        let Some(project_path) = account.public.metadata.project_path.clone() else {
            continue;
        };

        let normalized_path = normalize_project_path_value(project_path)?;
        let entry = directories_by_path.entry(normalized_path).or_insert((
            account.public.created_at,
            account
                .public
                .metadata
                .updated_at
                .max(account.public.created_at),
        ));
        entry.0 = entry.0.min(account.public.created_at);
        entry.1 = entry.1.max(
            account
                .public
                .metadata
                .updated_at
                .max(account.public.created_at),
        );
    }

    let mut normalized_directories = directories_by_path
        .into_iter()
        .map(|(path, (created_at, updated_at))| {
            ProjectDirectory::with_timestamps(path, created_at, updated_at)
        })
        .collect::<Result<Vec<_>, _>>()?;
    sort_directories(&mut normalized_directories);

    let changed = vault.directories != normalized_directories;
    vault.directories = normalized_directories;
    Ok(changed)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    let mut file = fs::File::create(path).map_err(StorageError::WriteFile)?;
    file.write_all(bytes).map_err(StorageError::WriteFile)?;
    file.sync_all().map_err(StorageError::WriteFile)?;
    Ok(())
}

fn write_bytes_at(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(StorageError::CreateDir)?;
    }

    let temp_path = sibling_path_with_suffix(path, "tmp");
    remove_if_exists(&temp_path)?;
    write_bytes(&temp_path, bytes)?;

    if let Err(error) = (|| {
        remove_if_exists(path)?;
        fs::rename(&temp_path, path).map_err(StorageError::PersistVault)
    })() {
        let _ = remove_if_exists(&temp_path);
        return Err(error);
    }

    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<(), StorageError> {
    if path.exists() {
        fs::remove_file(path).map_err(StorageError::RemoveFile)?;
    }

    Ok(())
}

fn sibling_path_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut file_name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| OsString::from("vault"));
    file_name.push(format!(".{suffix}"));
    path.with_file_name(file_name)
}

fn unix_timestamp_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use secrecy::SecretString;
    use tempfile::TempDir;

    use mfa_forge_core::{
        AccountHistoryEvent, AccountMetadata, AccountRecord, AccountSelector, TotpConfig,
        test_support::{base32_secret_from_seed, secret_string_from_seed},
    };

    use crate::{
        StorageError,
        types::{KdfParameters, VaultEnvelope},
    };

    use super::VaultRepository;

    fn repo_fixture() -> (TempDir, VaultRepository, SecretString) {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let repository = VaultRepository::new(temp_dir.path().join("vault.json"));
        let password = SecretString::from("correct horse battery staple".to_owned());

        repository
            .initialize(&password)
            .expect("vault should initialize");

        (temp_dir, repository, password)
    }

    #[test]
    fn vault_round_trip_and_no_plain_secret_on_disk() {
        let (_temp_dir, repository, password) = repo_fixture();
        let account = AccountRecord::new(
            "GitHub",
            "user@example.com",
            secret_string_from_seed("repository-primary"),
            TotpConfig::default(),
        )
        .expect("account should be valid");

        repository
            .add_account(&password, account)
            .expect("account should persist");

        let file_contents =
            fs::read_to_string(repository.path()).expect("vault file should be readable");
        assert!(!file_contents.contains(&base32_secret_from_seed("repository-primary")));

        let accounts = repository
            .list_accounts(&password)
            .expect("accounts should load");
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].service, "GitHub");
    }

    #[test]
    fn remove_account_updates_vault() {
        let (_temp_dir, repository, password) = repo_fixture();
        let account = AccountRecord::new(
            "GitHub",
            "user@example.com",
            secret_string_from_seed("repository-primary"),
            TotpConfig::default(),
        )
        .expect("account should be valid");

        repository
            .add_account(&password, account)
            .expect("account should persist");

        let selector =
            AccountSelector::new("GitHub", Some("user@example.com".to_owned())).expect("selector");

        repository
            .remove_account(&password, &selector)
            .expect("account should be removed");

        let accounts = repository
            .list_accounts(&password)
            .expect("accounts should load");
        assert!(accounts.is_empty());
    }

    #[test]
    fn backup_snapshot_is_encrypted_and_restorable() {
        let (_temp_dir, repository, password) = repo_fixture();
        let github = AccountRecord::new(
            "GitHub",
            "user@example.com",
            secret_string_from_seed("repository-primary"),
            TotpConfig::default(),
        )
        .expect("github account should be valid");
        let gitlab = AccountRecord::new(
            "GitLab",
            "dev@example.com",
            secret_string_from_seed("repository-secondary"),
            TotpConfig::default(),
        )
        .expect("gitlab account should be valid");

        repository
            .add_account(&password, github)
            .expect("github account should persist");
        repository
            .add_account(&password, gitlab)
            .expect("gitlab account should persist");

        let backup_path = repository.backup_path();
        assert!(backup_path.exists(), "backup snapshot should exist");

        let backup_contents = fs::read_to_string(&backup_path).expect("backup should be readable");
        assert!(!backup_contents.contains(&base32_secret_from_seed("repository-primary")));
        assert!(!backup_contents.contains(&base32_secret_from_seed("repository-secondary")));

        fs::write(repository.path(), "corrupted vault").expect("main vault should be corrupted");

        repository
            .restore_from_backup()
            .expect("backup should restore");

        let accounts = repository
            .list_accounts(&password)
            .expect("accounts should load from restored backup");
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].service, "GitHub");
    }

    #[test]
    fn update_account_persists_new_metadata() {
        let (_temp_dir, repository, password) = repo_fixture();
        let account = AccountRecord::new(
            "GitHub",
            "user@example.com",
            secret_string_from_seed("repository-primary"),
            TotpConfig::default(),
        )
        .expect("account should be valid");

        let stored = repository
            .add_account(&password, account)
            .expect("account should persist");

        let selector =
            AccountSelector::new("GitHub", Some("user@example.com".to_owned())).expect("selector");
        let existing = repository
            .find_account(&password, &selector)
            .expect("account should load");
        let updated = existing
            .update_with_metadata(
                "GitHub Enterprise",
                "dev@example.com",
                None,
                TotpConfig {
                    digits: 8,
                    ..TotpConfig::default()
                },
                AccountMetadata {
                    labels: vec!["work".to_owned()],
                    note: Some("Infra".to_owned()),
                    project_path: Some("ClientA/Auth".to_owned()),
                    source: Some("manual".to_owned()),
                    updated_at: 0,
                },
            )
            .expect("update should be valid");

        let updated_public = repository
            .update_account(&password, updated)
            .expect("updated account should persist");

        assert_eq!(updated_public.id, stored.id);
        assert_eq!(updated_public.service, "GitHub Enterprise");
        assert_eq!(updated_public.user, "dev@example.com");
        assert_eq!(updated_public.totp.digits, 8);
        assert_eq!(updated_public.metadata.labels, vec!["work"]);
        assert_eq!(
            updated_public.metadata.project_path.as_deref(),
            Some("ClientA/Auth")
        );

        let directories = repository
            .list_directories(&password)
            .expect("directories should load");
        assert_eq!(directories.len(), 1);
        assert_eq!(directories[0].path, "ClientA/Auth");
    }

    #[test]
    fn change_master_password_reencrypts_the_existing_vault() {
        let (_temp_dir, repository, password) = repo_fixture();
        let new_password = SecretString::from("new stronger password".to_owned());
        let account = AccountRecord::new(
            "GitHub",
            "user@example.com",
            secret_string_from_seed("repository-primary"),
            TotpConfig::default(),
        )
        .expect("account should be valid");

        repository
            .add_account(&password, account)
            .expect("account should persist");

        repository
            .change_master_password(&password, &new_password)
            .expect("password rotation should succeed");

        assert!(repository.list_accounts(&password).is_err());

        let accounts = repository
            .list_accounts(&new_password)
            .expect("new password should unlock the vault");
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].service, "GitHub");
    }

    #[test]
    fn add_accounts_is_atomic_when_a_duplicate_exists() {
        let (_temp_dir, repository, password) = repo_fixture();

        repository
            .add_account(
                &password,
                AccountRecord::new(
                    "GitHub",
                    "user@example.com",
                    secret_string_from_seed("repository-primary"),
                    TotpConfig::default(),
                )
                .expect("existing account should be valid"),
            )
            .expect("existing account should persist");

        let batch = vec![
            AccountRecord::new(
                "GitLab",
                "dev@example.com",
                secret_string_from_seed("repository-secondary"),
                TotpConfig::default(),
            )
            .expect("first batch account should be valid"),
            AccountRecord::new(
                "GitHub",
                "user@example.com",
                secret_string_from_seed("repository-primary"),
                TotpConfig::default(),
            )
            .expect("duplicate batch account should be valid before persistence"),
        ];

        let error = repository
            .add_accounts(&password, batch)
            .expect_err("batch import should fail on duplicate");
        assert!(matches!(
            error,
            StorageError::Core(mfa_forge_core::CoreError::DuplicateAccount)
        ));

        let accounts = repository
            .list_accounts(&password)
            .expect("vault should remain readable");
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].service, "GitHub");
    }

    #[test]
    fn update_and_remove_create_history_snapshots() {
        let (_temp_dir, repository, password) = repo_fixture();
        let stored = repository
            .add_account(
                &password,
                AccountRecord::new(
                    "GitHub",
                    "user@example.com",
                    secret_string_from_seed("repository-primary"),
                    TotpConfig::default(),
                )
                .expect("account should be valid"),
            )
            .expect("account should persist");

        let selector =
            AccountSelector::new("GitHub", Some("user@example.com".to_owned())).expect("selector");
        let existing = repository
            .find_account(&password, &selector)
            .expect("account should load");
        let updated = existing
            .update(
                "GitHub Enterprise",
                "dev@example.com",
                None,
                TotpConfig::default(),
            )
            .expect("update should be valid");
        repository
            .update_account(&password, updated)
            .expect("update should persist");

        let updated_selector =
            AccountSelector::new("GitHub Enterprise", Some("dev@example.com".to_owned()))
                .expect("selector");
        repository
            .remove_account(&password, &updated_selector)
            .expect("remove should persist");

        let history = repository
            .list_history(&password)
            .expect("history should load");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].account.id, stored.id);
        assert_eq!(history[1].account.id, stored.id);
    }

    #[test]
    fn restore_history_entry_recovers_removed_account() {
        let (_temp_dir, repository, password) = repo_fixture();
        repository
            .add_account(
                &password,
                AccountRecord::new(
                    "GitHub",
                    "user@example.com",
                    secret_string_from_seed("repository-primary"),
                    TotpConfig::default(),
                )
                .expect("account should be valid"),
            )
            .expect("account should persist");

        let selector =
            AccountSelector::new("GitHub", Some("user@example.com".to_owned())).expect("selector");
        repository
            .remove_account(&password, &selector)
            .expect("remove should persist");

        let history = repository
            .list_history(&password)
            .expect("history should load");
        let restored = repository
            .restore_history_entry(&password, history[0].entry_id)
            .expect("restore should succeed");

        assert_eq!(restored.service, "GitHub");
        let accounts = repository
            .list_accounts(&password)
            .expect("accounts should load");
        assert_eq!(accounts.len(), 1);
    }

    #[test]
    fn restore_candidates_include_all_removed_accounts_until_they_are_restored() {
        let (_temp_dir, repository, password) = repo_fixture();
        repository
            .add_account(
                &password,
                AccountRecord::new(
                    "GitHub",
                    "user1@example.com",
                    secret_string_from_seed("repository-primary"),
                    TotpConfig::default(),
                )
                .expect("first account should be valid"),
            )
            .expect("first account should persist");
        repository
            .add_account(
                &password,
                AccountRecord::new(
                    "GitLab",
                    "user2@example.com",
                    secret_string_from_seed("repository-secondary"),
                    TotpConfig::default(),
                )
                .expect("second account should be valid"),
            )
            .expect("second account should persist");

        let github_selector =
            AccountSelector::new("GitHub", Some("user1@example.com".to_owned())).expect("selector");
        let gitlab_selector =
            AccountSelector::new("GitLab", Some("user2@example.com".to_owned())).expect("selector");

        repository
            .remove_account(&password, &github_selector)
            .expect("first account should be removed");
        repository
            .remove_account(&password, &gitlab_selector)
            .expect("second account should be removed");

        let candidates = repository
            .list_restore_candidates(&password)
            .expect("restore candidates should load");
        assert_eq!(candidates.len(), 2);
        assert!(
            candidates
                .iter()
                .all(|entry| entry.event == AccountHistoryEvent::Removed)
        );

        repository
            .restore_history_entry(&password, candidates[0].entry_id)
            .expect("restore should succeed");

        let remaining = repository
            .list_restore_candidates(&password)
            .expect("remaining restore candidates should load");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].event, AccountHistoryEvent::Removed);
        assert_ne!(remaining[0].account.id, candidates[0].account.id);
    }

    #[test]
    fn create_directory_persists_empty_project_groups() {
        let (_temp_dir, repository, password) = repo_fixture();

        let directory = repository
            .create_directory(&password, "ClientA/Auth/Prod")
            .expect("directory should persist");
        assert_eq!(directory.path, "ClientA/Auth/Prod");

        let directories = repository
            .list_directories(&password)
            .expect("directories should load");
        assert_eq!(directories.len(), 1);
        assert_eq!(directories[0].path, "ClientA/Auth/Prod");
    }

    #[test]
    fn delete_directory_removes_only_empty_leaf_directories() {
        let (_temp_dir, repository, password) = repo_fixture();

        repository
            .create_directory(&password, "ClientA/Auth/Prod")
            .expect("leaf directory should persist");

        let removed = repository
            .delete_directory(&password, "ClientA/Auth/Prod")
            .expect("empty leaf directory should be removable");
        assert_eq!(removed.path, "ClientA/Auth/Prod");

        let directories = repository
            .list_directories(&password)
            .expect("directories should load");
        assert!(directories.is_empty());
    }

    #[test]
    fn delete_directory_rejects_non_empty_or_nested_paths() {
        let (_temp_dir, repository, password) = repo_fixture();

        repository
            .create_directory(&password, "ClientA/Auth")
            .expect("root directory should persist");
        repository
            .create_directory(&password, "ClientA/Auth/Prod")
            .expect("child directory should persist");

        let error = repository
            .delete_directory(&password, "ClientA/Auth")
            .expect_err("directory with child directories should not be removable");
        assert!(matches!(error, StorageError::DirectoryNotEmpty(path) if path == "ClientA/Auth"));
    }

    #[test]
    fn remove_accounts_by_ids_removes_multiple_accounts_and_keeps_history() {
        let (_temp_dir, repository, password) = repo_fixture();
        let first = repository
            .add_account(
                &password,
                AccountRecord::new(
                    "GitHub",
                    "user1@example.com",
                    secret_string_from_seed("repository-primary"),
                    TotpConfig::default(),
                )
                .expect("first account should be valid"),
            )
            .expect("first account should persist");
        let second = repository
            .add_account(
                &password,
                AccountRecord::new(
                    "GitLab",
                    "user2@example.com",
                    secret_string_from_seed("repository-secondary"),
                    TotpConfig::default(),
                )
                .expect("second account should be valid"),
            )
            .expect("second account should persist");

        let removed = repository
            .remove_accounts_by_ids(&password, &[first.id, second.id])
            .expect("multiple account removal should succeed");
        assert_eq!(removed.len(), 2);

        let accounts = repository
            .list_accounts(&password)
            .expect("accounts should load after removal");
        assert!(accounts.is_empty());

        let history = repository
            .list_history(&password)
            .expect("history should load");
        assert_eq!(history.len(), 2);
        assert!(
            history
                .iter()
                .all(|entry| entry.event == AccountHistoryEvent::Removed)
        );
    }

    #[test]
    fn remove_accounts_by_ids_keeps_restore_candidates_until_each_account_is_restored() {
        let (_temp_dir, repository, password) = repo_fixture();
        let first = repository
            .add_account(
                &password,
                AccountRecord::new(
                    "GitHub",
                    "user1@example.com",
                    secret_string_from_seed("repository-primary"),
                    TotpConfig::default(),
                )
                .expect("first account should be valid"),
            )
            .expect("first account should persist");
        let second = repository
            .add_account(
                &password,
                AccountRecord::new(
                    "GitLab",
                    "user2@example.com",
                    secret_string_from_seed("repository-secondary"),
                    TotpConfig::default(),
                )
                .expect("second account should be valid"),
            )
            .expect("second account should persist");

        repository
            .remove_accounts_by_ids(&password, &[first.id, second.id])
            .expect("multiple account removal should succeed");

        let candidates = repository
            .list_restore_candidates(&password)
            .expect("restore candidates should load after bulk removal");
        assert_eq!(candidates.len(), 2);
        assert!(
            candidates
                .iter()
                .all(|entry| entry.event == AccountHistoryEvent::Removed)
        );

        repository
            .restore_history_entry(&password, candidates[0].entry_id)
            .expect("first removed account should restore");

        let remaining = repository
            .list_restore_candidates(&password)
            .expect("remaining restore candidates should load");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].event, AccountHistoryEvent::Removed);
        assert_ne!(remaining[0].account.id, candidates[0].account.id);
    }

    #[test]
    fn list_accounts_migrates_a_v1_vault_to_current_version() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let repository = VaultRepository::new(temp_dir.path().join("vault.json"));
        let password = SecretString::from("correct horse battery staple".to_owned());
        let account = AccountRecord::new(
            "GitHub",
            "user@example.com",
            secret_string_from_seed("repository-primary"),
            TotpConfig::default(),
        )
        .expect("account should be valid");

        let legacy_data = serde_json::json!({
            "version": 1,
            "accounts": [account]
        });
        let envelope = VaultEnvelope {
            version: 1,
            kdf: KdfParameters::generated(),
            nonce_b64: String::new(),
            ciphertext_b64: String::new(),
        };

        let encrypted = crate::crypto::encrypt_vault(
            &serde_json::from_value(legacy_data).expect("legacy data should deserialize"),
            &password,
        )
        .expect("legacy vault should encrypt");

        let legacy_envelope = VaultEnvelope {
            version: 1,
            ..encrypted
        };

        fs::write(
            repository.path(),
            serde_json::to_vec_pretty(&legacy_envelope).expect("legacy envelope should serialize"),
        )
        .expect("legacy envelope should write");

        let accounts = repository
            .list_accounts(&password)
            .expect("legacy vault should migrate on read");
        assert_eq!(accounts.len(), 1);

        let stored = fs::read_to_string(repository.path()).expect("vault should be readable");
        let rewritten: VaultEnvelope =
            serde_json::from_str(&stored).expect("rewritten envelope should deserialize");
        assert_eq!(rewritten.version, 3);

        let directories = repository
            .list_directories(&password)
            .expect("directories should be derived during migration");
        assert!(directories.is_empty());
        let _ = envelope;
    }

    #[test]
    fn export_vault_file_copies_the_encrypted_backup() {
        let (temp_dir, repository, password) = repo_fixture();
        let account = AccountRecord::new(
            "GitHub",
            "user@example.com",
            secret_string_from_seed("repository-primary"),
            TotpConfig::default(),
        )
        .expect("account should be valid");
        let destination = temp_dir.path().join("exports").join("vault-export.json");

        repository
            .add_account(&password, account)
            .expect("account should persist");
        repository
            .export_vault_file(&destination)
            .expect("vault export should succeed");

        let exported = fs::read_to_string(&destination).expect("export should be readable");
        assert!(!exported.contains(&base32_secret_from_seed("repository-primary")));
    }

    #[test]
    fn import_vault_file_replaces_current_vault_with_valid_backup() {
        let (_temp_dir, repository, password) = repo_fixture();
        let backup_dir = TempDir::new().expect("backup dir should exist");
        let backup_repository = VaultRepository::new(backup_dir.path().join("vault.json"));
        let backup_destination = backup_dir.path().join("vault-backup.json");

        backup_repository
            .initialize(&password)
            .expect("backup vault should initialize");
        backup_repository
            .add_account(
                &password,
                AccountRecord::new(
                    "GitLab",
                    "dev@example.com",
                    secret_string_from_seed("repository-secondary"),
                    TotpConfig::default(),
                )
                .expect("backup account should be valid"),
            )
            .expect("backup account should persist");
        backup_repository
            .export_vault_file(&backup_destination)
            .expect("backup export should succeed");

        let imported_count = repository
            .import_vault_file(&password, &backup_destination)
            .expect("vault import should succeed");
        assert_eq!(imported_count, 1);

        let accounts = repository
            .list_accounts(&password)
            .expect("imported vault should be readable");
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].service, "GitLab");
    }
}

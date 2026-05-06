use std::path::Path;

use secrecy::SecretString;
use uuid::Uuid;

use mfa_forge_core::{
    AccountHistoryEntryPublic, AccountMetadata, AccountPublic, AccountRecord, AccountSelector,
    ProjectDirectory, TotpConfig, TotpToken,
};
use mfa_forge_storage::{StorageError, VaultRepository};

pub struct VaultFacade {
    repository: VaultRepository,
    session: Option<UnlockedSession>,
    path_display: String,
}

pub struct PendingUnlockSession {
    accounts: Vec<AccountPublic>,
    directories: Vec<ProjectDirectory>,
}

impl PendingUnlockSession {
    pub fn new(accounts: Vec<AccountPublic>, directories: Vec<ProjectDirectory>) -> Self {
        Self {
            accounts,
            directories,
        }
    }
}

struct UnlockedSession {
    master_password: SecretString,
    accounts: Vec<AccountPublic>,
    directories: Vec<ProjectDirectory>,
}

impl VaultFacade {
    pub fn try_new() -> Result<Self, StorageError> {
        Ok(Self::new(VaultRepository::with_default_path()?))
    }

    pub fn new(repository: VaultRepository) -> Self {
        let path_display = repository.path().display().to_string();

        Self {
            repository,
            session: None,
            path_display,
        }
    }

    pub fn path_display(&self) -> &str {
        &self.path_display
    }

    pub fn is_initialized(&self) -> bool {
        self.repository.path().exists()
    }

    pub fn is_unlocked(&self) -> bool {
        self.session.is_some()
    }

    pub fn accounts(&self) -> &[AccountPublic] {
        self.session
            .as_ref()
            .map(|session| session.accounts.as_slice())
            .unwrap_or(&[])
    }

    pub fn directories(&self) -> &[ProjectDirectory] {
        self.session
            .as_ref()
            .map(|session| session.directories.as_slice())
            .unwrap_or(&[])
    }

    pub fn account_by_id(&self, account_id: Uuid) -> Option<AccountPublic> {
        self.accounts()
            .iter()
            .find(|account| account.id == account_id)
            .cloned()
    }

    pub fn initialize_and_unlock(&mut self, password: SecretString) -> Result<(), String> {
        self.repository
            .initialize(&password)
            .map_err(to_user_error)?;
        self.unlock(password)
    }

    pub fn unlock(&mut self, password: SecretString) -> Result<(), String> {
        let pending = self.prepare_unlock(&password)?;
        self.finish_unlock(password, pending);
        Ok(())
    }

    pub fn prepare_unlock(&self, password: &SecretString) -> Result<PendingUnlockSession, String> {
        let accounts = self
            .repository
            .list_accounts(password)
            .map_err(to_user_error)?;
        let directories = self
            .repository
            .list_directories(password)
            .map_err(to_user_error)?;

        Ok(PendingUnlockSession {
            accounts,
            directories,
        })
    }

    pub fn finish_unlock(&mut self, password: SecretString, pending: PendingUnlockSession) {
        self.session = Some(UnlockedSession {
            master_password: password,
            accounts: pending.accounts,
            directories: pending.directories,
        });
    }

    /// Devuelve un snapshot serializable de la sesión ya desbloqueada.
    pub fn session_snapshot(&self) -> Result<PendingUnlockSession, String> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| "El vault está bloqueado.".to_owned())?;

        Ok(PendingUnlockSession::new(
            session.accounts.clone(),
            session.directories.clone(),
        ))
    }

    pub fn lock(&mut self) {
        self.session = None;
    }

    pub fn password_snapshot(&self) -> Result<SecretString, String> {
        self.password()
    }

    pub fn add_account(
        &mut self,
        service: String,
        user: String,
        secret: SecretString,
        config: TotpConfig,
    ) -> Result<AccountPublic, String> {
        self.add_account_with_metadata(service, user, secret, config, AccountMetadata::default())
    }

    pub fn add_account_with_metadata(
        &mut self,
        service: String,
        user: String,
        secret: SecretString,
        config: TotpConfig,
        metadata: AccountMetadata,
    ) -> Result<AccountPublic, String> {
        let password = self.password()?;
        let account = AccountRecord::new_with_metadata(service, user, secret, config, metadata)
            .map_err(to_user_error)?;
        let stored = self
            .repository
            .add_account(&password, account)
            .map_err(to_user_error)?;
        self.refresh_with(&password)?;
        Ok(stored)
    }

    pub fn import_otpauth(&mut self, uri: &str) -> Result<AccountPublic, String> {
        self.import_otpauth_with_metadata(uri, AccountMetadata::default())
    }

    pub fn import_otpauth_with_metadata(
        &mut self,
        uri: &str,
        metadata: AccountMetadata,
    ) -> Result<AccountPublic, String> {
        let password = self.password()?;
        let account =
            AccountRecord::from_otpauth_uri_with_metadata(uri, metadata).map_err(to_user_error)?;
        let stored = self
            .repository
            .add_account(&password, account)
            .map_err(to_user_error)?;
        self.refresh_with(&password)?;
        Ok(stored)
    }

    pub fn update_account(
        &mut self,
        existing: &AccountPublic,
        service: String,
        user: String,
        new_secret: Option<SecretString>,
        config: TotpConfig,
    ) -> Result<AccountPublic, String> {
        self.update_account_with_metadata(
            existing,
            service,
            user,
            new_secret,
            config,
            existing.metadata.clone(),
        )
    }

    pub fn update_account_with_metadata(
        &mut self,
        existing: &AccountPublic,
        service: String,
        user: String,
        new_secret: Option<SecretString>,
        config: TotpConfig,
        metadata: AccountMetadata,
    ) -> Result<AccountPublic, String> {
        let password = self.password()?;
        let selector = AccountSelector::new(existing.service.clone(), Some(existing.user.clone()))
            .map_err(to_user_error)?;
        let current = self
            .repository
            .find_account(&password, &selector)
            .map_err(to_user_error)?;
        let updated = current
            .update_with_metadata(service, user, new_secret, config, metadata)
            .map_err(to_user_error)?;
        let stored = self
            .repository
            .update_account(&password, updated)
            .map_err(to_user_error)?;
        self.refresh_with(&password)?;
        Ok(stored)
    }

    pub fn history(&self) -> Result<Vec<AccountHistoryEntryPublic>, String> {
        let password = self.password()?;
        self.repository
            .list_history(&password)
            .map_err(to_user_error)
    }

    /// Devuelve únicamente las entradas restaurables visibles para la GUI.
    pub fn restore_candidates(&self) -> Result<Vec<AccountHistoryEntryPublic>, String> {
        let password = self.password()?;
        self.repository
            .list_restore_candidates(&password)
            .map_err(to_user_error)
    }

    pub fn create_directory(
        &mut self,
        path: impl Into<String>,
    ) -> Result<ProjectDirectory, String> {
        let password = self.password()?;
        let directory = self
            .repository
            .create_directory(&password, path)
            .map_err(to_user_error)?;
        self.refresh_with(&password)?;
        Ok(directory)
    }

    pub fn delete_directory(
        &mut self,
        path: impl Into<String>,
    ) -> Result<ProjectDirectory, String> {
        let password = self.password()?;
        let directory = self
            .repository
            .delete_directory(&password, path)
            .map_err(to_user_error)?;
        self.refresh_with(&password)?;
        Ok(directory)
    }

    pub fn restore_history_entry(&mut self, entry_id: Uuid) -> Result<AccountPublic, String> {
        let password = self.password()?;
        let restored = self
            .repository
            .restore_history_entry(&password, entry_id)
            .map_err(to_user_error)?;
        self.refresh_with(&password)?;
        Ok(restored)
    }

    pub fn change_master_password(&mut self, new_password: SecretString) -> Result<(), String> {
        let current_password = self.password()?;
        self.repository
            .change_master_password(&current_password, &new_password)
            .map_err(to_user_error)?;

        if let Some(session) = &mut self.session {
            session.master_password = new_password;
        }

        Ok(())
    }

    pub fn remove_account(&mut self, account: &AccountPublic) -> Result<AccountPublic, String> {
        let password = self.password()?;
        let selector = AccountSelector::new(account.service.clone(), Some(account.user.clone()))
            .map_err(to_user_error)?;
        let removed = self
            .repository
            .remove_account(&password, &selector)
            .map_err(to_user_error)?;
        self.refresh_with(&password)?;
        Ok(removed)
    }

    pub fn remove_accounts(&mut self, account_ids: &[Uuid]) -> Result<Vec<AccountPublic>, String> {
        let password = self.password()?;
        let removed = self
            .repository
            .remove_accounts_by_ids(&password, account_ids)
            .map_err(to_user_error)?;
        self.refresh_with(&password)?;
        Ok(removed)
    }

    pub fn generate_token(&self, account: &AccountPublic) -> Result<TotpToken, String> {
        let password = self.password()?;
        let selector = AccountSelector::new(account.service.clone(), Some(account.user.clone()))
            .map_err(to_user_error)?;
        let record = self
            .repository
            .find_account(&password, &selector)
            .map_err(to_user_error)?;
        record.generate_current_token().map_err(to_user_error)
    }

    pub fn export_metadata_json(&self) -> Result<String, String> {
        let accounts = self.export_metadata()?;
        serde_json::to_string_pretty(&accounts).map_err(to_user_error)
    }

    pub fn export_metadata(&self) -> Result<Vec<AccountPublic>, String> {
        let password = self.password()?;
        self.repository
            .export_metadata(&password)
            .map_err(to_user_error)
    }

    /// Exporta el archivo cifrado actual del vault a una ruta externa.
    pub fn export_vault_backup(&self, destination: &Path) -> Result<(), String> {
        self.repository
            .export_vault_file(destination)
            .map_err(to_user_error)
    }

    /// Importa un backup cifrado compatible y refresca la sesión activa.
    pub fn import_vault_backup(&mut self, source: &Path) -> Result<usize, String> {
        let password = self.password()?;
        let imported = self
            .repository
            .import_vault_file(&password, source)
            .map_err(to_user_error)?;
        self.refresh_with(&password)?;
        Ok(imported)
    }

    /// Genera un URI `otpauth://` exportable para una cuenta visible.
    pub fn export_account_uri(&self, account: &AccountPublic) -> Result<String, String> {
        let password = self.password()?;
        let selector = AccountSelector::new(account.service.clone(), Some(account.user.clone()))
            .map_err(to_user_error)?;
        let record = self
            .repository
            .find_account(&password, &selector)
            .map_err(to_user_error)?;

        record.otpauth_uri().map_err(to_user_error)
    }

    fn refresh_with(&mut self, password: &SecretString) -> Result<(), String> {
        let accounts = self
            .repository
            .list_accounts(password)
            .map_err(to_user_error)?;
        let directories = self
            .repository
            .list_directories(password)
            .map_err(to_user_error)?;

        if let Some(session) = &mut self.session {
            session.accounts = accounts;
            session.directories = directories;
        }

        Ok(())
    }

    fn password(&self) -> Result<SecretString, String> {
        self.session
            .as_ref()
            .map(|session| session.master_password.clone())
            .ok_or_else(|| "El vault está bloqueado.".to_owned())
    }
}

fn to_user_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::VaultFacade;

    use secrecy::SecretString;
    use tempfile::TempDir;

    use mfa_forge_core::{
        AccountRecord, TotpConfig,
        test_support::{base32_secret_from_seed, secret_string_from_seed},
    };
    use mfa_forge_storage::VaultRepository;

    fn unlocked_facade() -> (TempDir, VaultFacade, SecretString) {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let repository = VaultRepository::new(temp_dir.path().join("vault.json"));
        let password = SecretString::from("correct horse battery staple".to_owned());
        let mut facade = VaultFacade::new(repository);

        facade
            .initialize_and_unlock(password.clone())
            .expect("vault should initialize and unlock");

        (temp_dir, facade, password)
    }

    fn add_account(facade: &mut VaultFacade, service: &str, user: &str) {
        facade
            .add_account(
                service.to_owned(),
                user.to_owned(),
                secret_string_from_seed("vault-facade-primary"),
                TotpConfig::default(),
            )
            .expect("account should persist");
    }

    #[test]
    fn remove_accounts_refreshes_session_and_restore_candidates() {
        let (_temp_dir, mut facade, _password) = unlocked_facade();
        add_account(&mut facade, "GitHub", "user1@example.com");
        add_account(&mut facade, "GitLab", "user2@example.com");

        let account_ids = facade
            .accounts()
            .iter()
            .map(|account| account.id)
            .collect::<Vec<_>>();

        let removed = facade
            .remove_accounts(&account_ids)
            .expect("bulk remove should succeed");
        assert_eq!(removed.len(), 2);
        assert!(facade.accounts().is_empty());

        let candidates = facade
            .restore_candidates()
            .expect("restore candidates should load");
        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn restore_history_entry_updates_restore_candidates() {
        let (_temp_dir, mut facade, _password) = unlocked_facade();
        add_account(&mut facade, "GitHub", "user1@example.com");
        add_account(&mut facade, "GitLab", "user2@example.com");

        let account_ids = facade
            .accounts()
            .iter()
            .map(|account| account.id)
            .collect::<Vec<_>>();
        facade
            .remove_accounts(&account_ids)
            .expect("bulk remove should succeed");

        let candidates = facade
            .restore_candidates()
            .expect("restore candidates should load");
        let restored = facade
            .restore_history_entry(candidates[0].entry_id)
            .expect("restore should succeed");

        let remaining = facade
            .restore_candidates()
            .expect("remaining restore candidates should load");
        assert_eq!(remaining.len(), 1);
        assert_ne!(remaining[0].account.id, restored.id);
    }

    #[test]
    fn export_account_uri_returns_otpauth_uri_for_visible_account() {
        let (_temp_dir, mut facade, _password) = unlocked_facade();
        add_account(&mut facade, "GitHub", "user@example.com");
        let account = facade.accounts()[0].clone();

        let uri = facade
            .export_account_uri(&account)
            .expect("account URI should export");
        assert!(uri.starts_with("otpauth://totp/"));
        assert!(uri.contains("GitHub"));
    }

    #[test]
    fn import_vault_backup_replaces_session_contents() {
        let (_temp_dir, mut facade, password) = unlocked_facade();
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
                    secret_string_from_seed("vault-facade-secondary"),
                    TotpConfig::default(),
                )
                .expect("backup account should be valid"),
            )
            .expect("backup account should persist");
        backup_repository
            .export_vault_file(&backup_destination)
            .expect("backup export should succeed");

        add_account(&mut facade, "GitHub", "user@example.com");
        let imported = facade
            .import_vault_backup(&backup_destination)
            .expect("vault import should succeed");

        assert_eq!(imported, 1);
        assert_eq!(facade.accounts().len(), 1);
        assert_eq!(facade.accounts()[0].service, "GitLab");
    }

    #[test]
    fn session_snapshot_matches_current_unlocked_state() {
        let (_temp_dir, mut facade, _password) = unlocked_facade();
        add_account(&mut facade, "GitHub", "user@example.com");
        facade
            .create_directory("ClientA/Auth")
            .expect("directory should persist");

        let snapshot = facade
            .session_snapshot()
            .expect("session snapshot should exist");

        assert_eq!(snapshot.accounts.len(), 1);
        assert_eq!(snapshot.directories.len(), 1);
        assert_eq!(snapshot.directories[0].path, "ClientA/Auth");
    }

    #[test]
    fn export_vault_backup_copies_the_encrypted_vault() {
        let (_temp_dir, mut facade, _password) = unlocked_facade();
        let export_dir = TempDir::new().expect("export dir should exist");
        let destination = export_dir.path().join("vault-export.json");

        add_account(&mut facade, "GitHub", "user@example.com");
        facade
            .export_vault_backup(&destination)
            .expect("vault export should succeed");

        let exported = std::fs::read_to_string(destination).expect("export should be readable");
        assert!(!exported.contains(&base32_secret_from_seed("vault-facade-primary")));
        assert!(!exported.contains(facade.path_display()));
    }
}

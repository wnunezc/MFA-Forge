use secrecy::SecretString;
use uuid::Uuid;

use mfa_forge_core::{AccountHistoryEntryPublic, AccountPublic, TotpConfig, TotpToken};

use crate::vault::VaultFacade;

/// Wrapper del adaptador GUI/stdio sobre la sesión reutilizable de aplicación.
pub struct AgentSession {
    inner: mfa_forge_application::session::AgentSession,
}

impl AgentSession {
    pub fn new(vault: VaultFacade) -> Self {
        Self {
            inner: mfa_forge_application::session::AgentSession::new(vault),
        }
    }

    pub fn path_display(&self) -> &str {
        self.inner.path_display()
    }

    pub fn is_unlocked(&self) -> bool {
        self.inner.is_unlocked()
    }

    pub fn account_count(&self) -> usize {
        self.inner.account_count()
    }

    pub fn list_accounts(&self) -> Vec<AccountPublic> {
        self.inner.list_accounts()
    }

    pub fn account_by_id(&self, account_id: Uuid) -> Option<AccountPublic> {
        self.inner.account_by_id(account_id)
    }

    pub fn generate_token(&self, account_id: Uuid) -> Result<TotpToken, String> {
        self.inner.generate_token(account_id)
    }

    pub fn add_account(
        &mut self,
        service: String,
        user: String,
        secret: String,
        totp: TotpConfig,
    ) -> Result<AccountPublic, String> {
        self.inner.add_account(service, user, secret, totp)
    }

    pub fn import_otpauth(&mut self, uri: &str) -> Result<AccountPublic, String> {
        self.inner.import_otpauth(uri)
    }

    pub fn update_account(
        &mut self,
        account_id: Uuid,
        service: Option<String>,
        user: Option<String>,
        secret: Option<String>,
        totp: Option<TotpConfig>,
    ) -> Result<AccountPublic, String> {
        self.inner
            .update_account(account_id, service, user, secret, totp)
    }

    pub fn remove_account(&mut self, account_id: Uuid) -> Result<AccountPublic, String> {
        self.inner.remove_account(account_id)
    }

    pub fn export_metadata(&self) -> Result<Vec<AccountPublic>, String> {
        self.inner.export_metadata()
    }

    pub fn history(&self) -> Result<Vec<AccountHistoryEntryPublic>, String> {
        self.inner.history()
    }

    pub fn change_master_password(&mut self, new_password: SecretString) -> Result<(), String> {
        self.inner.change_master_password(new_password)
    }

    pub fn close(&mut self) {
        self.inner.close();
    }
}

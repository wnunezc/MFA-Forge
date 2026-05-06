use secrecy::SecretString;
use uuid::Uuid;

use mfa_forge_core::{AccountHistoryEntryPublic, AccountPublic, TotpConfig, TotpToken};

use crate::vault::VaultFacade;

/// Mantiene una sesion local ya desbloqueada para exponer operaciones controladas a integraciones.
pub struct AgentSession {
    vault: VaultFacade,
}

impl AgentSession {
    /// Crea una nueva sesion basada en un vault ya desbloqueado.
    pub fn new(vault: VaultFacade) -> Self {
        Self { vault }
    }

    /// Devuelve la ruta del vault activo para diagnostico local.
    pub fn path_display(&self) -> &str {
        self.vault.path_display()
    }

    /// Indica si la sesion sigue desbloqueada.
    pub fn is_unlocked(&self) -> bool {
        self.vault.is_unlocked()
    }

    /// Devuelve la cantidad de cuentas disponibles en la sesion.
    pub fn account_count(&self) -> usize {
        self.vault.accounts().len()
    }

    /// Lista la metadata publica disponible para la sesion actual.
    pub fn list_accounts(&self) -> Vec<AccountPublic> {
        self.vault.accounts().to_vec()
    }

    /// Busca una cuenta visible por identificador dentro de la sesion actual.
    pub fn account_by_id(&self, account_id: Uuid) -> Option<AccountPublic> {
        self.vault.account_by_id(account_id)
    }

    /// Genera el TOTP actual para una cuenta ya existente.
    pub fn generate_token(&self, account_id: Uuid) -> Result<TotpToken, String> {
        let account = self.resolve_account(account_id)?;
        self.vault.generate_token(&account)
    }

    /// Agrega una nueva cuenta TOTP a la sesion actual.
    pub fn add_account(
        &mut self,
        service: String,
        user: String,
        secret: String,
        totp: TotpConfig,
    ) -> Result<AccountPublic, String> {
        self.vault
            .add_account(service, user, SecretString::from(secret), totp)
    }

    /// Importa una cuenta desde un URI `otpauth://`.
    pub fn import_otpauth(&mut self, uri: &str) -> Result<AccountPublic, String> {
        self.vault.import_otpauth(uri.trim())
    }

    /// Actualiza una cuenta existente sin exponer el secreto actual si no se reemplaza.
    pub fn update_account(
        &mut self,
        account_id: Uuid,
        service: Option<String>,
        user: Option<String>,
        secret: Option<String>,
        totp: Option<TotpConfig>,
    ) -> Result<AccountPublic, String> {
        let existing = self.resolve_account(account_id)?;
        self.vault.update_account(
            &existing,
            service.unwrap_or_else(|| existing.service.clone()),
            user.unwrap_or_else(|| existing.user.clone()),
            secret.map(SecretString::from),
            totp.unwrap_or_else(|| existing.totp.clone()),
        )
    }

    /// Elimina una cuenta existente de la sesion actual.
    pub fn remove_account(&mut self, account_id: Uuid) -> Result<AccountPublic, String> {
        let existing = self.resolve_account(account_id)?;
        self.vault.remove_account(&existing)
    }

    /// Exporta metadata publica del vault actual.
    pub fn export_metadata(&self) -> Result<Vec<AccountPublic>, String> {
        self.vault.export_metadata()
    }

    /// Devuelve el historial publico de cambios y restauraciones.
    pub fn history(&self) -> Result<Vec<AccountHistoryEntryPublic>, String> {
        self.vault.history()
    }

    /// Rota la contraseña maestra reutilizando la sesion ya desbloqueada.
    pub fn change_master_password(&mut self, new_password: SecretString) -> Result<(), String> {
        self.vault.change_master_password(new_password)
    }

    /// Cierra la sesion actual y borra el material sensible en memoria.
    pub fn close(&mut self) {
        self.vault.lock();
    }

    fn resolve_account(&self, account_id: Uuid) -> Result<AccountPublic, String> {
        self.vault
            .account_by_id(account_id)
            .ok_or_else(|| format!("No se encontró una cuenta con id {account_id}."))
    }
}

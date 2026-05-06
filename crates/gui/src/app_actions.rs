use std::path::PathBuf;

use eframe::egui;
use rfd::FileDialog;
use secrecy::SecretString;
use zeroize::Zeroize;

use mfa_forge_core::AccountPublic;
use uuid::Uuid;

use crate::{
    app::{ForgeApp, GuiPendingVaultJob, PendingVaultJobKind},
    app_tasks,
    state::{BannerTone, LoaderMode, Screen, WorkspaceScope},
};

impl ForgeApp {
    pub fn initialize_vault(&mut self) {
        let mut password = std::mem::take(&mut self.state.loader.password_input);
        let mut confirmation = std::mem::take(&mut self.state.loader.confirm_password_input);

        if password.trim().is_empty() {
            self.state.loader.error =
                Some("La contraseña maestra no puede estar vacía.".to_owned());
            confirmation.zeroize();
            return;
        }

        if password != confirmation {
            self.state.loader.error = Some("La confirmación no coincide.".to_owned());
            password.zeroize();
            confirmation.zeroize();
            return;
        }

        confirmation.zeroize();

        match self
            .vault
            .initialize_and_unlock(SecretString::from(std::mem::take(&mut password)))
        {
            Ok(()) => {
                self.state.loader.error = None;
                self.state.screen = Screen::Main;
                self.state.loader.mode = LoaderMode::Unlock;
                self.sync_selection();
                self.set_banner(
                    BannerTone::Success,
                    "Vault inicializado y desbloqueado. Ya puedes empezar a cargar cuentas MFA.",
                );
            }
            Err(error) => {
                self.state.loader.error = Some(error);
            }
        }
    }

    pub fn lock_vault(&mut self) {
        self.vault.lock();
        self.pending_prepare = None;
        self.pending_unlock = None;
        self.pending_vault_job = None;
        self.pending_history_job = None;
        self.pending_token_job = None;
        self.pending_search_job = None;
        self.state.screen = Screen::Loader;
        self.state.search_query.clear();
        self.state.search.clear();
        self.state.workspace_scope = WorkspaceScope::Unassigned;
        self.state.selected_account_id = None;
        self.state.checked_account_ids.clear();
        self.state.token_dialog.close();
        self.state.export_dialog.close();
        self.state.remove_dialog.clear();
        self.state.add_dialog.clear();
        self.state.edit_dialog.clear();
        self.state.import_dialog.clear();
        self.state.import_qr_dialog.clear();
        self.state.import_file_dialog.clear();
        self.state.restore_dialog.clear();
        self.state.create_directory_dialog.clear();
        self.state.change_password_dialog.clear();
        self.state.remove_directory_dialog.clear();
        self.state.account_uri_dialog.close();
        self.state.notice_dialog.close();
        self.set_banner(
            BannerTone::Info,
            "Sesión bloqueada. La contraseña maestra vuelve a ser requerida.",
        );
    }

    fn start_vault_job(
        &mut self,
        kind: PendingVaultJobKind,
        task: app_tasks::PendingTask<app_tasks::VaultJobResult>,
        busy_message: &str,
    ) -> bool {
        if self.has_background_vault_work() {
            self.set_banner(BannerTone::Info, busy_message);
            return false;
        }

        self.pending_vault_job = Some(GuiPendingVaultJob { kind, task });
        true
    }

    fn token_password_snapshot(&mut self) -> Option<SecretString> {
        match self.vault.password_snapshot() {
            Ok(password) => Some(password),
            Err(error) => {
                self.state.token_dialog.pending = false;
                self.state.token_dialog.error = Some(error);
                None
            }
        }
    }

    pub fn open_add_dialog(&mut self) {
        let selected_directory_path = self.selected_directory_path().map(str::to_owned);
        self.state.add_dialog.clear();
        self.state.add_dialog.open = true;
        self.state
            .add_dialog
            .form
            .metadata
            .set_project_path(selected_directory_path.as_deref());
    }

    pub fn open_add_dialog_for_directory(&mut self, directory_path: Option<String>) {
        self.state.workspace_scope = match directory_path {
            Some(path) => WorkspaceScope::Directory(path),
            None => WorkspaceScope::Unassigned,
        };
        self.open_add_dialog();
    }

    pub fn submit_add_dialog(&mut self) {
        let config = match self.state.add_dialog.form.totp_config() {
            Ok(config) => config,
            Err(error) => {
                self.state.add_dialog.error = Some(error);
                return;
            }
        };
        let metadata = self.state.add_dialog.form.metadata.to_metadata();
        let secret = SecretString::from(std::mem::take(&mut self.state.add_dialog.form.secret));
        let service = self.state.add_dialog.form.service.trim().to_owned();
        let user = self.state.add_dialog.form.user.trim().to_owned();

        match self
            .vault
            .add_account_with_metadata(service, user, secret, config, metadata)
        {
            Ok(account) => {
                self.invalidate_search();
                self.state.add_dialog.clear();
                self.state.selected_account_id = Some(account.id);
                self.sync_selection();
                self.set_banner(
                    BannerTone::Success,
                    format!("Cuenta {} agregada al vault.", account.display_name()),
                );
            }
            Err(error) => {
                self.state.add_dialog.error = Some(error);
            }
        }
    }

    pub fn open_import_dialog(&mut self) {
        let selected_directory_path = self.selected_directory_path().map(str::to_owned);
        self.state.import_dialog.clear();
        self.state.import_dialog.open = true;
        self.state
            .import_dialog
            .metadata
            .set_project_path(selected_directory_path.as_deref());
    }

    pub fn submit_import_dialog(&mut self) {
        if self.state.import_dialog.pending {
            return;
        }

        let uri = self.state.import_dialog.uri.trim().to_owned();
        if uri.is_empty() {
            self.state.import_dialog.error =
                Some("Pega un URI otpauth:// antes de importar.".to_owned());
            return;
        }

        let mut metadata = self.state.import_dialog.metadata.to_metadata();
        apply_default_source(&mut metadata, "manual_otpauth");
        if self.has_background_vault_work() {
            self.set_banner(
                BannerTone::Info,
                "Ya hay otra operación del vault en curso.",
            );
            return;
        }
        let password = match self.vault.password_snapshot() {
            Ok(password) => password,
            Err(error) => {
                self.state.import_dialog.error = Some(error);
                return;
            }
        };

        self.state.import_dialog.error = None;
        self.state.import_dialog.pending = true;
        if self.start_vault_job(
            PendingVaultJobKind::ImportUri,
            app_tasks::spawn_import_uri_job(password, uri, metadata),
            "Ya hay otra operación del vault en curso.",
        ) {
            self.set_banner(
                BannerTone::Info,
                "Importando cuenta desde URI sin bloquear la interfaz.",
            );
        }
    }

    pub fn open_import_qr_dialog(&mut self) {
        let selected_directory_path = self.selected_directory_path().map(str::to_owned);
        self.state.import_qr_dialog.clear();
        self.state.import_qr_dialog.open = true;
        self.state
            .import_qr_dialog
            .metadata
            .set_project_path(selected_directory_path.as_deref());
    }

    pub fn browse_import_qr_image(&mut self) {
        if let Some(path) = FileDialog::new()
            .add_filter("Imágenes", &["png", "jpg", "jpeg", "bmp"])
            .pick_file()
        {
            self.state.import_qr_dialog.image_path = path.display().to_string();
        }
    }

    pub fn submit_import_qr_dialog(&mut self) {
        if self.state.import_qr_dialog.pending {
            return;
        }

        let path = PathBuf::from(self.state.import_qr_dialog.image_path.trim());
        if path.as_os_str().is_empty() {
            self.state.import_qr_dialog.error =
                Some("Selecciona una imagen QR antes de importar.".to_owned());
            return;
        }

        let mut metadata = self.state.import_qr_dialog.metadata.to_metadata();
        apply_default_source(&mut metadata, "qr_import");
        if self.has_background_vault_work() {
            self.set_banner(
                BannerTone::Info,
                "Ya hay otra operación del vault en curso.",
            );
            return;
        }
        let password = match self.vault.password_snapshot() {
            Ok(password) => password,
            Err(error) => {
                self.state.import_qr_dialog.error = Some(error);
                return;
            }
        };

        self.state.import_qr_dialog.error = None;
        self.state.import_qr_dialog.pending = true;
        if self.start_vault_job(
            PendingVaultJobKind::ImportQr,
            app_tasks::spawn_import_qr_job(password, path, metadata),
            "Ya hay otra operación del vault en curso.",
        ) {
            self.set_banner(
                BannerTone::Info,
                "Importando cuenta desde QR sin bloquear la interfaz.",
            );
        }
    }

    pub fn open_import_file_dialog(&mut self) {
        let selected_directory_path = self.selected_directory_path().map(str::to_owned);
        self.state.import_file_dialog.clear();
        self.state.import_file_dialog.open = true;
        self.state
            .import_file_dialog
            .metadata
            .set_project_path(selected_directory_path.as_deref());
    }

    pub fn browse_import_file_dialog(&mut self) {
        if let Some(path) = FileDialog::new()
            .add_filter("Cuenta compatible", &["otpauth", "txt"])
            .pick_file()
        {
            self.state.import_file_dialog.file_path = path.display().to_string();
        }
    }

    pub fn submit_import_file_dialog(&mut self) {
        if self.state.import_file_dialog.pending {
            return;
        }

        let file_path = PathBuf::from(self.state.import_file_dialog.file_path.trim());
        if file_path.as_os_str().is_empty() {
            self.state.import_file_dialog.error =
                Some("Selecciona un archivo compatible antes de importar.".to_owned());
            return;
        }

        let mut metadata = self.state.import_file_dialog.metadata.to_metadata();
        apply_default_source(&mut metadata, "file_import");
        if self.has_background_vault_work() {
            self.set_banner(
                BannerTone::Info,
                "Ya hay otra operación del vault en curso.",
            );
            return;
        }
        let password = match self.vault.password_snapshot() {
            Ok(password) => password,
            Err(error) => {
                self.state.import_file_dialog.error = Some(error);
                return;
            }
        };

        self.state.import_file_dialog.error = None;
        self.state.import_file_dialog.pending = true;
        if self.start_vault_job(
            PendingVaultJobKind::ImportFile,
            app_tasks::spawn_import_file_job(password, file_path, metadata),
            "Ya hay otra operación del vault en curso.",
        ) {
            self.set_banner(
                BannerTone::Info,
                "Importando cuenta desde archivo sin bloquear la interfaz.",
            );
        }
    }

    pub fn open_edit_dialog(&mut self) {
        let Some(account) = self.selected_account() else {
            self.set_banner(
                BannerTone::Warning,
                "Selecciona una cuenta antes de intentar editarla.",
            );
            return;
        };

        self.state.edit_dialog.load_from_account(&account);
    }

    pub fn submit_edit_dialog(&mut self) {
        let Some(account_id) = self.state.edit_dialog.account_id else {
            self.state.edit_dialog.error =
                Some("No se encontró la cuenta seleccionada para editar.".to_owned());
            return;
        };
        let Some(existing) = self
            .vault
            .accounts()
            .iter()
            .find(|account| account.id == account_id)
            .cloned()
        else {
            self.state.edit_dialog.error =
                Some("La cuenta ya no está disponible en la sesión actual.".to_owned());
            return;
        };
        let config = match self.state.edit_dialog.form.totp_config() {
            Ok(config) => config,
            Err(error) => {
                self.state.edit_dialog.error = Some(error);
                return;
            }
        };
        let metadata = self.state.edit_dialog.form.metadata.to_metadata();
        let new_secret = {
            let secret = std::mem::take(&mut self.state.edit_dialog.form.secret);
            if secret.trim().is_empty() {
                None
            } else {
                Some(SecretString::from(secret))
            }
        };
        let service = self.state.edit_dialog.form.service.trim().to_owned();
        let user = self.state.edit_dialog.form.user.trim().to_owned();

        match self
            .vault
            .update_account_with_metadata(&existing, service, user, new_secret, config, metadata)
        {
            Ok(updated) => {
                self.invalidate_search();
                self.state.edit_dialog.clear();
                self.state.selected_account_id = Some(updated.id);
                self.sync_selection();
                self.set_banner(
                    BannerTone::Success,
                    format!("Cuenta {} actualizada.", updated.display_name()),
                );
            }
            Err(error) => {
                self.state.edit_dialog.error = Some(error);
            }
        }
    }

    pub fn open_restore_dialog(&mut self) {
        if self.pending_history_job.is_some() {
            return;
        }

        if self.has_background_vault_work() {
            self.set_banner(
                BannerTone::Info,
                "Ya hay otra operación del vault en curso.",
            );
            return;
        }

        let password = match self.vault.password_snapshot() {
            Ok(password) => password,
            Err(error) => {
                self.set_banner(BannerTone::Error, error);
                return;
            }
        };

        self.state
            .restore_dialog
            .begin_pending("Cargando historial restaurable...");
        self.pending_history_job = Some(app_tasks::spawn_load_history_job(password));
        self.set_banner(
            BannerTone::Info,
            "Cargando historial restaurable sin bloquear la interfaz.",
        );
    }

    pub fn restore_selected_history_entry(&mut self) {
        if self.state.restore_dialog.pending || self.pending_history_job.is_some() {
            return;
        }
        let Some(entry_id) = self.state.restore_dialog.selected_entry_id else {
            self.state.restore_dialog.error =
                Some("Selecciona una versión del historial antes de restaurar.".to_owned());
            return;
        };

        let password = match self.vault.password_snapshot() {
            Ok(password) => password,
            Err(error) => {
                self.state.restore_dialog.error = Some(error);
                return;
            }
        };

        self.state.restore_dialog.pending = true;
        self.state.restore_dialog.pending_message =
            Some("Restaurando la versión seleccionada...".to_owned());
        self.state.restore_dialog.error = None;
        self.pending_history_job = Some(app_tasks::spawn_restore_history_entry_job(
            password, entry_id,
        ));
        self.set_banner(
            BannerTone::Info,
            "Restaurando desde historial sin bloquear la interfaz.",
        );
    }

    pub fn open_change_password_dialog(&mut self) {
        self.state.change_password_dialog.clear();
        self.state.change_password_dialog.open = true;
    }

    pub fn open_create_directory_dialog(&mut self, parent_path: Option<String>) {
        self.state.create_directory_dialog.clear();
        self.state.create_directory_dialog.open = true;
        self.state.create_directory_dialog.parent_path =
            parent_path.or_else(|| self.selected_directory_path().map(ToOwned::to_owned));
    }

    pub fn open_remove_directory_dialog(&mut self, path: String) {
        self.state.remove_directory_dialog.load_path(path);
    }

    pub fn submit_create_directory_dialog(&mut self) {
        if self.state.create_directory_dialog.pending {
            return;
        }

        let name = std::mem::take(&mut self.state.create_directory_dialog.name);
        let parent_path = self.state.create_directory_dialog.parent_path.clone();
        let normalized_name = name.trim().replace('\\', "/");

        if normalized_name.is_empty() {
            self.state.create_directory_dialog.error =
                Some("El nombre del directorio no puede estar vacío.".to_owned());
            return;
        }

        let full_path = match parent_path.as_deref() {
            Some(parent) if !parent.trim().is_empty() => format!("{parent}/{normalized_name}"),
            _ => normalized_name,
        };

        if self.has_background_vault_work() {
            self.state.create_directory_dialog.error =
                Some("Ya hay otra operación del vault en curso.".to_owned());
            self.state.create_directory_dialog.name = name;
            self.state.create_directory_dialog.parent_path = parent_path;
            return;
        }

        let password = match self.vault.password_snapshot() {
            Ok(password) => password,
            Err(error) => {
                self.state.create_directory_dialog.error = Some(error);
                self.state.create_directory_dialog.name = name;
                self.state.create_directory_dialog.parent_path = parent_path;
                return;
            }
        };

        self.state.create_directory_dialog.error = None;
        self.state.create_directory_dialog.pending = true;
        self.state.create_directory_dialog.name = name;
        self.state.create_directory_dialog.parent_path = parent_path;

        if self.start_vault_job(
            PendingVaultJobKind::CreateDirectory,
            app_tasks::spawn_create_directory_job(password, full_path),
            "Ya hay otra operación del vault en curso.",
        ) {
            self.set_banner(
                BannerTone::Info,
                "Creando workspace sin bloquear la interfaz.",
            );
        }
    }

    pub fn submit_change_password_dialog(&mut self) {
        let mut new_password = std::mem::take(&mut self.state.change_password_dialog.new_password);
        let mut confirmation =
            std::mem::take(&mut self.state.change_password_dialog.confirm_password);

        if new_password.trim().is_empty() {
            self.state.change_password_dialog.error =
                Some("La nueva contraseña maestra no puede estar vacía.".to_owned());
            confirmation.zeroize();
            return;
        }

        if new_password != confirmation {
            self.state.change_password_dialog.error =
                Some("La confirmación de la nueva contraseña no coincide.".to_owned());
            new_password.zeroize();
            confirmation.zeroize();
            return;
        }

        confirmation.zeroize();

        match self
            .vault
            .change_master_password(SecretString::from(std::mem::take(&mut new_password)))
        {
            Ok(()) => {
                self.state.change_password_dialog.clear();
                self.set_banner(
                    BannerTone::Success,
                    "Contraseña maestra rotada y vault re-cifrado con éxito.",
                );
            }
            Err(error) => {
                self.state.change_password_dialog.error = Some(error);
            }
        }
    }

    pub fn open_remove_dialog(&mut self) {
        if self.selected_account().is_some() {
            let accounts = self.selected_account().into_iter().collect::<Vec<_>>();
            self.state.remove_dialog.load_accounts(&accounts);
        } else {
            self.set_banner(
                BannerTone::Warning,
                "Selecciona una cuenta antes de intentar eliminarla.",
            );
        }
    }

    pub fn open_remove_checked_accounts_dialog(&mut self) {
        let accounts = self.checked_accounts();
        if accounts.is_empty() {
            self.set_banner(
                BannerTone::Warning,
                "Marca al menos una cuenta antes de intentar eliminar varias.",
            );
            return;
        }

        self.state.remove_dialog.load_accounts(&accounts);
    }

    pub fn confirm_remove_selected(&mut self) {
        if self.state.remove_dialog.pending {
            return;
        }

        let account_ids = self.state.remove_dialog.account_ids.clone();
        if account_ids.is_empty() {
            self.state.remove_dialog.clear();
            return;
        }

        if self.has_background_vault_work() {
            self.state.remove_dialog.error =
                Some("Ya hay otra operación del vault en curso.".to_owned());
            return;
        }

        let password = match self.vault.password_snapshot() {
            Ok(password) => password,
            Err(error) => {
                self.state.remove_dialog.error = Some(error);
                return;
            }
        };

        self.state.remove_dialog.error = None;
        self.state.remove_dialog.pending = true;

        let job_started = if account_ids.len() == 1 {
            let Some(account) = self.vault.account_by_id(account_ids[0]) else {
                self.state.remove_dialog.error =
                    Some("La cuenta seleccionada ya no está disponible.".to_owned());
                self.state.remove_dialog.pending = false;
                return;
            };

            self.start_vault_job(
                PendingVaultJobKind::RemoveAccount,
                app_tasks::spawn_remove_account_job(password, account),
                "Ya hay otra operación del vault en curso.",
            )
        } else {
            let selected_ids = account_ids
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>();
            let accounts = self
                .vault
                .accounts()
                .iter()
                .filter(|account| selected_ids.contains(&account.id))
                .cloned()
                .collect::<Vec<_>>();

            if accounts.len() != account_ids.len() {
                self.state.remove_dialog.error =
                    Some("Una o más cuentas seleccionadas ya no están disponibles.".to_owned());
                self.state.remove_dialog.pending = false;
                return;
            }

            self.start_vault_job(
                PendingVaultJobKind::RemoveAccounts,
                app_tasks::spawn_remove_accounts_job(password, accounts),
                "Ya hay otra operación del vault en curso.",
            )
        };

        if job_started {
            self.set_banner(
                BannerTone::Info,
                if account_ids.len() == 1 {
                    "Eliminando cuenta sin bloquear la interfaz."
                } else {
                    "Eliminando cuentas seleccionadas sin bloquear la interfaz."
                },
            );
        }
    }

    pub fn confirm_remove_selected_directory(&mut self) {
        if self.state.remove_directory_dialog.pending {
            return;
        }

        let path = self.state.remove_directory_dialog.path.trim().to_owned();
        if path.is_empty() {
            self.state.remove_directory_dialog.clear();
            return;
        }

        if self.has_background_vault_work() {
            self.state.remove_directory_dialog.error =
                Some("Ya hay otra operación del vault en curso.".to_owned());
            return;
        }

        let password = match self.vault.password_snapshot() {
            Ok(password) => password,
            Err(error) => {
                self.state.remove_directory_dialog.error = Some(error);
                return;
            }
        };

        self.state.remove_directory_dialog.error = None;
        self.state.remove_directory_dialog.pending = true;
        if self.start_vault_job(
            PendingVaultJobKind::DeleteDirectory,
            app_tasks::spawn_delete_directory_job(password, path),
            "Ya hay otra operación del vault en curso.",
        ) {
            self.set_banner(
                BannerTone::Info,
                "Eliminando workspace vacío sin bloquear la interfaz.",
            );
        }
    }

    pub fn set_primary_account_selection(&mut self, account_id: Uuid) {
        self.state.selected_account_id = Some(account_id);
    }

    pub fn toggle_account_checked(&mut self, account_id: Uuid, checked: bool) {
        if checked {
            self.state.checked_account_ids.insert(account_id);
            self.state.selected_account_id = Some(account_id);
        } else {
            self.state.checked_account_ids.remove(&account_id);
        }
    }

    pub fn open_token_dialog(&mut self) {
        let Some(account) = self.selected_account() else {
            self.set_banner(
                BannerTone::Warning,
                "Selecciona una cuenta antes de generar un token.",
            );
            return;
        };

        self.state.token_dialog.open = true;
        self.state.token_dialog.refresh_count = 0;
        self.state.token_dialog.error = None;
        self.state.token_dialog.pending = true;
        self.state.token_dialog.action_message =
            Some("Calculando token en segundo plano...".to_owned());
        self.state.token_dialog.action_tone = Some(BannerTone::Info);
        self.request_token_for(account, None);
    }

    pub fn refresh_token_for_selected(&mut self) {
        let Some(account) = self.selected_account() else {
            return;
        };
        self.state.token_dialog.pending = true;
        self.state.token_dialog.refresh_count =
            self.state.token_dialog.refresh_count.saturating_add(1);
        self.state.token_dialog.action_message = Some(format!(
            "Refrescando token (#{}).",
            self.state.token_dialog.refresh_count
        ));
        self.state.token_dialog.action_tone = Some(BannerTone::Info);
        self.request_token_for(account, self.state.token_dialog.token.clone());
    }

    pub(crate) fn request_token_for(
        &mut self,
        account: AccountPublic,
        previous_token: Option<mfa_forge_core::TotpToken>,
    ) {
        if self.pending_token_job.is_some() {
            return;
        }

        let Some(password) = self.token_password_snapshot() else {
            return;
        };

        self.pending_token_job = Some(app_tasks::spawn_token_job(
            password,
            account,
            previous_token,
        ));
    }

    pub fn copy_selected_token(&mut self, ctx: &egui::Context) {
        let Some(token) = self.selected_token() else {
            self.set_banner(BannerTone::Warning, "No hay un token visible para copiar.");
            return;
        };

        ctx.copy_text(token.code.clone());
        self.set_banner(
            BannerTone::Success,
            format!("Código TOTP copiado para {}.", token.service),
        );
    }

    pub fn open_export_dialog(&mut self) {
        self.state.export_dialog.open = true;
        self.state.export_dialog.error = None;
        self.state.export_dialog.pending = false;
    }

    pub fn export_vault_backup(&mut self) {
        if self.state.export_dialog.pending {
            return;
        }

        let Some(path) = FileDialog::new()
            .add_filter("Backup MFA-Forge", &["json", "bak"])
            .set_file_name("mfa-forge-vault-backup.json")
            .save_file()
        else {
            return;
        };

        if self.has_background_vault_work() {
            self.state.export_dialog.error =
                Some("Ya hay otra operación del vault en curso.".to_owned());
            return;
        }

        self.state.export_dialog.error = None;
        self.state.export_dialog.pending = true;
        if self.start_vault_job(
            PendingVaultJobKind::ExportVaultBackup,
            app_tasks::spawn_export_vault_backup_job(path),
            "Ya hay otra operación del vault en curso.",
        ) {
            self.set_banner(BannerTone::Info, "Exportando el backup cifrado del vault.");
        }
    }

    pub fn import_vault_backup(&mut self) {
        let Some(path) = FileDialog::new()
            .add_filter("Backup MFA-Forge", &["json", "bak"])
            .pick_file()
        else {
            return;
        };

        if self.has_background_vault_work() {
            self.set_banner(
                BannerTone::Info,
                "Ya hay otra operación del vault en curso.",
            );
            return;
        }

        let password = match self.vault.password_snapshot() {
            Ok(password) => password,
            Err(error) => {
                self.set_banner(BannerTone::Error, error);
                return;
            }
        };

        if self.start_vault_job(
            PendingVaultJobKind::ImportVaultBackup,
            app_tasks::spawn_import_vault_backup_job(password, path),
            "Ya hay otra operación del vault en curso.",
        ) {
            self.set_banner(
                BannerTone::Info,
                "Validando e importando el backup del vault en segundo plano.",
            );
        }
    }

    pub fn export_selected_account_to_file(&mut self) {
        let Some(account) = self.selected_account() else {
            self.set_banner(
                BannerTone::Warning,
                "Selecciona una cuenta antes de exportarla.",
            );
            return;
        };

        let suggested_name = format!(
            "{}-{}.otpauth",
            sanitize_file_stem(&account.service),
            sanitize_file_stem(&account.user),
        );
        let Some(path) = FileDialog::new()
            .add_filter("Cuenta compatible", &["otpauth", "txt"])
            .set_file_name(&suggested_name)
            .save_file()
        else {
            return;
        };

        if self.has_background_vault_work() {
            self.set_banner(
                BannerTone::Info,
                "Ya hay otra operación del vault en curso.",
            );
            return;
        }

        let password = match self.vault.password_snapshot() {
            Ok(password) => password,
            Err(error) => {
                self.set_banner(BannerTone::Error, error);
                return;
            }
        };

        if self.start_vault_job(
            PendingVaultJobKind::ExportAccountFile,
            app_tasks::spawn_export_account_file_job(password, account, path),
            "Ya hay otra operación del vault en curso.",
        ) {
            self.set_banner(
                BannerTone::Info,
                "Exportando la cuenta seleccionada a un archivo compatible.",
            );
        }
    }

    pub fn export_selected_account_qr(&mut self) {
        let Some(account) = self.selected_account() else {
            self.set_banner(
                BannerTone::Warning,
                "Selecciona una cuenta antes de exportar su QR.",
            );
            return;
        };

        let suggested_name = format!(
            "{}-{}.png",
            sanitize_file_stem(&account.service),
            sanitize_file_stem(&account.user),
        );
        let Some(path) = FileDialog::new()
            .add_filter("QR PNG", &["png"])
            .set_file_name(&suggested_name)
            .save_file()
        else {
            return;
        };

        if self.has_background_vault_work() {
            self.set_banner(
                BannerTone::Info,
                "Ya hay otra operación del vault en curso.",
            );
            return;
        }

        let password = match self.vault.password_snapshot() {
            Ok(password) => password,
            Err(error) => {
                self.set_banner(BannerTone::Error, error);
                return;
            }
        };

        if self.start_vault_job(
            PendingVaultJobKind::ExportAccountQr,
            app_tasks::spawn_export_account_qr_job(password, account, path),
            "Ya hay otra operación del vault en curso.",
        ) {
            self.set_banner(
                BannerTone::Info,
                "Generando el QR de la cuenta seleccionada.",
            );
        }
    }

    pub fn export_selected_account_uri(&mut self) {
        let Some(account) = self.selected_account() else {
            self.set_banner(
                BannerTone::Warning,
                "Selecciona una cuenta antes de exportarla como URI.",
            );
            return;
        };

        if self.has_background_vault_work() {
            self.state.account_uri_dialog.open = true;
            self.state.account_uri_dialog.error =
                Some("Ya hay otra operación del vault en curso.".to_owned());
            self.state.account_uri_dialog.pending = false;
            return;
        }

        let password = match self.vault.password_snapshot() {
            Ok(password) => password,
            Err(error) => {
                self.state.account_uri_dialog.error = Some(error);
                self.state.account_uri_dialog.open = true;
                return;
            }
        };

        self.state.account_uri_dialog.open = true;
        self.state.account_uri_dialog.pending = true;
        self.state.account_uri_dialog.error = None;
        self.state.account_uri_dialog.account_label = account.display_name();
        self.state.account_uri_dialog.uri.zeroize();
        self.state.account_uri_dialog.uri.clear();

        if self.start_vault_job(
            PendingVaultJobKind::ExportAccountUri,
            app_tasks::spawn_export_account_uri_job(password, account),
            "Ya hay otra operación del vault en curso.",
        ) {
            self.set_banner(
                BannerTone::Info,
                "Preparando la exportación explícita de la cuenta como URI.",
            );
        }
    }
}

fn apply_default_source(metadata: &mut mfa_forge_core::AccountMetadata, source: &str) {
    if metadata.source.is_none() {
        metadata.source = Some(source.to_owned());
    }
}

fn sanitize_file_stem(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();

    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "cuenta".to_owned()
    } else {
        trimmed.to_owned()
    }
}

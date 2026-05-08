use std::{env, fs, path::PathBuf, process::Command};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use directories::ProjectDirs;
use eframe::egui;
use rfd::FileDialog;
use secrecy::SecretString;
use zeroize::Zeroize;

use mfa_forge_core::AccountPublic;
use uuid::Uuid;

use crate::{
    app::{ForgeApp, GuiPendingVaultJob, PendingVaultJobKind},
    app_tasks,
    i18n::{tr, trf},
    state::{BannerTone, LoaderMode, Screen, WorkspaceScope},
};

impl ForgeApp {
    pub fn current_release_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    pub fn update_stage_directory(&self) -> Result<PathBuf, String> {
        release_update_stage_directory("manual")
    }

    pub fn open_update_dialog(&mut self) {
        self.state.update_dialog.open();
    }

    pub fn start_latest_rc_update(&mut self, automatic: bool) {
        if !automatic {
            self.state.update_dialog.error = None;
        }

        let launcher_path = match launcher_path_from_current_install() {
            Ok(path) => path,
            Err(error) => {
                if automatic {
                    self.set_banner(BannerTone::Warning, error);
                } else {
                    self.state.update_dialog.error = Some(error);
                }
                return;
            }
        };

        let stage_dir =
            match release_update_stage_directory(if automatic { "startup" } else { "manual" }) {
                Ok(path) => path,
                Err(error) => {
                    if automatic {
                        self.set_banner(BannerTone::Warning, error);
                    } else {
                        self.state.update_dialog.error = Some(error);
                    }
                    return;
                }
            };

        if let Err(error) = fs::create_dir_all(&stage_dir) {
            let message =
                format!("The local update staging directory could not be created: {error}");
            if automatic {
                self.set_banner(BannerTone::Warning, message);
            } else {
                self.state.update_dialog.error = Some(message);
            }
            return;
        }

        let report_path = stage_dir.join(if automatic {
            "launcher-auto-report.json"
        } else {
            "launcher-manual-report.json"
        });
        let mut command = Command::new(&launcher_path);
        command
            .arg("--repo")
            .arg("wnunezc/MFA-Forge")
            .arg("--current-version")
            .arg(self.current_release_version())
            .arg("--output-dir")
            .arg(&stage_dir)
            .arg("--report-path")
            .arg(&report_path)
            .arg("--apply");

        #[cfg(windows)]
        command.creation_flags(0x08000000);

        match command.spawn() {
            Ok(_) => {
                if automatic {
                    self.set_banner(
                        BannerTone::Info,
                        tr(
                            "Automatic RC update check started. If GitHub has a newer prerelease, the launcher will verify it and hand control to Windows Installer.",
                        ),
                    );
                } else {
                    self.state.update_dialog.close();
                    self.set_banner(
                        BannerTone::Info,
                        tr(
                            "Launcher started. It will check GitHub for a newer prerelease RC, verify the MSI checksum, and then open Windows Installer if an update exists.",
                        ),
                    );
                }
            }
            Err(error) => {
                let message = trf(
                    "The update launcher could not be started: {error}",
                    &[("error", &error.to_string())],
                );
                if automatic {
                    self.set_banner(BannerTone::Warning, message);
                } else {
                    self.state.update_dialog.error = Some(message);
                }
            }
        }
    }

    pub fn initialize_vault(&mut self) {
        let mut password = std::mem::take(&mut self.state.loader.password_input);
        let mut confirmation = std::mem::take(&mut self.state.loader.confirm_password_input);

        if password.trim().is_empty() {
            self.state.loader.error = Some(tr("The master password cannot be empty."));
            confirmation.zeroize();
            return;
        }

        if password != confirmation {
            self.state.loader.error = Some(tr("The confirmation does not match."));
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
                    tr("Vault initialized and unlocked. You can start adding MFA accounts."),
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
        self.state.update_dialog.close();
        self.startup_update_check_attempted = false;
        self.set_banner(
            BannerTone::Info,
            tr("Session locked. The master password is required again."),
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
                    trf(
                        "Account {name} added to the vault.",
                        &[("name", &account.display_name())],
                    ),
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
            self.state.import_dialog.error = Some(tr("Paste an otpauth:// URI before importing."));
            return;
        }

        let mut metadata = self.state.import_dialog.metadata.to_metadata();
        apply_default_source(&mut metadata, "manual_otpauth");
        if self.has_background_vault_work() {
            self.set_banner(
                BannerTone::Info,
                tr("Another vault operation is already in progress."),
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
            &tr("Another vault operation is already in progress."),
        ) {
            self.set_banner(
                BannerTone::Info,
                tr("Importing an account from a URI without blocking the interface."),
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
            .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
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
            self.state.import_qr_dialog.error = Some(tr("Select a QR image before importing."));
            return;
        }

        let mut metadata = self.state.import_qr_dialog.metadata.to_metadata();
        apply_default_source(&mut metadata, "qr_import");
        if self.has_background_vault_work() {
            self.set_banner(
                BannerTone::Info,
                tr("Another vault operation is already in progress."),
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
            &tr("Another vault operation is already in progress."),
        ) {
            self.set_banner(
                BannerTone::Info,
                tr("Importing an account from a QR image without blocking the interface."),
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
            .add_filter("Compatible account", &["otpauth", "txt"])
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
                Some(tr("Select a compatible file before importing."));
            return;
        }

        let mut metadata = self.state.import_file_dialog.metadata.to_metadata();
        apply_default_source(&mut metadata, "file_import");
        if self.has_background_vault_work() {
            self.set_banner(
                BannerTone::Info,
                tr("Another vault operation is already in progress."),
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
            &tr("Another vault operation is already in progress."),
        ) {
            self.set_banner(
                BannerTone::Info,
                tr("Importing an account from a file without blocking the interface."),
            );
        }
    }

    pub fn open_edit_dialog(&mut self) {
        let Some(account) = self.selected_account() else {
            self.set_banner(
                BannerTone::Warning,
                tr("Select an account before trying to edit it."),
            );
            return;
        };

        self.state.edit_dialog.load_from_account(&account);
    }

    pub fn submit_edit_dialog(&mut self) {
        let Some(account_id) = self.state.edit_dialog.account_id else {
            self.state.edit_dialog.error =
                Some(tr("The selected account could not be found for editing."));
            return;
        };
        let Some(existing) = self
            .vault
            .accounts()
            .iter()
            .find(|account| account.id == account_id)
            .cloned()
        else {
            self.state.edit_dialog.error = Some(tr(
                "The account is no longer available in the current session.",
            ));
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
                    trf(
                        "Account {name} updated.",
                        &[("name", &updated.display_name())],
                    ),
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
                tr("Another vault operation is already in progress."),
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
            .begin_pending(tr("Loading restorable history..."));
        self.pending_history_job = Some(app_tasks::spawn_load_history_job(password));
        self.set_banner(
            BannerTone::Info,
            tr("Loading restorable history without blocking the interface."),
        );
    }

    pub fn restore_selected_history_entry(&mut self) {
        if self.state.restore_dialog.pending || self.pending_history_job.is_some() {
            return;
        }
        let Some(entry_id) = self.state.restore_dialog.selected_entry_id else {
            self.state.restore_dialog.error =
                Some(tr("Select a history version before restoring."));
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
        self.state.restore_dialog.pending_message = Some(tr("Restoring the selected version..."));
        self.state.restore_dialog.error = None;
        self.pending_history_job = Some(app_tasks::spawn_restore_history_entry_job(
            password, entry_id,
        ));
        self.set_banner(
            BannerTone::Info,
            tr("Restoring from history without blocking the interface."),
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
                Some(tr("The directory name cannot be empty."));
            return;
        }

        let full_path = match parent_path.as_deref() {
            Some(parent) if !parent.trim().is_empty() => format!("{parent}/{normalized_name}"),
            _ => normalized_name,
        };

        if self.has_background_vault_work() {
            self.state.create_directory_dialog.error =
                Some(tr("Another vault operation is already in progress."));
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
            &tr("Another vault operation is already in progress."),
        ) {
            self.set_banner(
                BannerTone::Info,
                tr("Creating the workspace without blocking the interface."),
            );
        }
    }

    pub fn submit_change_password_dialog(&mut self) {
        let mut new_password = std::mem::take(&mut self.state.change_password_dialog.new_password);
        let mut confirmation =
            std::mem::take(&mut self.state.change_password_dialog.confirm_password);

        if new_password.trim().is_empty() {
            self.state.change_password_dialog.error =
                Some(tr("The new master password cannot be empty."));
            confirmation.zeroize();
            return;
        }

        if new_password != confirmation {
            self.state.change_password_dialog.error =
                Some(tr("The new password confirmation does not match."));
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
                    tr("Master password rotated and vault re-encrypted successfully."),
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
                tr("Select an account before trying to remove it."),
            );
        }
    }

    pub fn open_remove_checked_accounts_dialog(&mut self) {
        let accounts = self.checked_accounts();
        if accounts.is_empty() {
            self.set_banner(
                BannerTone::Warning,
                tr("Check at least one account before trying to remove multiple accounts."),
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
                Some(tr("Another vault operation is already in progress."));
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
                    Some(tr("The selected account is no longer available."));
                self.state.remove_dialog.pending = false;
                return;
            };

            self.start_vault_job(
                PendingVaultJobKind::RemoveAccount,
                app_tasks::spawn_remove_account_job(password, account),
                &tr("Another vault operation is already in progress."),
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
                    Some(tr("One or more selected accounts are no longer available."));
                self.state.remove_dialog.pending = false;
                return;
            }

            self.start_vault_job(
                PendingVaultJobKind::RemoveAccounts,
                app_tasks::spawn_remove_accounts_job(password, accounts),
                &tr("Another vault operation is already in progress."),
            )
        };

        if job_started {
            self.set_banner(
                BannerTone::Info,
                if account_ids.len() == 1 {
                    tr("Removing the account without blocking the interface.")
                } else {
                    tr("Removing the selected accounts without blocking the interface.")
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
                Some(tr("Another vault operation is already in progress."));
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
            &tr("Another vault operation is already in progress."),
        ) {
            self.set_banner(
                BannerTone::Info,
                tr("Removing the empty workspace without blocking the interface."),
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
                tr("Select an account before generating a token."),
            );
            return;
        };

        self.state.token_dialog.open = true;
        self.state.token_dialog.refresh_count = 0;
        self.state.token_dialog.error = None;
        self.state.token_dialog.pending = true;
        self.state.token_dialog.action_message =
            Some(tr("Calculating the token in the background..."));
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
        self.state.token_dialog.action_message = Some(trf(
            "Refreshing token (#{count}).",
            &[("count", &self.state.token_dialog.refresh_count.to_string())],
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
            self.set_banner(
                BannerTone::Warning,
                tr("There is no visible token to copy."),
            );
            return;
        };

        ctx.copy_text(token.code.clone());
        self.set_banner(
            BannerTone::Success,
            trf(
                "TOTP code copied for {service}.",
                &[("service", &token.service)],
            ),
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
                Some(tr("Another vault operation is already in progress."));
            return;
        }

        self.state.export_dialog.error = None;
        self.state.export_dialog.pending = true;
        if self.start_vault_job(
            PendingVaultJobKind::ExportVaultBackup,
            app_tasks::spawn_export_vault_backup_job(path),
            &tr("Another vault operation is already in progress."),
        ) {
            self.set_banner(
                BannerTone::Info,
                tr("Exporting the encrypted vault backup."),
            );
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
                tr("Another vault operation is already in progress."),
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
            &tr("Another vault operation is already in progress."),
        ) {
            self.set_banner(
                BannerTone::Info,
                tr("Validating and importing the vault backup in the background."),
            );
        }
    }

    pub fn export_selected_account_to_file(&mut self) {
        let Some(account) = self.selected_account() else {
            self.set_banner(
                BannerTone::Warning,
                tr("Select an account before exporting it."),
            );
            return;
        };

        let suggested_name = format!(
            "{}-{}.otpauth",
            sanitize_file_stem(&account.service),
            sanitize_file_stem(&account.user),
        );
        let Some(path) = FileDialog::new()
            .add_filter("Compatible account", &["otpauth", "txt"])
            .set_file_name(&suggested_name)
            .save_file()
        else {
            return;
        };

        if self.has_background_vault_work() {
            self.set_banner(
                BannerTone::Info,
                tr("Another vault operation is already in progress."),
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
            &tr("Another vault operation is already in progress."),
        ) {
            self.set_banner(
                BannerTone::Info,
                tr("Exporting the selected account to a compatible file."),
            );
        }
    }

    pub fn export_selected_account_qr(&mut self) {
        let Some(account) = self.selected_account() else {
            self.set_banner(
                BannerTone::Warning,
                tr("Select an account before exporting its QR."),
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
                tr("Another vault operation is already in progress."),
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
            &tr("Another vault operation is already in progress."),
        ) {
            self.set_banner(
                BannerTone::Info,
                tr("Generating the QR for the selected account."),
            );
        }
    }

    pub fn export_selected_account_uri(&mut self) {
        let Some(account) = self.selected_account() else {
            self.set_banner(
                BannerTone::Warning,
                tr("Select an account before exporting it as a URI."),
            );
            return;
        };

        if self.has_background_vault_work() {
            self.state.account_uri_dialog.open = true;
            self.state.account_uri_dialog.error =
                Some(tr("Another vault operation is already in progress."));
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
            &tr("Another vault operation is already in progress."),
        ) {
            self.set_banner(
                BannerTone::Info,
                tr("Preparing the explicit export of the account as a URI."),
            );
        }
    }
}

fn launcher_path_from_current_install() -> Result<PathBuf, String> {
    let launcher_path = env::current_exe()
        .map_err(|error| format!("The current GUI executable path could not be resolved: {error}"))?
        .with_file_name("mfa-forge-launcher.exe");

    if launcher_path.is_file() {
        Ok(launcher_path)
    } else {
        Err(trf(
            "The installed launcher could not be found at {path}. This build cannot start the launcher-driven path.",
            &[("path", &launcher_path.display().to_string())],
        ))
    }
}

fn release_update_stage_directory(channel: &str) -> Result<PathBuf, String> {
    let project_dirs = ProjectDirs::from("dev", "OpsZone", "MFA-Forge")
        .ok_or_else(|| "The MFA-Forge local data directory could not be resolved.".to_owned())?;
    Ok(project_dirs.data_local_dir().join("updates").join(channel))
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
        "account".to_owned()
    } else {
        trimmed.to_owned()
    }
}

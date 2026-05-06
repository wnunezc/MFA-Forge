use std::collections::HashSet;

use eframe::egui;

use mfa_forge_core::{AccountPublic, ProjectDirectory};

use crate::app_status::request_open_windows_repaint;
use crate::app_tasks::{
    HistoryTaskResult, PendingPoll, PendingTask, SearchTaskResult, TokenTaskResult, VaultJobResult,
};
use crate::app_unlock::{GuiPendingPrepareUnlock, GuiPendingUnlockFlow};
use crate::{
    dialogs, platform_auth,
    state::{AppState, BannerTone, Screen, WorkspaceScope},
    theme,
    vault::VaultFacade,
    views,
};

pub(crate) struct GuiPendingVaultJob {
    pub(crate) kind: PendingVaultJobKind,
    pub(crate) task: PendingTask<VaultJobResult>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PendingVaultJobKind {
    CreateDirectory,
    DeleteDirectory,
    RemoveAccount,
    RemoveAccounts,
    ImportUri,
    ImportQr,
    ImportFile,
    ImportVaultBackup,
    ExportVaultBackup,
    ExportAccountFile,
    ExportAccountQr,
    ExportAccountUri,
}

pub struct ForgeApp {
    pub(crate) owner_window: platform_auth::OwnerWindow,
    pub(crate) vault: VaultFacade,
    pub(crate) state: AppState,
    pub(crate) pending_prepare: Option<GuiPendingPrepareUnlock>,
    pub(crate) pending_unlock: Option<GuiPendingUnlockFlow>,
    pub(crate) pending_vault_job: Option<GuiPendingVaultJob>,
    pub(crate) pending_history_job: Option<PendingTask<HistoryTaskResult>>,
    pub(crate) pending_token_job: Option<PendingTask<TokenTaskResult>>,
    pub(crate) pending_search_job: Option<PendingTask<SearchTaskResult>>,
}

impl ForgeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let theme_preference = theme::load_preference();
        theme::apply(&cc.egui_ctx, theme_preference);

        let owner_window = platform_auth::capture_owner_window(cc)?;
        let vault = VaultFacade::try_new().map_err(|error| error.to_string())?;
        let state = AppState::new(vault.is_initialized(), theme_preference);

        Ok(Self {
            owner_window,
            vault,
            state,
            pending_prepare: None,
            pending_unlock: None,
            pending_vault_job: None,
            pending_history_job: None,
            pending_token_job: None,
            pending_search_job: None,
        })
    }

    pub fn vault_path(&self) -> &str {
        self.vault.path_display()
    }

    pub fn visible_accounts(&self) -> Vec<AccountPublic> {
        if let Some(query) = self.active_search_query() {
            if !self.state.search.is_active_for(&query) {
                return Vec::new();
            }

            let matched_ids = self
                .state
                .search
                .matched_account_ids
                .iter()
                .copied()
                .collect::<HashSet<_>>();

            return self
                .vault
                .accounts()
                .iter()
                .filter(|account| matched_ids.contains(&account.id))
                .cloned()
                .collect();
        }

        self.vault
            .accounts()
            .iter()
            .filter(|account| workspace_scope_matches(account, &self.state.workspace_scope))
            .cloned()
            .collect()
    }

    pub fn directories(&self) -> &[ProjectDirectory] {
        self.vault.directories()
    }

    pub fn accounts(&self) -> &[AccountPublic] {
        self.vault.accounts()
    }

    pub fn total_accounts(&self) -> usize {
        self.vault.accounts().len()
    }

    pub fn vault_status_label(&self) -> &'static str {
        if self.vault.is_unlocked() {
            "Activo"
        } else if self.vault.is_initialized() {
            "Bloqueado"
        } else {
            "Sin inicializar"
        }
    }

    pub fn workspace_scope(&self) -> &WorkspaceScope {
        &self.state.workspace_scope
    }

    pub fn selected_directory_path(&self) -> Option<&str> {
        self.state.workspace_scope.directory_path()
    }

    pub fn selected_account(&self) -> Option<AccountPublic> {
        let selected_id = self.state.selected_account_id?;
        self.vault
            .accounts()
            .iter()
            .find(|account| account.id == selected_id)
            .cloned()
    }

    pub fn checked_account_count(&self) -> usize {
        self.state.checked_account_ids.len()
    }

    pub fn is_account_checked(&self, account_id: uuid::Uuid) -> bool {
        self.state.checked_account_ids.contains(&account_id)
    }

    pub fn checked_accounts(&self) -> Vec<AccountPublic> {
        self.vault
            .accounts()
            .iter()
            .filter(|account| self.state.checked_account_ids.contains(&account.id))
            .cloned()
            .collect()
    }

    pub fn sync_selection(&mut self) {
        if self.is_search_pending() {
            return;
        }

        let visible = self.visible_accounts();
        let visible_ids = visible
            .iter()
            .map(|account| account.id)
            .collect::<HashSet<_>>();
        self.state
            .checked_account_ids
            .retain(|account_id| visible_ids.contains(account_id));
        let selection_is_visible = self
            .state
            .selected_account_id
            .is_some_and(|selected_id| visible.iter().any(|account| account.id == selected_id));

        if !selection_is_visible {
            self.state.selected_account_id = visible.first().map(|account| account.id);
        }
    }

    pub fn active_search_query(&self) -> Option<String> {
        let trimmed = self.state.search_query.trim();
        (trimmed.chars().count() >= 3).then(|| trimmed.to_owned())
    }

    pub fn is_search_pending(&self) -> bool {
        self.state
            .search
            .pending_query
            .as_deref()
            .is_some_and(|query| self.active_search_query().as_deref() == Some(query))
    }

    pub fn has_search_results(&self) -> bool {
        self.active_search_query()
            .as_deref()
            .is_some_and(|query| self.state.search.is_active_for(query))
    }

    pub(crate) fn has_background_vault_work(&self) -> bool {
        self.pending_vault_job.is_some() || self.pending_history_job.is_some()
    }

    pub(crate) fn start_search_if_needed(&mut self, ctx: &egui::Context) {
        let Some(query) = self.active_search_query() else {
            self.state.search.clear();
            self.pending_search_job = None;
            return;
        };

        if self.state.search.is_active_for(&query)
            || self.state.search.pending_query.as_deref() == Some(query.as_str())
        {
            return;
        }

        self.state.search.pending_query = Some(query.clone());
        self.state.search.active_query.clear();
        self.state.search.matched_account_ids.clear();
        self.pending_search_job = Some(crate::app_tasks::spawn_search_job(
            query,
            self.vault.accounts().to_vec(),
        ));
        ctx.request_repaint_after(std::time::Duration::from_millis(75));
    }

    pub(crate) fn invalidate_search(&mut self) {
        self.state.search.clear();
        self.pending_search_job = None;
    }

    fn poll_pending_search(&mut self, ctx: &egui::Context) {
        let poll = self.pending_search_job.as_ref().map(PendingTask::poll);

        match poll {
            Some(PendingPoll::Pending) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(75));
            }
            Some(PendingPoll::Finished(Ok(result))) => {
                self.pending_search_job = None;
                if self.active_search_query().as_deref() == Some(result.query.as_str()) {
                    self.state.search.active_query = result.query;
                    self.state.search.pending_query = None;
                    self.state.search.matched_account_ids = result.matched_account_ids;
                    self.sync_selection();
                }
            }
            Some(PendingPoll::Finished(Err(error))) => {
                self.pending_search_job = None;
                self.state.search.pending_query = None;
                self.state.search.active_query.clear();
                self.state.search.matched_account_ids.clear();
                self.set_banner(
                    BannerTone::Warning,
                    format!("La búsqueda no pudo completarse: {error}"),
                );
            }
            None => {}
        }
    }

    fn poll_pending_token_job(&mut self, ctx: &egui::Context) {
        let poll = self.pending_token_job.as_ref().map(PendingTask::poll);

        match poll {
            Some(PendingPoll::Pending) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
            Some(PendingPoll::Finished(Ok(result))) => {
                self.pending_token_job = None;
                self.state.token_dialog.pending = false;
                self.apply_token_result(result);
            }
            Some(PendingPoll::Finished(Err(error))) => {
                self.pending_token_job = None;
                self.state.token_dialog.pending = false;
                self.state.token_dialog.error = Some(error);
                self.state.token_dialog.action_message = None;
                self.state.token_dialog.action_tone = Some(BannerTone::Error);
            }
            None => {}
        }
    }

    fn apply_token_result(&mut self, result: TokenTaskResult) {
        if Some(result.account_id) != self.state.selected_account_id {
            return;
        }

        let token = result.token;
        let previous_token = result.previous_token;
        self.state.token_dialog.error = None;
        self.state.token_dialog.last_visible_second = Some(token.generated_at);
        self.state.token_dialog.token = Some(token.clone());

        let message = match previous_token {
            Some(previous)
                if previous.account_id == token.account_id
                    && previous.code == token.code
                    && previous.expires_at == token.expires_at =>
            {
                self.state.token_dialog.action_tone = Some(BannerTone::Info);
                format!(
                    "Refresh confirmado al instante. El período TOTP actual sigue vigente, por eso el código no cambió todavía. Vence en {}s.",
                    token.seconds_remaining,
                )
            }
            Some(_) => {
                self.state.token_dialog.action_tone = Some(BannerTone::Success);
                format!(
                    "Refresh confirmado al instante. Se detectó el período TOTP nuevo y el código visible ya fue actualizado. Vence en {}s.",
                    token.seconds_remaining,
                )
            }
            None => {
                self.state.token_dialog.action_tone = Some(BannerTone::Info);
                "Token leído en segundo plano para el período TOTP vigente.".to_owned()
            }
        };

        self.state.token_dialog.action_message = Some(message);
    }

    fn poll_pending_vault_job(&mut self, ctx: &egui::Context) {
        let poll = self
            .pending_vault_job
            .as_ref()
            .map(|job| (job.kind, job.task.poll()));

        match poll {
            Some((_, PendingPoll::Pending)) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(75));
            }
            Some((kind, PendingPoll::Finished(Ok(result)))) => {
                self.pending_vault_job = None;
                self.clear_pending_state(kind);
                self.apply_vault_job_result(result);
            }
            Some((kind, PendingPoll::Finished(Err(error)))) => {
                self.pending_vault_job = None;
                self.apply_vault_job_error(kind, error);
            }
            None => {}
        }
    }

    fn poll_pending_history_job(&mut self, ctx: &egui::Context) {
        let poll = self.pending_history_job.as_ref().map(PendingTask::poll);

        match poll {
            Some(PendingPoll::Pending) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(75));
            }
            Some(PendingPoll::Finished(Ok(HistoryTaskResult::Loaded(entries)))) => {
                self.pending_history_job = None;
                if self.state.restore_dialog.open {
                    self.state.restore_dialog.load_entries(entries);
                }
            }
            Some(PendingPoll::Finished(Ok(HistoryTaskResult::Restored {
                result,
                remaining_entries,
            }))) => {
                self.pending_history_job = None;
                let result = *result;
                self.vault.finish_unlock(result.password, result.session);
                self.invalidate_search();
                self.state.selected_account_id = Some(result.payload.id);
                if self.state.restore_dialog.open {
                    self.state.restore_dialog.load_entries(remaining_entries);
                }
                self.sync_selection();
                self.set_banner(
                    BannerTone::Success,
                    format!(
                        "Cuenta {} restaurada desde historial.",
                        result.payload.display_name()
                    ),
                );
            }
            Some(PendingPoll::Finished(Err(error))) => {
                self.pending_history_job = None;
                if self.state.restore_dialog.open {
                    self.state.restore_dialog.pending = false;
                    self.state.restore_dialog.pending_message = None;
                    self.state.restore_dialog.error = Some(error);
                } else {
                    self.set_banner(
                        BannerTone::Error,
                        format!("La operación del historial no pudo completarse: {error}"),
                    );
                }
            }
            None => {}
        }
    }

    fn clear_pending_state(&mut self, kind: PendingVaultJobKind) {
        match kind {
            PendingVaultJobKind::CreateDirectory => {
                self.state.create_directory_dialog.pending = false;
            }
            PendingVaultJobKind::DeleteDirectory => {
                self.state.remove_directory_dialog.pending = false;
            }
            PendingVaultJobKind::RemoveAccount => {
                self.state.remove_dialog.pending = false;
            }
            PendingVaultJobKind::RemoveAccounts => {
                self.state.remove_dialog.pending = false;
            }
            PendingVaultJobKind::ImportUri => {
                self.state.import_dialog.pending = false;
            }
            PendingVaultJobKind::ImportQr => {
                self.state.import_qr_dialog.pending = false;
            }
            PendingVaultJobKind::ImportFile => {
                self.state.import_file_dialog.pending = false;
            }
            PendingVaultJobKind::ImportVaultBackup => {}
            PendingVaultJobKind::ExportVaultBackup => {
                self.state.export_dialog.pending = false;
            }
            PendingVaultJobKind::ExportAccountFile
            | PendingVaultJobKind::ExportAccountQr
            | PendingVaultJobKind::ExportAccountUri => {
                self.state.account_uri_dialog.pending = false;
            }
        }
    }

    fn refresh_restore_dialog_if_open(&mut self) {
        if !self.state.restore_dialog.open || self.pending_history_job.is_some() {
            return;
        }

        let Ok(password) = self.vault.password_snapshot() else {
            self.state.restore_dialog.error =
                Some("No se pudo refrescar el historial restaurable.".to_owned());
            return;
        };

        self.state
            .restore_dialog
            .begin_pending("Actualizando historial restaurable...");
        self.pending_history_job = Some(crate::app_tasks::spawn_load_history_job(password));
    }

    fn apply_vault_job_error(&mut self, kind: PendingVaultJobKind, error: String) {
        self.clear_pending_state(kind);

        match kind {
            PendingVaultJobKind::CreateDirectory => {
                self.state.create_directory_dialog.error = Some(error);
            }
            PendingVaultJobKind::DeleteDirectory => {
                self.state.remove_directory_dialog.error = Some(error);
            }
            PendingVaultJobKind::RemoveAccount => {
                self.state.remove_dialog.error = Some(error);
            }
            PendingVaultJobKind::RemoveAccounts => {
                self.state.remove_dialog.error = Some(error);
            }
            PendingVaultJobKind::ImportUri => {
                self.state.import_dialog.error = Some(error);
            }
            PendingVaultJobKind::ImportQr => {
                self.state.import_qr_dialog.error = Some(error);
            }
            PendingVaultJobKind::ImportFile => {
                self.state.import_file_dialog.error = Some(error);
            }
            PendingVaultJobKind::ExportVaultBackup => {
                self.state.export_dialog.error = Some(error);
            }
            PendingVaultJobKind::ExportAccountUri => {
                self.state.account_uri_dialog.error = Some(error);
                self.state.account_uri_dialog.open = true;
            }
            PendingVaultJobKind::ExportAccountFile
            | PendingVaultJobKind::ExportAccountQr
            | PendingVaultJobKind::ImportVaultBackup => {
                self.set_banner(BannerTone::Error, error);
            }
        }
    }

    fn apply_vault_job_result(&mut self, result: VaultJobResult) {
        match result {
            VaultJobResult::DirectoryCreated(result) => {
                self.vault.finish_unlock(result.password, result.session);
                self.invalidate_search();
                self.state.create_directory_dialog.clear();
                self.state.workspace_scope = WorkspaceScope::Directory(result.payload.path.clone());
                self.sync_selection();
                self.set_banner(
                    BannerTone::Success,
                    format!("Directorio {} creado.", result.payload.path),
                );
            }
            VaultJobResult::DirectoryDeleted(result) => {
                self.vault.finish_unlock(result.password, result.session);
                self.invalidate_search();
                if self.selected_directory_path() == Some(result.payload.path.as_str()) {
                    self.state.workspace_scope = result
                        .payload
                        .parent_path()
                        .map(|path| WorkspaceScope::Directory(path.to_owned()))
                        .unwrap_or(WorkspaceScope::Unassigned);
                }
                self.state.remove_directory_dialog.clear();
                self.sync_selection();
                self.set_banner(
                    BannerTone::Success,
                    format!("Workspace {} eliminado.", result.payload.path),
                );
            }
            VaultJobResult::AccountRemoved(result) => {
                self.vault.finish_unlock(result.password, result.session);
                self.invalidate_search();
                self.state.remove_dialog.clear();
                self.state.checked_account_ids.remove(&result.payload.id);
                if self.state.selected_account_id == Some(result.payload.id) {
                    self.state.selected_account_id = None;
                }
                self.refresh_restore_dialog_if_open();
                self.sync_selection();
                self.set_banner(
                    BannerTone::Success,
                    format!(
                        "Cuenta {} eliminada del vault.",
                        result.payload.display_name()
                    ),
                );
            }
            VaultJobResult::AccountsRemoved(result) => {
                let removed_count = result.payload.len();
                self.vault.finish_unlock(result.password, result.session);
                self.invalidate_search();
                self.state.remove_dialog.clear();
                self.state.selected_account_id = None;
                self.state.checked_account_ids.clear();
                self.refresh_restore_dialog_if_open();
                self.sync_selection();
                self.set_banner(
                    BannerTone::Success,
                    format!("{removed_count} cuenta(s) eliminada(s) del vault."),
                );
            }
            VaultJobResult::AccountImported(result) => {
                self.vault.finish_unlock(result.password, result.session);
                self.invalidate_search();
                self.state.import_dialog.clear();
                self.state.import_qr_dialog.clear();
                self.state.import_file_dialog.clear();
                self.state.selected_account_id = Some(result.payload.id);
                self.sync_selection();
                self.set_banner(
                    BannerTone::Success,
                    format!(
                        "Cuenta {} importada correctamente.",
                        result.payload.display_name()
                    ),
                );
            }
            VaultJobResult::VaultImported(result) => {
                self.vault.finish_unlock(result.password, result.session);
                self.invalidate_search();
                self.state.workspace_scope = WorkspaceScope::Unassigned;
                self.sync_selection();
                self.set_banner(
                    BannerTone::Success,
                    format!(
                        "Backup importado. El vault activo ahora contiene {} cuenta(s).",
                        result.payload
                    ),
                );
            }
            VaultJobResult::VaultExported { path } => {
                self.state.export_dialog.close();
                self.set_banner(
                    BannerTone::Success,
                    format!("Backup del vault exportado en {}.", path.display()),
                );
            }
            VaultJobResult::AccountExportedFile {
                account_label,
                path,
            } => {
                self.set_banner(
                    BannerTone::Success,
                    format!(
                        "Cuenta {} exportada a archivo compatible en {}.",
                        account_label,
                        path.display()
                    ),
                );
            }
            VaultJobResult::AccountExportedQr {
                account_label,
                path,
            } => {
                self.set_banner(
                    BannerTone::Success,
                    format!(
                        "QR de la cuenta {} guardado en {}.",
                        account_label,
                        path.display()
                    ),
                );
            }
            VaultJobResult::AccountUriReady { account_label, uri } => {
                self.state.account_uri_dialog.open = true;
                self.state.account_uri_dialog.pending = false;
                self.state.account_uri_dialog.error = None;
                self.state.account_uri_dialog.account_label = account_label;
                self.state.account_uri_dialog.uri = uri;
                self.state.account_uri_dialog.reveal = false;
            }
        }
    }
}

fn workspace_scope_matches(account: &AccountPublic, scope: &WorkspaceScope) -> bool {
    match scope {
        WorkspaceScope::Unassigned => account.metadata.project_path.is_none(),
        WorkspaceScope::Directory(selected_directory_path) => {
            let Some(account_directory) = account.metadata.project_path.as_deref() else {
                return false;
            };

            account_directory == selected_directory_path
                || account_directory
                    .strip_prefix(selected_directory_path.as_str())
                    .is_some_and(|suffix| suffix.starts_with('/'))
        }
    }
}

impl eframe::App for ForgeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::apply(ctx, self.state.theme_preference);
        self.poll_pending_prepare(ctx);
        self.poll_pending_unlock(ctx);
        self.poll_pending_vault_job(ctx);
        self.poll_pending_history_job(ctx);
        self.poll_pending_token_job(ctx);
        self.poll_pending_search(ctx);
        self.start_search_if_needed(ctx);

        match self.state.screen {
            Screen::Loader => views::loader::render(ctx, self),
            Screen::Main => {
                self.sync_token_dialog();
                views::main_window::render(ctx, self);
                dialogs::render(ctx, self);
            }
        }

        if self.state.token_dialog.open {
            request_open_windows_repaint(ctx);
        }
    }
}

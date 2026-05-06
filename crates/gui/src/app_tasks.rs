use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};

use image::Luma;
use qrcode::QrCode;
use secrecy::SecretString;
use uuid::Uuid;

use mfa_forge_core::{AccountHistoryEntryPublic, AccountMetadata, AccountPublic, TotpToken};

use crate::{
    qr_import,
    vault::{PendingUnlockSession, VaultFacade},
};

pub(crate) enum PendingPoll<T> {
    Pending,
    Finished(Result<T, String>),
}

pub(crate) struct PendingTask<T> {
    receiver: Receiver<Result<T, String>>,
}

impl<T> PendingTask<T> {
    pub(crate) fn poll(&self) -> PendingPoll<T> {
        match self.receiver.try_recv() {
            Ok(result) => PendingPoll::Finished(result),
            Err(TryRecvError::Empty) => PendingPoll::Pending,
            Err(TryRecvError::Disconnected) => {
                PendingPoll::Finished(Err("The background task was interrupted.".to_owned()))
            }
        }
    }
}

pub(crate) struct TokenTaskResult {
    pub account_id: Uuid,
    pub token: TotpToken,
    pub previous_token: Option<TotpToken>,
}

pub(crate) struct SessionTaskResult<T> {
    pub password: SecretString,
    pub session: PendingUnlockSession,
    pub payload: T,
}

pub(crate) enum VaultJobResult {
    DirectoryCreated(SessionTaskResult<mfa_forge_core::ProjectDirectory>),
    DirectoryDeleted(SessionTaskResult<mfa_forge_core::ProjectDirectory>),
    AccountRemoved(SessionTaskResult<AccountPublic>),
    AccountsRemoved(SessionTaskResult<Vec<AccountPublic>>),
    AccountImported(SessionTaskResult<AccountPublic>),
    VaultImported(SessionTaskResult<usize>),
    VaultExported {
        path: PathBuf,
    },
    AccountExportedFile {
        account_label: String,
        path: PathBuf,
    },
    AccountExportedQr {
        account_label: String,
        path: PathBuf,
    },
    AccountUriReady {
        account_label: String,
        uri: String,
    },
}

pub(crate) enum HistoryTaskResult {
    Loaded(Vec<AccountHistoryEntryPublic>),
    Restored {
        result: Box<SessionTaskResult<AccountPublic>>,
        remaining_entries: Vec<AccountHistoryEntryPublic>,
    },
}

pub(crate) struct SearchTaskResult {
    pub query: String,
    pub matched_account_ids: Vec<Uuid>,
}

pub(crate) fn spawn_token_job(
    password: SecretString,
    account: AccountPublic,
    previous_token: Option<TotpToken>,
) -> PendingTask<TokenTaskResult> {
    spawn_task(move || {
        let vault = unlocked_vault(&password)?;
        let token = vault.generate_token(&account)?;

        Ok(TokenTaskResult {
            account_id: account.id,
            token,
            previous_token,
        })
    })
}

pub(crate) fn spawn_create_directory_job(
    password: SecretString,
    path: String,
) -> PendingTask<VaultJobResult> {
    spawn_task(move || {
        let result = run_session_task(password, move |vault| vault.create_directory(path))?;
        Ok(VaultJobResult::DirectoryCreated(result))
    })
}

pub(crate) fn spawn_delete_directory_job(
    password: SecretString,
    path: String,
) -> PendingTask<VaultJobResult> {
    spawn_task(move || {
        let result = run_session_task(password, move |vault| vault.delete_directory(path))?;
        Ok(VaultJobResult::DirectoryDeleted(result))
    })
}

pub(crate) fn spawn_remove_account_job(
    password: SecretString,
    account: AccountPublic,
) -> PendingTask<VaultJobResult> {
    spawn_task(move || {
        let result = run_session_task(password, move |vault| vault.remove_account(&account))?;
        Ok(VaultJobResult::AccountRemoved(result))
    })
}

pub(crate) fn spawn_remove_accounts_job(
    password: SecretString,
    accounts: Vec<AccountPublic>,
) -> PendingTask<VaultJobResult> {
    spawn_task(move || {
        let account_ids = accounts
            .into_iter()
            .map(|account| account.id)
            .collect::<Vec<_>>();
        let result = run_session_task(password, move |vault| vault.remove_accounts(&account_ids))?;
        Ok(VaultJobResult::AccountsRemoved(result))
    })
}

pub(crate) fn spawn_import_uri_job(
    password: SecretString,
    uri: String,
    metadata: AccountMetadata,
) -> PendingTask<VaultJobResult> {
    spawn_task(move || {
        let result = run_session_task(password, move |vault| {
            vault.import_otpauth_with_metadata(uri.trim(), metadata)
        })?;
        Ok(VaultJobResult::AccountImported(result))
    })
}

pub(crate) fn spawn_import_file_job(
    password: SecretString,
    file_path: PathBuf,
    metadata: AccountMetadata,
) -> PendingTask<VaultJobResult> {
    spawn_task(move || {
        let uri = fs::read_to_string(&file_path).map_err(|error| {
            format!(
                "The account file {} could not be read: {error}",
                file_path.display()
            )
        })?;
        let result = run_session_task(password, move |vault| {
            vault.import_otpauth_with_metadata(uri.trim(), metadata)
        })?;
        Ok(VaultJobResult::AccountImported(result))
    })
}

pub(crate) fn spawn_import_qr_job(
    password: SecretString,
    image_path: PathBuf,
    metadata: AccountMetadata,
) -> PendingTask<VaultJobResult> {
    spawn_task(move || {
        let image_path_text = image_path.display().to_string();
        let uri = qr_import::decode_otpauth_uri_from_image(&image_path_text)?;
        let result = run_session_task(password, move |vault| {
            vault.import_otpauth_with_metadata(uri.trim(), metadata)
        })?;
        Ok(VaultJobResult::AccountImported(result))
    })
}

pub(crate) fn spawn_import_vault_backup_job(
    password: SecretString,
    source: PathBuf,
) -> PendingTask<VaultJobResult> {
    spawn_task(move || {
        let result = run_session_task(password, move |vault| vault.import_vault_backup(&source))?;
        Ok(VaultJobResult::VaultImported(result))
    })
}

pub(crate) fn spawn_export_vault_backup_job(destination: PathBuf) -> PendingTask<VaultJobResult> {
    spawn_task(move || {
        let vault = VaultFacade::try_new().map_err(|error| error.to_string())?;
        vault.export_vault_backup(&destination)?;
        Ok(VaultJobResult::VaultExported { path: destination })
    })
}

pub(crate) fn spawn_export_account_file_job(
    password: SecretString,
    account: AccountPublic,
    destination: PathBuf,
) -> PendingTask<VaultJobResult> {
    spawn_task(move || {
        let vault = unlocked_vault(&password)?;
        let uri = vault.export_account_uri(&account)?;
        write_text_file(&destination, &uri)?;

        Ok(VaultJobResult::AccountExportedFile {
            account_label: account.display_name(),
            path: destination,
        })
    })
}

pub(crate) fn spawn_export_account_qr_job(
    password: SecretString,
    account: AccountPublic,
    destination: PathBuf,
) -> PendingTask<VaultJobResult> {
    spawn_task(move || {
        let vault = unlocked_vault(&password)?;
        let uri = vault.export_account_uri(&account)?;
        let qr_code = QrCode::new(uri.as_bytes())
            .map_err(|error| format!("The account QR could not be generated: {error}"))?;
        let image = qr_code
            .render::<Luma<u8>>()
            .min_dimensions(320, 320)
            .build();

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "The destination folder {} could not be prepared: {error}",
                    parent.display()
                )
            })?;
        }

        image.save(&destination).map_err(|error| {
            format!(
                "The account QR could not be saved to {}: {error}",
                destination.display()
            )
        })?;

        Ok(VaultJobResult::AccountExportedQr {
            account_label: account.display_name(),
            path: destination,
        })
    })
}

pub(crate) fn spawn_export_account_uri_job(
    password: SecretString,
    account: AccountPublic,
) -> PendingTask<VaultJobResult> {
    spawn_task(move || {
        let vault = unlocked_vault(&password)?;
        let uri = vault.export_account_uri(&account)?;
        Ok(VaultJobResult::AccountUriReady {
            account_label: account.display_name(),
            uri,
        })
    })
}

pub(crate) fn spawn_load_history_job(password: SecretString) -> PendingTask<HistoryTaskResult> {
    spawn_task(move || {
        let vault = unlocked_vault(&password)?;
        let entries = vault.restore_candidates()?;
        Ok(HistoryTaskResult::Loaded(entries))
    })
}

pub(crate) fn spawn_restore_history_entry_job(
    password: SecretString,
    entry_id: Uuid,
) -> PendingTask<HistoryTaskResult> {
    spawn_task(move || {
        let mut vault = unlocked_vault(&password)?;
        let restored = vault.restore_history_entry(entry_id)?;
        let remaining_entries = vault.restore_candidates()?;
        let session = vault.session_snapshot()?;

        Ok(HistoryTaskResult::Restored {
            result: Box::new(SessionTaskResult {
                password,
                session,
                payload: restored,
            }),
            remaining_entries,
        })
    })
}

pub(crate) fn spawn_search_job(
    query: String,
    accounts: Vec<AccountPublic>,
) -> PendingTask<SearchTaskResult> {
    spawn_task(move || {
        let matched_account_ids = accounts
            .into_iter()
            .filter(|account| account.matches_query(&query))
            .map(|account| account.id)
            .collect();

        Ok(SearchTaskResult {
            query,
            matched_account_ids,
        })
    })
}

fn spawn_task<T, F>(task: F) -> PendingTask<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(task());
    });
    PendingTask { receiver }
}

fn unlocked_vault(password: &SecretString) -> Result<VaultFacade, String> {
    let mut vault = VaultFacade::try_new().map_err(|error| error.to_string())?;
    let session = vault.prepare_unlock(password)?;
    vault.finish_unlock(password.clone(), session);
    Ok(vault)
}

fn run_session_task<T, F>(password: SecretString, task: F) -> Result<SessionTaskResult<T>, String>
where
    F: FnOnce(&mut VaultFacade) -> Result<T, String>,
{
    let mut vault = unlocked_vault(&password)?;
    let payload = task(&mut vault)?;
    let session = vault.session_snapshot()?;

    Ok(SessionTaskResult {
        password,
        session,
        payload,
    })
}

fn write_text_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "The destination folder {} could not be prepared: {error}",
                parent.display()
            )
        })?;
    }

    let mut file = fs::File::create(path)
        .map_err(|error| format!("The file {} could not be created: {error}", path.display()))?;
    file.write_all(contents.as_bytes())
        .map_err(|error| format!("The file {} could not be written: {error}", path.display()))?;
    file.sync_all().map_err(|error| {
        format!(
            "The file {} could not be finalized: {error}",
            path.display()
        )
    })
}

mod agent;
mod args;
mod csv_io;
mod output;
mod password;

use std::{collections::BTreeSet, fs, path::Path};

use anyhow::{Context, Result};
use clap::Parser;
use secrecy::ExposeSecret;
use serde::Serialize;
use uuid::Uuid;

use mfa_forge_core::{
    AccountHistoryEntryPublic, AccountMetadata, AccountPublic, AccountRecord, AccountSelector,
    TotpConfig,
};
use mfa_forge_storage::VaultRepository;

use crate::{
    agent::{proxy_agent_session, proxy_mcp_server},
    args::{Cli, Command, ExportDataFormat},
    csv_io::{
        ExternalImportPreviewRow, export_accounts_metadata_csv, import_accounts_from_bitwarden_csv,
        import_accounts_from_csv, preview_bitwarden_accounts,
    },
    output::{print_payload, render_accounts, render_external_import_preview, render_history},
    password::{prompt_master_password, prompt_otpauth_uri, prompt_totp_secret},
};

#[derive(Debug, Serialize)]
struct StatusMessage {
    status: &'static str,
    message: String,
}

#[derive(Debug, Serialize)]
struct InitResult {
    status: &'static str,
    vault_path: String,
}

#[derive(Debug, Serialize)]
struct AccountResult {
    status: &'static str,
    account: mfa_forge_core::AccountPublic,
}

#[derive(Debug, Serialize)]
struct AccountsResult {
    accounts: Vec<mfa_forge_core::AccountPublic>,
}

#[derive(Debug, Serialize)]
struct ImportCsvResult {
    status: &'static str,
    source_path: String,
    imported_count: usize,
    accounts: Vec<mfa_forge_core::AccountPublic>,
}

#[derive(Debug, Serialize)]
struct ExternalImportPreviewResult {
    status: &'static str,
    source_path: String,
    selectable_rows: Vec<ExternalImportPreviewRow>,
}

#[derive(Debug, Serialize)]
struct ExternalImportResult {
    status: &'static str,
    source_path: String,
    imported_count: usize,
    selected_rows: Option<Vec<usize>>,
    accounts: Vec<AccountPublic>,
}

#[derive(Debug, Serialize)]
struct HistoryResult {
    entries: Vec<AccountHistoryEntryPublic>,
}

#[derive(Debug, Serialize)]
struct RestoreResult {
    status: &'static str,
    restored_from_entry_id: Uuid,
    account: AccountPublic,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let repository = VaultRepository::with_default_path()?;

    match cli.command {
        Command::Agent => {
            proxy_agent_session()?;
        }
        Command::Mcp => {
            proxy_mcp_server()?;
        }
        Command::Init => {
            let password = prompt_master_password(true)?;
            repository.initialize(&password)?;
            let payload = InitResult {
                status: "initialized",
                vault_path: repository.path().display().to_string(),
            };
            print_payload(
                cli.format,
                format!("Vault initialized at {}", payload.vault_path),
                &payload,
            )?;
        }
        Command::Add {
            service,
            user,
            secret,
            labels,
            note,
            project_path,
            source,
        } => {
            let password = prompt_master_password(false)?;
            let secret = prompt_totp_secret(secret)?;
            let account = AccountRecord::new_with_metadata(
                service,
                user,
                secret,
                TotpConfig::default(),
                cli_metadata(labels, note, project_path, source),
            )?;
            let stored = repository.add_account(&password, account)?;
            let payload = AccountResult {
                status: "added",
                account: stored.clone(),
            };
            print_payload(
                cli.format,
                format!("Added {}", stored.display_name()),
                &payload,
            )?;
        }
        Command::Import {
            uri,
            labels,
            note,
            project_path,
            source,
        } => {
            let password = prompt_master_password(false)?;
            let uri = prompt_otpauth_uri(uri)?;
            let account = AccountRecord::from_otpauth_uri_with_metadata(
                uri.expose_secret(),
                cli_metadata(labels, note, project_path, source),
            )?;
            let stored = repository.add_account(&password, account)?;
            let payload = AccountResult {
                status: "imported",
                account: stored.clone(),
            };
            print_payload(
                cli.format,
                format!("Imported {}", stored.display_name()),
                &payload,
            )?;
        }
        Command::ImportCsv { path } => {
            let password = prompt_master_password(false)?;
            let import_path = Path::new(&path);
            let accounts = import_accounts_from_csv(import_path)?;
            let imported = repository.add_accounts(&password, accounts)?;
            let payload = ImportCsvResult {
                status: "imported",
                source_path: import_path.display().to_string(),
                imported_count: imported.len(),
                accounts: imported,
            };
            print_payload(
                cli.format,
                format!(
                    "Imported {} MFA accounts from {}",
                    payload.imported_count, payload.source_path
                ),
                &payload,
            )?;
        }
        Command::ImportBitwardenCsv {
            path,
            rows,
            preview,
        } => {
            let import_path = Path::new(&path);
            if preview {
                let selectable_rows = preview_bitwarden_accounts(import_path)?;
                let payload = ExternalImportPreviewResult {
                    status: "preview",
                    source_path: import_path.display().to_string(),
                    selectable_rows: selectable_rows.clone(),
                };
                print_payload(
                    cli.format,
                    render_external_import_preview(&selectable_rows),
                    &payload,
                )?;
            } else {
                let password = prompt_master_password(false)?;
                let selected_rows = parse_selected_rows(rows.as_deref())?;
                let accounts =
                    import_accounts_from_bitwarden_csv(import_path, selected_rows.as_ref())?;
                let imported = repository.add_accounts(&password, accounts)?;
                let payload = ExternalImportResult {
                    status: "imported",
                    source_path: import_path.display().to_string(),
                    imported_count: imported.len(),
                    selected_rows: selected_rows.map(|rows| rows.into_iter().collect()),
                    accounts: imported.clone(),
                };
                print_payload(
                    cli.format,
                    format!(
                        "Imported {} MFA accounts from {}",
                        payload.imported_count, payload.source_path
                    ),
                    &payload,
                )?;
            }
        }
        Command::List => {
            let password = prompt_master_password(false)?;
            let accounts = repository.list_accounts(&password)?;
            let payload = AccountsResult {
                accounts: accounts.clone(),
            };
            print_payload(cli.format, render_accounts(&accounts), &payload)?;
        }
        Command::History => {
            let password = prompt_master_password(false)?;
            let entries = repository.list_history(&password)?;
            let payload = HistoryResult {
                entries: entries.clone(),
            };
            print_payload(cli.format, render_history(&entries), &payload)?;
        }
        Command::Restore { entry_id } => {
            let password = prompt_master_password(false)?;
            let entry_id = Uuid::parse_str(&entry_id)
                .with_context(|| format!("ENTRY_ID inválido: {entry_id}"))?;
            let restored = repository.restore_history_entry(&password, entry_id)?;
            let payload = RestoreResult {
                status: "restored",
                restored_from_entry_id: entry_id,
                account: restored.clone(),
            };
            print_payload(
                cli.format,
                format!(
                    "Restored {} from history entry {}",
                    restored.display_name(),
                    entry_id
                ),
                &payload,
            )?;
        }
        Command::Token { service, user } => {
            let password = prompt_master_password(false)?;
            let selector = AccountSelector::new(service, user)?;
            let account = repository.find_account(&password, &selector)?;
            let token = account.generate_current_token()?;
            print_payload(
                cli.format,
                format!(
                    "Token for {} ({}): {} [{}s remaining]",
                    token.service, token.user, token.code, token.seconds_remaining
                ),
                &token,
            )?;
        }
        Command::Remove { service, user } => {
            let password = prompt_master_password(false)?;
            let selector = AccountSelector::new(service, user)?;
            let removed = repository.remove_account(&password, &selector)?;
            let payload = AccountResult {
                status: "removed",
                account: removed.clone(),
            };
            print_payload(
                cli.format,
                format!("Removed {}", removed.display_name()),
                &payload,
            )?;
        }
        Command::RotatePassword => {
            let current_password = prompt_master_password(false)?;
            let new_password = prompt_master_password(true)?;
            repository.change_master_password(&current_password, &new_password)?;
            print_payload(
                cli.format,
                "Master password rotated and vault re-encrypted.".to_owned(),
                &StatusMessage {
                    status: "password_rotated",
                    message: "Vault re-encrypted with the new master password.".to_owned(),
                },
            )?;
        }
        Command::Export { data_format, path } => {
            let password = prompt_master_password(false)?;
            let accounts = repository.export_metadata(&password)?;
            let payload = AccountsResult {
                accounts: accounts.clone(),
            };
            match data_format {
                ExportDataFormat::Json => {
                    let json = serde_json::to_string_pretty(&payload)?;
                    if let Some(path) = path {
                        fs::write(&path, json)
                            .with_context(|| format!("No se pudo escribir {path}"))?;
                        print_payload(
                            cli.format,
                            format!("Metadata export written to {path}"),
                            &StatusMessage {
                                status: "exported",
                                message: format!("Structured metadata export written to {path}."),
                            },
                        )?;
                    } else {
                        println!("{json}");
                    }
                }
                ExportDataFormat::Csv => {
                    let csv = export_accounts_metadata_csv(&accounts)?;
                    if let Some(path) = path {
                        fs::write(&path, csv)
                            .with_context(|| format!("No se pudo escribir {path}"))?;
                        print_payload(
                            cli.format,
                            format!("Metadata CSV export written to {path}"),
                            &StatusMessage {
                                status: "exported",
                                message: format!("Metadata CSV export written to {path}."),
                            },
                        )?;
                    } else {
                        print!("{csv}");
                    }
                }
            }
        }
    }

    Ok(())
}

fn cli_metadata(
    labels: Vec<String>,
    note: Option<String>,
    project_path: Option<String>,
    source: Option<String>,
) -> AccountMetadata {
    AccountMetadata {
        labels,
        note,
        project_path,
        source,
        updated_at: 0,
    }
}

fn parse_selected_rows(rows: Option<&str>) -> Result<Option<BTreeSet<usize>>> {
    let Some(rows) = rows.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    let mut selected_rows = BTreeSet::new();
    for raw_value in rows.split(',') {
        let trimmed = raw_value.trim();
        if trimmed.is_empty() {
            continue;
        }

        let row_number = trimmed
            .parse::<usize>()
            .with_context(|| format!("Fila inválida en --rows: {trimmed}"))?;
        selected_rows.insert(row_number);
    }

    Ok(Some(selected_rows))
}

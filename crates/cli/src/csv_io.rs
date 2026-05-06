use std::{collections::BTreeSet, path::Path};

use anyhow::{Context, Result};
use csv::{ReaderBuilder, WriterBuilder};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

use mfa_forge_core::{AccountMetadata, AccountPublic, AccountRecord, TotpAlgorithm, TotpConfig};

#[derive(Debug, Deserialize)]
struct CsvImportRow {
    service: String,
    user: String,
    secret: String,
    #[serde(default)]
    algorithm: Option<TotpAlgorithm>,
    #[serde(default)]
    digits: Option<u32>,
    #[serde(default)]
    period_seconds: Option<u64>,
    #[serde(default)]
    labels: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    project_path: Option<String>,
    #[serde(default)]
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BitwardenCsvRow {
    #[serde(default)]
    folder: Option<String>,
    #[serde(default)]
    favorite: Option<bool>,
    #[serde(rename = "type", default)]
    entry_type: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    login_username: Option<String>,
    #[serde(default)]
    login_totp: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExternalImportPreviewRow {
    pub row_number: usize,
    pub service: String,
    pub user: String,
    pub project_path: Option<String>,
    pub labels: Vec<String>,
    pub source: String,
}

#[derive(Debug)]
struct BitwardenImportCandidate {
    row_number: usize,
    account: AccountRecord,
}

impl BitwardenImportCandidate {
    fn preview(&self) -> ExternalImportPreviewRow {
        let public = self.account.public();
        ExternalImportPreviewRow {
            row_number: self.row_number,
            service: public.service.clone(),
            user: public.user.clone(),
            project_path: public.metadata.project_path.clone(),
            labels: public.metadata.labels.clone(),
            source: public
                .metadata
                .source
                .clone()
                .unwrap_or_else(|| "bitwarden_csv".to_owned()),
        }
    }
}

pub fn import_accounts_from_csv(path: &Path) -> Result<Vec<AccountRecord>> {
    let mut reader = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_path(path)
        .with_context(|| format!("No se pudo abrir el CSV {}", path.display()))?;

    let mut accounts = Vec::new();
    for (index, row) in reader.deserialize::<CsvImportRow>().enumerate() {
        let row_number = index + 2;
        let row = row.with_context(|| {
            format!("El CSV no cumple el esquema esperado en la fila {row_number}")
        })?;

        let config = TotpConfig {
            algorithm: row.algorithm.unwrap_or_default(),
            digits: row.digits.unwrap_or(6),
            period_seconds: row.period_seconds.unwrap_or(30),
        };

        let account = AccountRecord::new_with_metadata(
            row.service,
            row.user,
            SecretString::from(row.secret),
            config,
            metadata_from_optional_fields(row.labels, row.note, row.project_path, row.source),
        )
        .with_context(|| format!("La fila {row_number} del CSV es inválida"))?;

        accounts.push(account);
    }

    Ok(accounts)
}

pub fn preview_bitwarden_accounts(path: &Path) -> Result<Vec<ExternalImportPreviewRow>> {
    collect_bitwarden_candidates(path).map(|candidates| {
        candidates
            .into_iter()
            .map(|candidate| candidate.preview())
            .collect()
    })
}

pub fn import_accounts_from_bitwarden_csv(
    path: &Path,
    selected_rows: Option<&BTreeSet<usize>>,
) -> Result<Vec<AccountRecord>> {
    let candidates = collect_bitwarden_candidates(path)?;

    Ok(candidates
        .into_iter()
        .filter(|candidate| {
            selected_rows.is_none_or(|selected| selected.contains(&candidate.row_number))
        })
        .map(|candidate| candidate.account)
        .collect())
}

pub fn export_accounts_metadata_csv(accounts: &[AccountPublic]) -> Result<String> {
    let mut writer = WriterBuilder::new().from_writer(Vec::new());
    writer.write_record([
        "id",
        "service",
        "user",
        "kind",
        "algorithm",
        "digits",
        "period_seconds",
        "created_at",
        "updated_at",
        "project_path",
        "labels",
        "note",
        "source",
    ])?;

    for account in accounts {
        writer.write_record([
            account.id.to_string(),
            account.service.clone(),
            account.user.clone(),
            account.kind.as_str().to_owned(),
            account.totp.algorithm.as_str().to_owned(),
            account.totp.digits.to_string(),
            account.totp.period_seconds.to_string(),
            account.created_at.to_string(),
            account.metadata.updated_at.to_string(),
            account.metadata.project_path.clone().unwrap_or_default(),
            account.metadata.labels_csv(),
            account.metadata.note.clone().unwrap_or_default(),
            account.metadata.source.clone().unwrap_or_default(),
        ])?;
    }

    let bytes = writer
        .into_inner()
        .map_err(|error| error.into_error())
        .context("No se pudo terminar de serializar el CSV")?;
    String::from_utf8(bytes).context("El CSV generado no es UTF-8 válido")
}

fn collect_bitwarden_candidates(path: &Path) -> Result<Vec<BitwardenImportCandidate>> {
    let mut reader = ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_path(path)
        .with_context(|| format!("No se pudo abrir el CSV de Bitwarden {}", path.display()))?;

    let mut candidates = Vec::new();

    for (index, row) in reader.deserialize::<BitwardenCsvRow>().enumerate() {
        let row_number = index + 2;
        let row = row
            .with_context(|| format!("El CSV de Bitwarden es inválido en la fila {row_number}"))?;

        if row
            .entry_type
            .as_deref()
            .is_some_and(|entry_type| !entry_type.trim().eq_ignore_ascii_case("login"))
        {
            continue;
        }

        let Some(totp_value) = row
            .login_totp
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };

        let service = row
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .with_context(|| format!("La fila {row_number} no trae nombre de servicio"))?;
        let user = row
            .login_username
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| "sin-usuario".to_owned());

        let mut labels = Vec::new();
        if row.favorite.unwrap_or(false) {
            labels.push("favorite".to_owned());
        }

        let metadata = AccountMetadata {
            labels,
            note: row.notes.and_then(optional_trimmed),
            project_path: row.folder.and_then(optional_trimmed),
            source: Some("bitwarden_csv".to_owned()),
            updated_at: 0,
        };

        let account = if totp_value.starts_with("otpauth://") {
            AccountRecord::from_otpauth_uri_with_metadata(totp_value, metadata)
        } else {
            AccountRecord::new_with_metadata(
                service,
                user,
                SecretString::from(totp_value.to_owned()),
                TotpConfig::default(),
                metadata,
            )
        }
        .with_context(|| format!("La fila {row_number} no contiene una cuenta TOTP válida"))?;

        candidates.push(BitwardenImportCandidate {
            row_number,
            account,
        });
    }

    Ok(candidates)
}

fn metadata_from_optional_fields(
    labels: Option<String>,
    note: Option<String>,
    project_path: Option<String>,
    source: Option<String>,
) -> AccountMetadata {
    AccountMetadata {
        labels: labels
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        note: note.and_then(optional_trimmed),
        project_path: project_path.and_then(optional_trimmed),
        source: source.and_then(optional_trimmed),
        updated_at: 0,
    }
}

fn optional_trimmed(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs};

    use mfa_forge_core::test_support::{base32_secret_from_seed, secret_string_from_seed};
    use tempfile::TempDir;

    use super::*;

    fn csv_secret(seed: &str) -> String {
        base32_secret_from_seed(seed)
    }

    fn csv_otpauth_uri(service: &str, user: &str, seed: &str) -> String {
        AccountRecord::new(
            service,
            user,
            secret_string_from_seed(seed),
            TotpConfig::default(),
        )
        .expect("seeded account should be valid")
        .otpauth_uri()
        .expect("seeded otpauth URI should build")
    }

    #[test]
    fn import_accounts_from_csv_parses_metadata_and_totp_fields() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let path = temp_dir.path().join("accounts.csv");
        let secret = csv_secret("csv-import");
        fs::write(
            &path,
            format!(
                "service,user,secret,algorithm,digits,period_seconds,labels,note,project_path,source\nGitHub,user@example.com,{secret},sha256,8,45,\"work,unused\",Primary,ClientA/Auth,manual\n"
            ),
        )
        .expect("csv should be written");

        let accounts = import_accounts_from_csv(&path).expect("csv should import");

        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].public.service, "GitHub");
        assert_eq!(accounts[0].public.user, "user@example.com");
        assert_eq!(accounts[0].public.totp.algorithm, TotpAlgorithm::Sha256);
        assert_eq!(accounts[0].public.totp.digits, 8);
        assert_eq!(accounts[0].public.totp.period_seconds, 45);
        assert_eq!(
            accounts[0].public.metadata.labels,
            vec!["unused".to_owned(), "work".to_owned()]
        );
        assert_eq!(
            accounts[0].public.metadata.project_path.as_deref(),
            Some("ClientA/Auth")
        );
    }

    #[test]
    fn preview_and_selective_import_bitwarden_csv() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let path = temp_dir.path().join("bitwarden.csv");
        let import_uri = csv_otpauth_uri("GitHub", "user@example.com", "bitwarden-uri");
        let raw_secret = csv_secret("bitwarden-raw");
        fs::write(
            &path,
            format!(
                "folder,favorite,type,name,notes,login_username,login_totp\nClientA/Auth,true,login,GitHub,Primary,user@example.com,{import_uri}\nClientB,false,login,GitLab,,dev@example.com,{raw_secret}\n"
            ),
        )
        .expect("csv should be written");

        let preview = preview_bitwarden_accounts(&path).expect("preview should work");
        assert_eq!(preview.len(), 2);
        assert_eq!(preview[0].row_number, 2);
        assert_eq!(preview[0].project_path.as_deref(), Some("ClientA/Auth"));

        let mut selected = BTreeSet::new();
        selected.insert(3);
        let imported = import_accounts_from_bitwarden_csv(&path, Some(&selected))
            .expect("selective import should work");
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].public.service, "GitLab");
    }

    #[test]
    fn export_accounts_metadata_csv_omits_secret_columns() {
        let account = AccountRecord::new_with_metadata(
            "GitHub",
            "user@example.com",
            secret_string_from_seed("csv-export"),
            TotpConfig::default(),
            AccountMetadata {
                labels: vec!["work".to_owned()],
                note: Some("Primary".to_owned()),
                project_path: Some("ClientA/Auth".to_owned()),
                source: Some("manual".to_owned()),
                updated_at: 0,
            },
        )
        .expect("account should be valid");

        let csv = export_accounts_metadata_csv(&[account.public_view()])
            .expect("metadata csv should render");

        assert!(csv.contains(
            "id,service,user,kind,algorithm,digits,period_seconds,created_at,updated_at,project_path,labels,note,source"
        ));
        assert!(csv.contains("GitHub,user@example.com,totp,sha1,6,30"));
        assert!(!csv.contains("secret"));
        assert!(!csv.contains(&base32_secret_from_seed("csv-export")));
    }
}

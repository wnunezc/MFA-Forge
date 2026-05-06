use secrecy::SecretString;
use serde::{Deserialize, Serialize, Serializer, de::Deserializer, ser::SerializeStruct};
use uuid::Uuid;

use crate::{CoreError, TotpToken, totp};

const ACCOUNT_LABEL_LIMIT: usize = 12;
const ACCOUNT_LABEL_LENGTH_LIMIT: usize = 32;
const ACCOUNT_NOTE_LENGTH_LIMIT: usize = 280;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FactorKind {
    Totp,
}

impl FactorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Totp => "totp",
        }
    }
}

impl std::fmt::Display for FactorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TotpAlgorithm {
    #[default]
    Sha1,
    Sha256,
    Sha512,
}

impl TotpAlgorithm {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
            Self::Sha512 => "sha512",
        }
    }
}

impl std::fmt::Display for TotpAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TotpConfig {
    pub algorithm: TotpAlgorithm,
    pub digits: u32,
    pub period_seconds: u64,
}

impl Default for TotpConfig {
    fn default() -> Self {
        Self {
            algorithm: TotpAlgorithm::Sha1,
            digits: 6,
            period_seconds: 30,
        }
    }
}

impl TotpConfig {
    pub fn validate(&self) -> Result<(), CoreError> {
        if !(6..=10).contains(&self.digits) {
            return Err(CoreError::InvalidTotpDigits);
        }
        if !(15..=300).contains(&self.period_seconds) {
            return Err(CoreError::InvalidTotpPeriod);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountMetadata {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub updated_at: u64,
}

impl AccountMetadata {
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
            && self.note.is_none()
            && self.project_path.is_none()
            && self.source.is_none()
    }

    pub fn labels_csv(&self) -> String {
        self.labels.join(", ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectDirectory {
    pub path: String,
    pub created_at: u64,
    pub updated_at: u64,
}

impl ProjectDirectory {
    /// Build a validated project directory path ready to be persisted.
    pub fn new(path: impl Into<String>) -> Result<Self, CoreError> {
        let timestamp = unix_timestamp_now()?;
        Self::with_timestamps(path, timestamp, timestamp)
    }

    /// Build a validated project directory path with explicit timestamps.
    pub fn with_timestamps(
        path: impl Into<String>,
        created_at: u64,
        updated_at: u64,
    ) -> Result<Self, CoreError> {
        Ok(Self {
            path: normalize_project_path_value(path)?,
            created_at,
            updated_at,
        })
    }

    /// Return the trailing visible folder name of the directory path.
    pub fn display_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(self.path.as_str())
    }

    /// Return the parent directory path when this entry belongs to a subtree.
    pub fn parent_path(&self) -> Option<&str> {
        self.path.rsplit_once('/').map(|(parent, _)| parent)
    }

    /// Return the directory nesting depth relative to the root.
    pub fn depth(&self) -> usize {
        self.path.split('/').count().saturating_sub(1)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccountHistoryEvent {
    Updated,
    Removed,
    Restored,
}

impl AccountHistoryEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Updated => "updated",
            Self::Removed => "removed",
            Self::Restored => "restored",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Updated => "Version previa",
            Self::Removed => "Cuenta eliminada",
            Self::Restored => "Restore anterior",
        }
    }
}

impl std::fmt::Display for AccountHistoryEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountHistoryEntryPublic {
    pub entry_id: Uuid,
    pub event: AccountHistoryEvent,
    pub captured_at: u64,
    pub account: AccountPublic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountPublic {
    pub id: Uuid,
    pub service: String,
    pub user: String,
    pub kind: FactorKind,
    pub totp: TotpConfig,
    pub created_at: u64,
    #[serde(default)]
    pub metadata: AccountMetadata,
}

impl AccountPublic {
    pub fn display_name(&self) -> String {
        format!("{} ({})", self.service, self.user)
    }

    pub fn sort_key(&self) -> (String, String) {
        (
            canonical_identity(&self.service),
            canonical_identity(&self.user),
        )
    }

    pub fn shares_identity_with(&self, other: &Self) -> bool {
        self.sort_key() == other.sort_key()
    }

    pub fn matches_query(&self, query: &str) -> bool {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return true;
        }

        trimmed
            .split_whitespace()
            .all(|token| self.matches_query_token(token))
    }

    pub fn monogram(&self) -> String {
        self.service
            .chars()
            .find(|character| character.is_alphanumeric())
            .map(|character| character.to_ascii_uppercase().to_string())
            .unwrap_or_else(|| "?".to_owned())
    }

    fn matches_query_token(&self, token: &str) -> bool {
        let normalized = canonical_identity(token);
        if normalized.is_empty() {
            return true;
        }

        if let Some((prefix, value)) = normalized.split_once(':') {
            return match prefix {
                "service" => contains_term(&self.service, value),
                "user" => contains_term(&self.user, value),
                "factor" | "kind" => contains_term(self.kind.as_str(), value),
                "label" | "labels" => self
                    .metadata
                    .labels
                    .iter()
                    .any(|label| contains_term(label, value)),
                "project" | "tree" | "path" => self
                    .metadata
                    .project_path
                    .as_deref()
                    .is_some_and(|project_path| contains_term(project_path, value)),
                "source" => self
                    .metadata
                    .source
                    .as_deref()
                    .is_some_and(|source| contains_term(source, value)),
                "note" => self
                    .metadata
                    .note
                    .as_deref()
                    .is_some_and(|note| contains_term(note, value)),
                _ => false,
            };
        }

        contains_term(&self.service, &normalized)
            || contains_term(&self.user, &normalized)
            || contains_term(self.kind.as_str(), &normalized)
            || self
                .metadata
                .labels
                .iter()
                .any(|label| contains_term(label, &normalized))
            || self
                .metadata
                .project_path
                .as_deref()
                .is_some_and(|project_path| contains_term(project_path, &normalized))
            || self
                .metadata
                .source
                .as_deref()
                .is_some_and(|source| contains_term(source, &normalized))
            || self
                .metadata
                .note
                .as_deref()
                .is_some_and(|note| contains_term(note, &normalized))
    }
}

#[derive(Debug, Clone)]
pub struct AccountRecord {
    pub public: AccountPublic,
    secret: SecretString,
}

impl AccountRecord {
    /// Build a validated TOTP account record ready to be persisted.
    pub fn new(
        service: impl Into<String>,
        user: impl Into<String>,
        secret: SecretString,
        config: TotpConfig,
    ) -> Result<Self, CoreError> {
        Self::new_with_metadata(service, user, secret, config, AccountMetadata::default())
    }

    /// Build a validated TOTP account record ready to be persisted with public metadata.
    pub fn new_with_metadata(
        service: impl Into<String>,
        user: impl Into<String>,
        secret: SecretString,
        config: TotpConfig,
        metadata: AccountMetadata,
    ) -> Result<Self, CoreError> {
        let (service, user, secret, config, mut metadata) =
            validate_account_input(service, user, secret, config, metadata)?;
        let created_at = unix_timestamp_now()?;
        metadata.updated_at = created_at;

        Ok(Self {
            public: AccountPublic {
                id: Uuid::new_v4(),
                service,
                user,
                kind: FactorKind::Totp,
                totp: config,
                created_at,
                metadata,
            },
            secret,
        })
    }

    /// Build a validated record from an otpauth URI.
    pub fn from_otpauth_uri(uri: &str) -> Result<Self, CoreError> {
        Self::from_otpauth_uri_with_metadata(uri, AccountMetadata::default())
    }

    /// Build a validated record from an otpauth URI plus public metadata.
    pub fn from_otpauth_uri_with_metadata(
        uri: &str,
        metadata: AccountMetadata,
    ) -> Result<Self, CoreError> {
        let imported = totp::parse_otpauth_uri(uri)?;
        Self::new_with_metadata(
            imported.service,
            imported.user,
            SecretString::from(imported.secret),
            imported.config,
            metadata,
        )
    }

    /// Produce an updated record while preserving the existing identity metadata.
    pub fn update(
        &self,
        service: impl Into<String>,
        user: impl Into<String>,
        secret: Option<SecretString>,
        config: TotpConfig,
    ) -> Result<Self, CoreError> {
        self.update_with_metadata(service, user, secret, config, self.public.metadata.clone())
    }

    /// Produce an updated record while replacing public metadata.
    pub fn update_with_metadata(
        &self,
        service: impl Into<String>,
        user: impl Into<String>,
        secret: Option<SecretString>,
        config: TotpConfig,
        metadata: AccountMetadata,
    ) -> Result<Self, CoreError> {
        let secret = secret.unwrap_or_else(|| self.secret.clone());
        let (service, user, secret, config, mut metadata) =
            validate_account_input(service, user, secret, config, metadata)?;
        metadata.updated_at = unix_timestamp_now()?;

        Ok(Self {
            public: AccountPublic {
                id: self.public.id,
                service,
                user,
                kind: self.public.kind,
                totp: config,
                created_at: self.public.created_at,
                metadata,
            },
            secret,
        })
    }

    pub fn public(&self) -> &AccountPublic {
        &self.public
    }

    pub fn public_view(&self) -> AccountPublic {
        self.public.clone()
    }

    pub fn shares_identity_with(&self, other: &AccountPublic) -> bool {
        self.public.shares_identity_with(other)
    }

    pub fn otpauth_uri(&self) -> Result<String, CoreError> {
        totp::build_otpauth_uri(&self.public, self.secret.expose_secret())
    }

    /// Generate the current token for this account.
    pub fn generate_current_token(&self) -> Result<TotpToken, CoreError> {
        self.generate_token_at(unix_timestamp_now()?)
    }

    /// Generate a deterministic token for a specific Unix timestamp.
    pub fn generate_token_at(&self, timestamp: u64) -> Result<TotpToken, CoreError> {
        totp::generate_token_at(&self.public, self.secret.expose_secret(), timestamp)
    }
}

impl Serialize for AccountRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AccountRecord", 2)?;
        state.serialize_field("public", &self.public)?;
        state.serialize_field("secret", self.secret.expose_secret())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for AccountRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct AccountRecordWire {
            public: AccountPublic,
            secret: String,
        }

        let wire = AccountRecordWire::deserialize(deserializer)?;
        Ok(Self {
            public: wire.public,
            secret: SecretString::from(wire.secret),
        })
    }
}

pub(crate) fn normalize_text_field(
    value: impl Into<String>,
    field: &'static str,
) -> Result<String, CoreError> {
    let trimmed = value.into().trim().to_owned();
    if trimmed.is_empty() {
        return Err(CoreError::EmptyField(field));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(CoreError::InvalidTextField(field));
    }
    Ok(trimmed)
}

pub(crate) fn canonical_identity(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn validate_account_input(
    service: impl Into<String>,
    user: impl Into<String>,
    secret: SecretString,
    config: TotpConfig,
    metadata: AccountMetadata,
) -> Result<(String, String, SecretString, TotpConfig, AccountMetadata), CoreError> {
    let service = normalize_text_field(service.into(), "service")?;
    let user = normalize_text_field(user.into(), "user")?;
    let normalized_secret = totp::validate_secret(secret.expose_secret())?;
    config.validate()?;
    let metadata = normalize_metadata(metadata)?;

    Ok((
        service,
        user,
        SecretString::from(normalized_secret),
        config,
        metadata,
    ))
}

fn normalize_metadata(metadata: AccountMetadata) -> Result<AccountMetadata, CoreError> {
    let labels = normalize_labels(metadata.labels)?;
    let note = normalize_note(metadata.note)?;
    let project_path = metadata
        .project_path
        .map(normalize_project_path)
        .transpose()?;
    let source = metadata
        .source
        .map(|value| normalize_text_field(value, "source"))
        .transpose()?;

    Ok(AccountMetadata {
        labels,
        note,
        project_path,
        source,
        updated_at: metadata.updated_at,
    })
}

fn normalize_labels(labels: Vec<String>) -> Result<Vec<String>, CoreError> {
    let mut normalized: Vec<String> = Vec::new();

    for raw_label in labels {
        for part in raw_label.split(',') {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed.chars().any(char::is_control) || trimmed.len() > ACCOUNT_LABEL_LENGTH_LIMIT {
                return Err(CoreError::InvalidTextField("label"));
            }

            if normalized
                .iter()
                .any(|existing| canonical_identity(existing) == canonical_identity(trimmed))
            {
                continue;
            }

            normalized.push(trimmed.to_owned());
        }
    }

    if normalized.len() > ACCOUNT_LABEL_LIMIT {
        return Err(CoreError::InvalidTextField("labels"));
    }

    normalized.sort_by_key(|label| canonical_identity(label));
    Ok(normalized)
}

fn normalize_note(note: Option<String>) -> Result<Option<String>, CoreError> {
    let Some(note) = note else {
        return Ok(None);
    };

    let trimmed = note.trim().to_owned();
    if trimmed.is_empty() {
        return Ok(None);
    }

    if trimmed.len() > ACCOUNT_NOTE_LENGTH_LIMIT
        || trimmed
            .chars()
            .any(|character| character.is_control() && character != '\n' && character != '\t')
    {
        return Err(CoreError::InvalidTextField("note"));
    }

    Ok(Some(trimmed))
}

/// Normalize a project directory path into the canonical slash-separated form.
pub fn normalize_project_path_value(project_path: impl Into<String>) -> Result<String, CoreError> {
    normalize_project_path(project_path.into())
}

fn normalize_project_path(project_path: String) -> Result<String, CoreError> {
    let normalized = normalize_text_field(project_path, "project_path")?.replace('\\', "/");
    let segments = normalized
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if segments.is_empty() {
        return Err(CoreError::InvalidTextField("project_path"));
    }

    if segments
        .iter()
        .any(|segment| *segment == "." || *segment == ".." || segment.chars().any(char::is_control))
    {
        return Err(CoreError::InvalidTextField("project_path"));
    }

    Ok(segments.join("/"))
}

fn contains_term(text: &str, term: &str) -> bool {
    canonical_identity(text).contains(term)
}

fn unix_timestamp_now() -> Result<u64, CoreError> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs())
}

use secrecy::ExposeSecret;

#[cfg(test)]
mod tests {
    use secrecy::SecretString;

    use super::{AccountMetadata, AccountRecord, TotpConfig, canonical_identity};

    #[test]
    fn account_creation_normalizes_identity() {
        let account = AccountRecord::new(
            "  GitHub  ",
            "  user@example.com  ",
            SecretString::from("jbswy3dpehpk3pxp".to_owned()),
            TotpConfig::default(),
        )
        .expect("account should be valid");

        assert_eq!(account.public.service, "GitHub");
        assert_eq!(account.public.user, "user@example.com");
    }

    #[test]
    fn canonical_identity_is_case_insensitive() {
        assert_eq!(canonical_identity("GitHub"), canonical_identity("github"));
    }

    #[test]
    fn account_update_preserves_existing_identity_metadata() {
        let account = AccountRecord::new(
            "GitHub",
            "user@example.com",
            SecretString::from("jbswy3dpehpk3pxp".to_owned()),
            TotpConfig::default(),
        )
        .expect("account should be valid");

        let updated = account
            .update(
                "GitHub Enterprise",
                "dev@example.com",
                None,
                TotpConfig {
                    digits: 8,
                    ..TotpConfig::default()
                },
            )
            .expect("account update should be valid");

        assert_eq!(updated.public.id, account.public.id);
        assert_eq!(updated.public.created_at, account.public.created_at);
        assert_eq!(updated.public.service, "GitHub Enterprise");
        assert_eq!(updated.public.user, "dev@example.com");
        assert_eq!(updated.public.totp.digits, 8);
    }

    #[test]
    fn account_can_be_imported_from_otpauth_uri() {
        let imported = AccountRecord::from_otpauth_uri(
            "otpauth://totp/GitHub:user%40example.com?secret=JBSWY3DPEHPK3PXP&issuer=GitHub",
        )
        .expect("otpauth URI should import");

        assert_eq!(imported.public.service, "GitHub");
        assert_eq!(imported.public.user, "user@example.com");
    }

    #[test]
    fn metadata_labels_are_trimmed_and_deduplicated() {
        let imported = AccountRecord::new_with_metadata(
            "GitHub",
            "user@example.com",
            SecretString::from("JBSWY3DPEHPK3PXP".to_owned()),
            TotpConfig::default(),
            AccountMetadata {
                labels: vec![" Work ".to_owned(), "work, critical".to_owned()],
                note: Some(" Primary account ".to_owned()),
                project_path: Some(r"ClientA\Auth\Prod".to_owned()),
                source: Some(" manual ".to_owned()),
                updated_at: 0,
            },
        )
        .expect("account should accept normalized metadata");

        assert_eq!(imported.public.metadata.labels, vec!["critical", "Work"]);
        assert_eq!(
            imported.public.metadata.note.as_deref(),
            Some("Primary account")
        );
        assert_eq!(
            imported.public.metadata.project_path.as_deref(),
            Some("ClientA/Auth/Prod")
        );
        assert_eq!(imported.public.metadata.source.as_deref(), Some("manual"));
        assert_eq!(
            imported.public.created_at,
            imported.public.metadata.updated_at
        );
    }

    #[test]
    fn query_matches_labels_and_prefixed_filters() {
        let account = AccountRecord::new_with_metadata(
            "GitHub",
            "user@example.com",
            SecretString::from("JBSWY3DPEHPK3PXP".to_owned()),
            TotpConfig::default(),
            AccountMetadata {
                labels: vec!["work".to_owned(), "critical".to_owned()],
                note: Some("SSH automation".to_owned()),
                project_path: Some("ClientA/Auth/Prod".to_owned()),
                source: Some("bitwarden_csv".to_owned()),
                updated_at: 0,
            },
        )
        .expect("account should be valid")
        .public_view();

        assert!(account.matches_query("work"));
        assert!(account.matches_query("label:critical"));
        assert!(account.matches_query("project:clienta/auth"));
        assert!(account.matches_query("source:bitwarden"));
        assert!(account.matches_query("service:github user:user@example.com"));
        assert!(!account.matches_query("label:personal"));
    }

    #[test]
    fn account_can_export_otpauth_uri() {
        let account = AccountRecord::new(
            "GitHub",
            "user@example.com",
            SecretString::from("JBSWY3DPEHPK3PXP".to_owned()),
            TotpConfig::default(),
        )
        .expect("account should be valid");

        let uri = account.otpauth_uri().expect("uri should build");
        assert!(uri.starts_with("otpauth://totp/"));
        assert!(uri.contains("issuer=GitHub"));
    }
}

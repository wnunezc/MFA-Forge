use std::collections::BTreeSet;

use mfa_forge_core::{
    AccountHistoryEntryPublic, AccountMetadata, AccountPublic, TotpAlgorithm, TotpConfig, TotpToken,
};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::i18n::{Language, tr};
use crate::theme::ThemePreference;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Loader,
    Main,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LoaderMode {
    Initialize,
    #[default]
    Unlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerTone {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Banner {
    pub tone: BannerTone,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum WorkspaceScope {
    #[default]
    Unassigned,
    Directory(String),
}

impl WorkspaceScope {
    pub fn directory_path(&self) -> Option<&str> {
        match self {
            Self::Unassigned => None,
            Self::Directory(path) => Some(path.as_str()),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Unassigned => tr("No workspace"),
            Self::Directory(path) => path.clone(),
        }
    }

    pub fn is_unassigned(&self) -> bool {
        matches!(self, Self::Unassigned)
    }
}

#[derive(Debug, Default)]
pub struct LoaderState {
    pub password_input: String,
    pub confirm_password_input: String,
    pub error: Option<String>,
    pub mode: LoaderMode,
}

impl LoaderState {
    pub fn current_mode(&self) -> LoaderMode {
        self.mode
    }
}

#[derive(Debug, Default, Clone)]
pub struct MetadataFormState {
    pub labels: String,
    pub note: String,
    pub project_path: String,
    pub source: String,
}

impl MetadataFormState {
    pub fn clear(&mut self) {
        self.labels.clear();
        self.note.clear();
        self.project_path.clear();
        self.source.clear();
    }

    pub fn load_from_account(&mut self, account: &AccountPublic) {
        self.labels = account.metadata.labels_csv();
        self.note = account.metadata.note.clone().unwrap_or_default();
        self.project_path = account.metadata.project_path.clone().unwrap_or_default();
        self.source = account.metadata.source.clone().unwrap_or_default();
    }

    pub fn set_project_path(&mut self, project_path: Option<&str>) {
        self.project_path = project_path.unwrap_or_default().to_owned();
    }

    pub fn to_metadata(&self) -> AccountMetadata {
        AccountMetadata {
            labels: self
                .labels
                .split(',')
                .map(str::trim)
                .filter(|label| !label.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            note: optional_trimmed(&self.note),
            project_path: optional_trimmed(&self.project_path),
            source: optional_trimmed(&self.source),
            updated_at: 0,
        }
    }
}

#[derive(Debug)]
pub struct AccountFormState {
    pub service: String,
    pub user: String,
    pub secret: String,
    pub algorithm: TotpAlgorithm,
    pub digits: String,
    pub period_seconds: String,
    pub metadata: MetadataFormState,
}

impl Default for AccountFormState {
    fn default() -> Self {
        Self {
            service: String::new(),
            user: String::new(),
            secret: String::new(),
            algorithm: TotpAlgorithm::default(),
            digits: "6".to_owned(),
            period_seconds: "30".to_owned(),
            metadata: MetadataFormState::default(),
        }
    }
}

impl AccountFormState {
    pub fn clear(&mut self) {
        self.service.clear();
        self.user.clear();
        self.secret.zeroize();
        self.secret.clear();
        self.algorithm = TotpAlgorithm::default();
        self.digits.clear();
        self.digits.push('6');
        self.period_seconds.clear();
        self.period_seconds.push_str("30");
        self.metadata.clear();
    }

    pub fn load_from_account(&mut self, account: &AccountPublic) {
        self.service = account.service.clone();
        self.user = account.user.clone();
        self.secret.zeroize();
        self.secret.clear();
        self.algorithm = account.totp.algorithm;
        self.digits = account.totp.digits.to_string();
        self.period_seconds = account.totp.period_seconds.to_string();
        self.metadata.load_from_account(account);
    }

    pub fn totp_config(&self) -> Result<TotpConfig, String> {
        let digits = self
            .digits
            .trim()
            .parse::<u32>()
            .map_err(|_| "TOTP digits must be a valid number.".to_owned())?;
        let period_seconds = self
            .period_seconds
            .trim()
            .parse::<u64>()
            .map_err(|_| "The TOTP period must be a valid number.".to_owned())?;

        let config = TotpConfig {
            algorithm: self.algorithm,
            digits,
            period_seconds,
        };
        config.validate().map_err(|error| error.to_string())?;
        Ok(config)
    }
}

#[derive(Debug, Default)]
pub struct AddDialogState {
    pub open: bool,
    pub form: AccountFormState,
    pub error: Option<String>,
}

impl AddDialogState {
    pub fn clear(&mut self) {
        self.open = false;
        self.form.clear();
        self.error = None;
    }
}

#[derive(Debug, Default)]
pub struct EditDialogState {
    pub open: bool,
    pub account_id: Option<Uuid>,
    pub form: AccountFormState,
    pub error: Option<String>,
}

impl EditDialogState {
    pub fn load_from_account(&mut self, account: &AccountPublic) {
        self.open = true;
        self.account_id = Some(account.id);
        self.form.load_from_account(account);
        self.error = None;
    }

    pub fn clear(&mut self) {
        self.open = false;
        self.account_id = None;
        self.form.clear();
        self.error = None;
    }
}

#[derive(Debug, Default)]
pub struct ImportDialogState {
    pub open: bool,
    pub uri: String,
    pub metadata: MetadataFormState,
    pub error: Option<String>,
    pub pending: bool,
}

impl ImportDialogState {
    pub fn clear(&mut self) {
        self.open = false;
        self.uri.zeroize();
        self.uri.clear();
        self.metadata.clear();
        self.error = None;
        self.pending = false;
    }
}

#[derive(Debug, Default)]
pub struct ImportQrDialogState {
    pub open: bool,
    pub image_path: String,
    pub metadata: MetadataFormState,
    pub error: Option<String>,
    pub pending: bool,
}

impl ImportQrDialogState {
    pub fn clear(&mut self) {
        self.open = false;
        self.image_path.clear();
        self.metadata.clear();
        self.error = None;
        self.pending = false;
    }
}

#[derive(Debug, Default)]
pub struct ImportFileDialogState {
    pub open: bool,
    pub file_path: String,
    pub metadata: MetadataFormState,
    pub error: Option<String>,
    pub pending: bool,
}

impl ImportFileDialogState {
    pub fn clear(&mut self) {
        self.open = false;
        self.file_path.clear();
        self.metadata.clear();
        self.error = None;
        self.pending = false;
    }
}

#[derive(Debug, Default)]
pub struct RestoreDialogState {
    pub open: bool,
    pub entries: Vec<AccountHistoryEntryPublic>,
    pub selected_entry_id: Option<Uuid>,
    pub error: Option<String>,
    pub pending: bool,
    pub pending_message: Option<String>,
}

impl RestoreDialogState {
    pub fn begin_pending(&mut self, message: impl Into<String>) {
        self.open = true;
        self.entries.clear();
        self.selected_entry_id = None;
        self.pending = true;
        self.pending_message = Some(message.into());
        self.error = None;
    }

    pub fn load_entries(&mut self, entries: Vec<AccountHistoryEntryPublic>) {
        let previous_selection = self.selected_entry_id;
        self.open = true;
        self.pending = false;
        self.pending_message = None;
        self.selected_entry_id = previous_selection
            .filter(|selected_id| entries.iter().any(|entry| entry.entry_id == *selected_id))
            .or_else(|| entries.first().map(|entry| entry.entry_id));
        self.entries = entries;
        self.error = None;
    }

    pub fn clear(&mut self) {
        self.open = false;
        self.entries.clear();
        self.selected_entry_id = None;
        self.error = None;
        self.pending = false;
        self.pending_message = None;
    }
}

#[derive(Debug, Default)]
pub struct CreateDirectoryDialogState {
    pub open: bool,
    pub parent_path: Option<String>,
    pub name: String,
    pub error: Option<String>,
    pub pending: bool,
}

impl CreateDirectoryDialogState {
    pub fn clear(&mut self) {
        self.open = false;
        self.parent_path = None;
        self.name.clear();
        self.error = None;
        self.pending = false;
    }
}

#[derive(Debug, Default)]
pub struct ChangePasswordDialogState {
    pub open: bool,
    pub new_password: String,
    pub confirm_password: String,
    pub error: Option<String>,
}

impl ChangePasswordDialogState {
    pub fn clear(&mut self) {
        self.open = false;
        self.new_password.zeroize();
        self.new_password.clear();
        self.confirm_password.zeroize();
        self.confirm_password.clear();
        self.error = None;
    }
}

#[derive(Debug, Default)]
pub struct RemoveDialogState {
    pub open: bool,
    pub account_ids: Vec<Uuid>,
    pub account_labels: Vec<String>,
    pub error: Option<String>,
    pub pending: bool,
}

impl RemoveDialogState {
    pub fn clear(&mut self) {
        self.open = false;
        self.account_ids.clear();
        self.account_labels.clear();
        self.error = None;
        self.pending = false;
    }

    pub fn load_accounts(&mut self, accounts: &[AccountPublic]) {
        self.open = true;
        self.account_ids = accounts.iter().map(|account| account.id).collect();
        self.account_labels = accounts.iter().map(AccountPublic::display_name).collect();
        self.error = None;
        self.pending = false;
    }
}

#[derive(Debug, Default)]
pub struct RemoveDirectoryDialogState {
    pub open: bool,
    pub path: String,
    pub error: Option<String>,
    pub pending: bool,
}

impl RemoveDirectoryDialogState {
    pub fn clear(&mut self) {
        self.open = false;
        self.path.clear();
        self.error = None;
        self.pending = false;
    }

    pub fn load_path(&mut self, path: impl Into<String>) {
        self.open = true;
        self.path = path.into();
        self.error = None;
        self.pending = false;
    }
}

#[derive(Debug, Default)]
pub struct TokenDialogState {
    pub open: bool,
    pub token: Option<TotpToken>,
    pub error: Option<String>,
    pub last_visible_second: Option<u64>,
    pub action_message: Option<String>,
    pub action_tone: Option<BannerTone>,
    pub refresh_count: u32,
    pub pending: bool,
}

impl TokenDialogState {
    pub fn close(&mut self) {
        self.open = false;
        self.token = None;
        self.error = None;
        self.last_visible_second = None;
        self.action_message = None;
        self.action_tone = None;
        self.refresh_count = 0;
        self.pending = false;
    }
}

#[derive(Debug, Default)]
pub struct ExportDialogState {
    pub open: bool,
    pub error: Option<String>,
    pub pending: bool,
}

impl ExportDialogState {
    pub fn close(&mut self) {
        self.open = false;
        self.error = None;
        self.pending = false;
    }
}

#[derive(Debug, Default)]
pub struct AccountUriDialogState {
    pub open: bool,
    pub account_label: String,
    pub uri: String,
    pub reveal: bool,
    pub error: Option<String>,
    pub pending: bool,
}

impl AccountUriDialogState {
    pub fn close(&mut self) {
        self.open = false;
        self.account_label.clear();
        self.uri.zeroize();
        self.uri.clear();
        self.reveal = false;
        self.error = None;
        self.pending = false;
    }
}

#[derive(Debug, Default)]
pub struct SearchState {
    pub active_query: String,
    pub pending_query: Option<String>,
    pub matched_account_ids: Vec<Uuid>,
}

impl SearchState {
    pub fn clear(&mut self) {
        self.active_query.clear();
        self.pending_query = None;
        self.matched_account_ids.clear();
    }

    pub fn is_active_for(&self, query: &str) -> bool {
        self.active_query == query
    }
}

#[derive(Debug, Default)]
pub struct NoticeDialogState {
    pub open: bool,
    pub title: String,
    pub message: String,
}

impl NoticeDialogState {
    pub fn show(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.open = true;
        self.title = title.into();
        self.message = message.into();
    }

    pub fn close(&mut self) {
        self.open = false;
        self.title.clear();
        self.message.clear();
    }
}

#[derive(Debug, Default)]
pub struct UpdateDialogState {
    pub open: bool,
    pub error: Option<String>,
}

impl UpdateDialogState {
    pub fn open(&mut self) {
        self.open = true;
        self.error = None;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.error = None;
    }
}

#[derive(Debug, Default)]
pub struct HelpDialogState {
    pub open: bool,
    pub search_query: String,
    pub selected_section: Option<usize>,
}

impl HelpDialogState {
    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.search_query.clear();
        self.selected_section = None;
    }
}

#[derive(Debug)]
pub struct AppState {
    pub screen: Screen,
    pub loader: LoaderState,
    pub theme_preference: ThemePreference,
    pub language: Language,
    pub search_query: String,
    pub workspace_scope: WorkspaceScope,
    pub selected_account_id: Option<Uuid>,
    pub checked_account_ids: BTreeSet<Uuid>,
    pub banner: Option<Banner>,
    pub add_dialog: AddDialogState,
    pub edit_dialog: EditDialogState,
    pub import_dialog: ImportDialogState,
    pub import_qr_dialog: ImportQrDialogState,
    pub import_file_dialog: ImportFileDialogState,
    pub restore_dialog: RestoreDialogState,
    pub create_directory_dialog: CreateDirectoryDialogState,
    pub change_password_dialog: ChangePasswordDialogState,
    pub remove_dialog: RemoveDialogState,
    pub remove_directory_dialog: RemoveDirectoryDialogState,
    pub token_dialog: TokenDialogState,
    pub export_dialog: ExportDialogState,
    pub account_uri_dialog: AccountUriDialogState,
    pub notice_dialog: NoticeDialogState,
    pub update_dialog: UpdateDialogState,
    pub help_dialog: HelpDialogState,
    pub search: SearchState,
}

impl AppState {
    pub fn new(vault_exists: bool, theme_preference: ThemePreference, language: Language) -> Self {
        Self {
            screen: Screen::Loader,
            loader: LoaderState {
                mode: if vault_exists {
                    LoaderMode::Unlock
                } else {
                    LoaderMode::Initialize
                },
                ..Default::default()
            },
            theme_preference,
            language,
            search_query: String::new(),
            workspace_scope: WorkspaceScope::Unassigned,
            selected_account_id: None,
            checked_account_ids: BTreeSet::new(),
            banner: None,
            add_dialog: AddDialogState::default(),
            edit_dialog: EditDialogState::default(),
            import_dialog: ImportDialogState::default(),
            import_qr_dialog: ImportQrDialogState::default(),
            import_file_dialog: ImportFileDialogState::default(),
            restore_dialog: RestoreDialogState::default(),
            create_directory_dialog: CreateDirectoryDialogState::default(),
            change_password_dialog: ChangePasswordDialogState::default(),
            remove_dialog: RemoveDialogState::default(),
            remove_directory_dialog: RemoveDirectoryDialogState::default(),
            token_dialog: TokenDialogState::default(),
            export_dialog: ExportDialogState::default(),
            account_uri_dialog: AccountUriDialogState::default(),
            notice_dialog: NoticeDialogState::default(),
            update_dialog: UpdateDialogState::default(),
            help_dialog: HelpDialogState::default(),
            search: SearchState::default(),
        }
    }
}

fn optional_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

use std::{
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime},
};

use serde_json::{Value, json};
use uuid::Uuid;

use mfa_forge_core::AccountPublic;

use crate::{diagnostics, vault::VaultFacade};

use super::super::{
    audit::{
        AuditEntry, AuditLogger, default_mcp_audit_path, read_recent_audit_events,
        summarize_recent_audit_events,
    },
    grant::{
        ACCOUNT_PROVISIONING_ALLOWED_TOOLS, ACCOUNT_PROVISIONING_GRANT_MAX_ACCOUNTS,
        ACCOUNT_PROVISIONING_GRANT_TTL, ACCOUNT_PROVISIONING_OPERATION,
        AUDIT_REPORTING_ALLOWED_TOOLS, AUDIT_REPORTING_GRANT_MAX_READS, AUDIT_REPORTING_GRANT_TTL,
        AUDIT_REPORTING_OPERATION, AuditReportingGrant, AuditReportingGrantFailure,
        GENERATE_TOKEN_GRANT_TTL, GENERATE_TOKEN_OPERATION, ProvisioningGrant,
        ProvisioningGrantFailure, TokenGrant, TokenGrantFailure,
    },
    prompt_helper,
    session::AgentSession,
    stdio_runtime::ProcessIdentity,
    unlock::{
        self, AuditReportingGrantPromptDecision, PasswordRotationPromptDecision,
        ProvisioningGrantPromptDecision, TokenGrantPromptDecision,
    },
};
use super::tools::{CreateAccountArgs, UpdateAccountArgs, validate_requested_account_limit};

pub(super) const SERVER_NAME: &str = "mfa-forge-mcp";
const WINDOWS_UNLOCK_STATUS: &str = "in_review";
const POST_UNLOCK_PROMPT_STABILIZATION: Duration = Duration::from_millis(500);
const DEFAULT_AUDIT_EVENT_LIMIT: usize = 25;
const MAX_AUDIT_EVENT_LIMIT: usize = 200;

pub(super) type TokenGrantPromptFn =
    fn(&AccountPublic, u64) -> Result<TokenGrantPromptDecision, String>;
pub(super) type ProvisioningGrantPromptFn =
    fn(u8, u64) -> Result<ProvisioningGrantPromptDecision, String>;
pub(super) type AuditReportingGrantPromptFn =
    fn(u8, u64) -> Result<AuditReportingGrantPromptDecision, String>;
pub(super) type PasswordRotationPromptFn = fn() -> Result<PasswordRotationPromptDecision, String>;

pub(super) struct SessionHost {
    pub(super) identity: ProcessIdentity,
    pub(super) vault_path: String,
    pub(super) audit_log_path: String,
    pub(super) vault_initialized: bool,
    pub(super) session: Option<ActiveMcpSession>,
    pub(super) audit: AuditLogger,
    pub(super) token_grant_prompt: TokenGrantPromptFn,
    pub(super) provisioning_grant_prompt: ProvisioningGrantPromptFn,
    pub(super) audit_reporting_grant_prompt: AuditReportingGrantPromptFn,
    pub(super) password_rotation_prompt: PasswordRotationPromptFn,
}

pub(super) struct ActiveMcpSession {
    pub(super) id: Uuid,
    pub(super) session: AgentSession,
    pub(super) token_grant: Option<TokenGrant>,
    pub(super) provisioning_grant: Option<ProvisioningGrant>,
    pub(super) audit_reporting_grant: Option<AuditReportingGrant>,
    pub(super) prompt_quiet_until: Instant,
}

impl SessionHost {
    pub(super) fn bootstrap() -> Result<Self, String> {
        let vault = VaultFacade::try_new().map_err(|error| error.to_string())?;
        let audit = AuditLogger::new(default_mcp_audit_path(&PathBuf::from(vault.path_display())));

        Ok(Self {
            identity: ProcessIdentity::new(),
            vault_path: vault.path_display().to_owned(),
            audit_log_path: audit.path().display().to_string(),
            vault_initialized: vault.is_initialized(),
            session: None,
            audit,
            token_grant_prompt: prompt_helper::request_generate_token_grant,
            provisioning_grant_prompt: prompt_helper::request_account_provisioning_grant,
            audit_reporting_grant_prompt: prompt_helper::request_audit_reporting_grant,
            password_rotation_prompt: prompt_helper::request_master_password_rotation,
        })
    }

    pub(super) fn health_value(&self) -> Value {
        json!({
            "status": "ok",
            "server": SERVER_NAME,
            "version": env!("CARGO_PKG_VERSION"),
            "process_id": self.identity.process_id,
            "instance_id": self.identity.instance_id,
            "started_at_epoch_ms": self.identity.started_at_epoch_ms,
            "protocol_versions": super::transport::SUPPORTED_PROTOCOL_VERSIONS,
            "vault_initialized": self.vault_initialized,
            "session_open": self.session_is_open(),
            "audit_log_path": self.audit_log_path,
            "generate_token_grant_required": true,
            "account_provisioning_grant_required": true,
            "audit_reporting_grant_required": true,
        })
    }

    pub(super) fn session_info_value(&mut self) -> Value {
        self.cleanup_expired_token_grant();
        self.cleanup_expired_provisioning_grant();
        self.cleanup_expired_audit_reporting_grant();
        json!({
            "status": if self.session_is_open() { "access_granted" } else { "locked" },
            "process_id": self.identity.process_id,
            "instance_id": self.identity.instance_id,
            "started_at_epoch_ms": self.identity.started_at_epoch_ms,
            "session_id": self.session.as_ref().map(|session| session.id),
            "vault_path": self.vault_path,
            "audit_log_path": self.audit_log_path,
            "vault_initialized": self.vault_initialized,
            "session_open": self.session_is_open(),
            "account_count": self.session.as_ref().map(|session| session.session.account_count()).unwrap_or(0),
            "windows_reinforced_unlock": WINDOWS_UNLOCK_STATUS,
            "generate_token_policy": self.generate_token_policy_value(SystemTime::now()),
            "account_provisioning_policy": self.account_provisioning_policy_value(SystemTime::now()),
            "audit_reporting_policy": self.audit_reporting_policy_value(SystemTime::now()),
        })
    }

    pub(super) fn open_session(&mut self) -> Result<Value, String> {
        diagnostics::log_event("mcp", "open_session.start", json!({}));
        self.cleanup_expired_token_grant();
        self.cleanup_expired_provisioning_grant();
        self.cleanup_expired_audit_reporting_grant();

        if self.session_is_open() {
            return Ok(json!({
                "status": "access_granted",
                "process_id": self.identity.process_id,
                "instance_id": self.identity.instance_id,
                "started_at_epoch_ms": self.identity.started_at_epoch_ms,
                "session_id": self.session.as_ref().map(|session| session.id),
                "vault_path": self.vault_path,
                "audit_log_path": self.audit_log_path,
                "account_count": self.session.as_ref().map(|session| session.session.account_count()).unwrap_or(0),
                "windows_reinforced_unlock": WINDOWS_UNLOCK_STATUS,
                "generate_token_policy": self.generate_token_policy_value(SystemTime::now()),
                "account_provisioning_policy": self.account_provisioning_policy_value(SystemTime::now()),
                "audit_reporting_policy": self.audit_reporting_policy_value(SystemTime::now()),
                "message": "La sesión MCP ya estaba abierta para este proceso. generate_token sigue requiriendo grant_generate_token; create_account/import_otpauth/update_account/remove_account siguen requiriendo grant_account_provisioning; list_history/read_audit_events/summarize_audit_events siguen requiriendo grant_audit_reporting.",
            }));
        }

        if !self.vault_initialized {
            return Err(
                "El vault aún no está inicializado. Usa la GUI o la CLI humana antes de exponer MFA-Forge por MCP."
                    .to_owned(),
            );
        }

        let vault = unlock::run_unlock_window()?;
        self.vault_path = vault.path_display().to_owned();
        self.vault_initialized = vault.is_initialized();
        let session = AgentSession::new(vault);
        let account_count = session.account_count();
        let session_id = Uuid::new_v4();

        self.session = Some(ActiveMcpSession {
            id: session_id,
            session,
            token_grant: None,
            provisioning_grant: None,
            audit_reporting_grant: None,
            prompt_quiet_until: Instant::now() + POST_UNLOCK_PROMPT_STABILIZATION,
        });

        let _ = self.audit.record(
            AuditEntry::new(session_id, "session_open", "granted")
                .with_details(json!({ "account_count": account_count })),
        );
        diagnostics::log_event(
            "mcp",
            "open_session.granted",
            json!({
                "session_id": session_id.to_string(),
                "account_count": account_count,
            }),
        );

        Ok(json!({
            "status": "access_granted",
            "process_id": self.identity.process_id,
            "instance_id": self.identity.instance_id,
            "started_at_epoch_ms": self.identity.started_at_epoch_ms,
            "session_id": session_id,
            "vault_path": self.vault_path,
            "audit_log_path": self.audit_log_path,
            "account_count": account_count,
            "windows_reinforced_unlock": WINDOWS_UNLOCK_STATUS,
            "generate_token_policy": self.generate_token_policy_value(SystemTime::now()),
            "account_provisioning_policy": self.account_provisioning_policy_value(SystemTime::now()),
            "audit_reporting_policy": self.audit_reporting_policy_value(SystemTime::now()),
            "message": "La sesión MCP quedó abierta mientras este proceso siga vivo o hasta llamar close_session. generate_token requiere grant_generate_token por cuenta; create_account/import_otpauth/update_account/remove_account requieren grant_account_provisioning; list_history/read_audit_events/summarize_audit_events requieren grant_audit_reporting.",
        }))
    }

    pub(super) fn list_accounts_value(&self) -> Result<Value, String> {
        let session = self.require_open_session()?;
        Ok(json!({ "accounts": session.list_accounts() }))
    }

    pub(super) fn get_account_metadata_value(&self, account_id: Uuid) -> Result<Value, String> {
        let session = self.require_open_session()?;
        let account = session
            .account_by_id(account_id)
            .ok_or_else(|| format!("No se encontró una cuenta con id {account_id}."))?;
        Ok(json!({ "account": account }))
    }

    pub(super) fn grant_audit_reporting_value(
        &mut self,
        requested_read_limit: u8,
    ) -> Result<Value, String> {
        diagnostics::log_event(
            "mcp",
            "grant_audit_reporting.start",
            json!({ "requested_read_limit": requested_read_limit }),
        );
        self.cleanup_expired_audit_reporting_grant();

        let requested_read_limit = validate_requested_read_limit(requested_read_limit)?;
        let session_id = self.require_open_session_mut()?.id;

        if let Some(active_session) = self.session.as_mut() {
            active_session.audit_reporting_grant = None;
        }

        self.wait_for_prompt_stability("grant_audit_reporting");
        match (self.audit_reporting_grant_prompt)(
            requested_read_limit,
            AUDIT_REPORTING_GRANT_TTL.as_secs() / 60,
        )? {
            AuditReportingGrantPromptDecision::Approved => {
                diagnostics::log_event(
                    "mcp",
                    "grant_audit_reporting.approved",
                    json!({ "requested_read_limit": requested_read_limit }),
                );
                let grant = AuditReportingGrant::new(requested_read_limit);
                let snapshot = grant.snapshot_value(SystemTime::now());
                self.audit.record(
                    AuditEntry::new(session_id, "audit_reporting_grant", "granted")
                        .with_operation(AUDIT_REPORTING_OPERATION)
                        .with_details(json!({
                            "requested_read_limit": requested_read_limit,
                            "grant_ttl_seconds": AUDIT_REPORTING_GRANT_TTL.as_secs(),
                            "expires_at_epoch_ms": grant.expires_at_epoch_ms(),
                            "remaining_reads": grant.remaining_reads(),
                            "allowed_tools": AUDIT_REPORTING_ALLOWED_TOOLS,
                        })),
                )?;

                let active_session = self.require_open_session_mut()?;
                active_session.audit_reporting_grant = Some(grant);

                Ok(json!({
                    "status": "granted",
                    "message": format!(
                        "Se aprobó un grant temporal para hasta {} lecturas sensibles por MCP.",
                        requested_read_limit
                    ),
                    "grant": snapshot,
                }))
            }
            AuditReportingGrantPromptDecision::Denied => {
                diagnostics::log_event(
                    "mcp",
                    "grant_audit_reporting.denied",
                    json!({ "requested_read_limit": requested_read_limit }),
                );
                let _ = self.audit.record(
                    AuditEntry::new(session_id, "audit_reporting_grant", "denied")
                        .with_operation(AUDIT_REPORTING_OPERATION)
                        .with_details(json!({
                            "requested_read_limit": requested_read_limit,
                            "grant_ttl_seconds": AUDIT_REPORTING_GRANT_TTL.as_secs(),
                            "allowed_tools": AUDIT_REPORTING_ALLOWED_TOOLS,
                        })),
                );

                Ok(json!({
                    "status": "denied",
                    "message": "El usuario denegó el grant para revisar historial o auditoría local.",
                    "grant": Value::Null,
                }))
            }
        }
    }

    pub(super) fn grant_account_provisioning_value(
        &mut self,
        requested_account_limit: u8,
    ) -> Result<Value, String> {
        diagnostics::log_event(
            "mcp",
            "grant_account_provisioning.start",
            json!({ "requested_account_limit": requested_account_limit }),
        );
        self.cleanup_expired_provisioning_grant();

        let requested_account_limit = validate_requested_account_limit(requested_account_limit)?;
        let session_id = self.require_open_session_mut()?.id;

        if let Some(active_session) = self.session.as_mut() {
            active_session.provisioning_grant = None;
        }

        self.wait_for_prompt_stability("grant_account_provisioning");
        match (self.provisioning_grant_prompt)(
            requested_account_limit,
            ACCOUNT_PROVISIONING_GRANT_TTL.as_secs() / 60,
        )? {
            ProvisioningGrantPromptDecision::Approved => {
                diagnostics::log_event(
                    "mcp",
                    "grant_account_provisioning.approved",
                    json!({ "requested_account_limit": requested_account_limit }),
                );
                let grant = ProvisioningGrant::new(requested_account_limit);
                let snapshot = grant.snapshot_value(SystemTime::now());
                self.audit.record(
                    AuditEntry::new(session_id, "account_provisioning_grant", "granted")
                        .with_operation(ACCOUNT_PROVISIONING_OPERATION)
                        .with_details(json!({
                            "requested_account_limit": requested_account_limit,
                            "grant_ttl_seconds": ACCOUNT_PROVISIONING_GRANT_TTL.as_secs(),
                            "expires_at_epoch_ms": grant.expires_at_epoch_ms(),
                            "remaining_accounts": grant.remaining_accounts(),
                            "allowed_tools": ACCOUNT_PROVISIONING_ALLOWED_TOOLS,
                        })),
                )?;

                let active_session = self.require_open_session_mut()?;
                active_session.provisioning_grant = Some(grant);

                Ok(json!({
                    "status": "granted",
                    "message": format!(
                        "Se aprobó un provisioning grant temporal para hasta {} cuentas nuevas por MCP.",
                        requested_account_limit
                    ),
                    "grant": snapshot,
                }))
            }
            ProvisioningGrantPromptDecision::Denied => {
                diagnostics::log_event(
                    "mcp",
                    "grant_account_provisioning.denied",
                    json!({ "requested_account_limit": requested_account_limit }),
                );
                let _ = self.audit.record(
                    AuditEntry::new(session_id, "account_provisioning_grant", "denied")
                        .with_operation(ACCOUNT_PROVISIONING_OPERATION)
                        .with_details(json!({
                            "requested_account_limit": requested_account_limit,
                            "grant_ttl_seconds": ACCOUNT_PROVISIONING_GRANT_TTL.as_secs(),
                            "allowed_tools": ACCOUNT_PROVISIONING_ALLOWED_TOOLS,
                        })),
                );

                Ok(json!({
                    "status": "denied",
                    "message": "El usuario denegó el provisioning grant para crear/importar cuentas MFA.",
                    "grant": Value::Null,
                }))
            }
        }
    }

    pub(super) fn create_account_value(
        &mut self,
        args: CreateAccountArgs,
    ) -> Result<Value, String> {
        self.cleanup_expired_provisioning_grant();

        let session_id = self.require_open_session_mut()?.id;
        self.require_account_provisioning_grant(session_id, "create_account")?;

        let account = {
            let active_session = self.require_open_session_mut()?;
            active_session.session.add_account(
                args.service,
                args.user,
                args.secret,
                args.totp.unwrap_or_default(),
            )?
        };

        self.audit.record(
            AuditEntry::new(session_id, "create_account", "created")
                .with_operation("create_account")
                .with_account(&account)
                .with_details(json!({
                    "grant_mode": "quota",
                })),
        )?;
        self.consume_account_provisioning_grant()?;

        Ok(json!({ "account": account }))
    }

    pub(super) fn import_otpauth_value(&mut self, uri: String) -> Result<Value, String> {
        self.cleanup_expired_provisioning_grant();

        let session_id = self.require_open_session_mut()?.id;
        self.require_account_provisioning_grant(session_id, "import_otpauth")?;

        let account = {
            let active_session = self.require_open_session_mut()?;
            active_session.session.import_otpauth(&uri)?
        };

        self.audit.record(
            AuditEntry::new(session_id, "import_otpauth", "imported")
                .with_operation("import_otpauth")
                .with_account(&account)
                .with_details(json!({
                    "grant_mode": "quota",
                })),
        )?;
        self.consume_account_provisioning_grant()?;

        Ok(json!({ "account": account }))
    }

    pub(super) fn update_account_value(
        &mut self,
        args: UpdateAccountArgs,
    ) -> Result<Value, String> {
        self.cleanup_expired_provisioning_grant();

        let session_id = self.require_open_session_mut()?.id;
        self.require_account_provisioning_grant(session_id, "update_account")?;

        let account = {
            let active_session = self.require_open_session_mut()?;
            active_session.session.update_account(
                args.account_id,
                args.service,
                args.user,
                args.secret,
                args.totp,
            )?
        };

        self.audit.record(
            AuditEntry::new(session_id, "update_account", "updated")
                .with_operation("update_account")
                .with_account(&account)
                .with_details(json!({
                    "grant_mode": "quota",
                })),
        )?;
        self.consume_account_provisioning_grant()?;

        Ok(json!({ "account": account }))
    }

    pub(super) fn remove_account_value(&mut self, account_id: Uuid) -> Result<Value, String> {
        self.cleanup_expired_provisioning_grant();

        let session_id = self.require_open_session_mut()?.id;
        self.require_account_provisioning_grant(session_id, "remove_account")?;

        let account = {
            let active_session = self.require_open_session_mut()?;
            active_session.session.remove_account(account_id)?
        };

        self.audit.record(
            AuditEntry::new(session_id, "remove_account", "removed")
                .with_operation("remove_account")
                .with_account(&account)
                .with_details(json!({
                    "grant_mode": "quota",
                })),
        )?;
        self.consume_account_provisioning_grant()?;

        Ok(json!({ "account": account }))
    }

    pub(super) fn grant_generate_token_value(&mut self, account_id: Uuid) -> Result<Value, String> {
        diagnostics::log_event(
            "mcp",
            "grant_generate_token.start",
            json!({ "account_id": account_id.to_string() }),
        );
        self.cleanup_expired_token_grant();

        let (session_id, account) = {
            let active_session = self.require_open_session_mut()?;
            let account = active_session
                .session
                .account_by_id(account_id)
                .ok_or_else(|| format!("No se encontró una cuenta con id {account_id}."))?;
            active_session.token_grant = None;
            (active_session.id, account)
        };

        self.wait_for_prompt_stability("grant_generate_token");
        match (self.token_grant_prompt)(&account, GENERATE_TOKEN_GRANT_TTL.as_secs())? {
            TokenGrantPromptDecision::Approved => {
                diagnostics::log_event(
                    "mcp",
                    "grant_generate_token.approved",
                    json!({ "account_id": account.id.to_string() }),
                );
                let grant = TokenGrant::new(account.id);
                let snapshot = grant.snapshot_value(SystemTime::now());
                self.audit.record(
                    AuditEntry::new(session_id, "generate_token_grant", "granted")
                        .with_operation(GENERATE_TOKEN_OPERATION)
                        .with_account(&account)
                        .with_details(json!({
                            "grant_mode": "single_use",
                            "grant_ttl_seconds": GENERATE_TOKEN_GRANT_TTL.as_secs(),
                            "expires_at_epoch_ms": grant.expires_at_epoch_ms(),
                            "remaining_uses": grant.remaining_uses(),
                        })),
                )?;

                let active_session = self.require_open_session_mut()?;
                active_session.token_grant = Some(grant);

                Ok(json!({
                    "status": "granted",
                    "message": "Se aprobó un grant explícito de un solo uso para generate_token.",
                    "grant": snapshot,
                }))
            }
            TokenGrantPromptDecision::Denied => {
                diagnostics::log_event(
                    "mcp",
                    "grant_generate_token.denied",
                    json!({ "account_id": account.id.to_string() }),
                );
                let _ = self.audit.record(
                    AuditEntry::new(session_id, "generate_token_grant", "denied")
                        .with_operation(GENERATE_TOKEN_OPERATION)
                        .with_account(&account)
                        .with_details(json!({
                            "grant_mode": "single_use",
                            "grant_ttl_seconds": GENERATE_TOKEN_GRANT_TTL.as_secs(),
                        })),
                );

                Ok(json!({
                    "status": "denied",
                    "message": "El usuario denegó el grant explícito para generate_token.",
                    "grant": Value::Null,
                }))
            }
        }
    }

    pub(super) fn generate_token_value(&mut self, account_id: Uuid) -> Result<Value, String> {
        self.cleanup_expired_token_grant();

        let (session_id, account) = {
            let active_session = self.require_open_session_mut()?;
            let account = active_session
                .session
                .account_by_id(account_id)
                .ok_or_else(|| format!("No se encontró una cuenta con id {account_id}."))?;
            (active_session.id, account)
        };

        self.require_generate_token_grant(session_id, &account)?;

        let token = {
            let active_session = self.require_open_session_mut()?;
            active_session.session.generate_token(account_id)?
        };

        self.audit.record(
            AuditEntry::new(session_id, "generate_token", "delivered")
                .with_operation(GENERATE_TOKEN_OPERATION)
                .with_account(&account)
                .with_details(json!({
                    "grant_mode": "single_use",
                })),
        )?;
        self.consume_generate_token_grant()?;

        serde_json::to_value(json!({ "token": token }))
            .map_err(|error| format!("No se pudo serializar el token: {error}"))
    }

    pub(super) fn export_metadata_value(&self) -> Result<Value, String> {
        let session = self.require_open_session()?;
        let accounts = session.export_metadata()?;
        Ok(json!({ "accounts": accounts }))
    }

    pub(super) fn list_history_value(&mut self) -> Result<Value, String> {
        self.cleanup_expired_audit_reporting_grant();

        let session_id = self.require_open_session_mut()?.id;
        self.require_audit_reporting_grant(session_id, "list_history")?;

        let entries = {
            let active_session = self.require_open_session_mut()?;
            active_session.session.history()?
        };

        self.audit.record(
            AuditEntry::new(session_id, "list_history", "read")
                .with_operation("list_history")
                .with_details(json!({
                    "grant_mode": "quota",
                    "entry_count": entries.len(),
                })),
        )?;
        self.consume_audit_reporting_grant()?;

        Ok(json!({ "entries": entries }))
    }

    pub(super) fn read_audit_events_value(&mut self, limit: usize) -> Result<Value, String> {
        self.cleanup_expired_audit_reporting_grant();

        let session_id = self.require_open_session_mut()?.id;
        self.require_audit_reporting_grant(session_id, "read_audit_events")?;
        let limit = normalize_audit_event_limit(limit);
        let events = read_recent_audit_events(Path::new(&self.audit_log_path), limit)?;

        self.audit.record(
            AuditEntry::new(session_id, "read_audit_events", "read")
                .with_operation("read_audit_events")
                .with_details(json!({
                    "grant_mode": "quota",
                    "event_count": events.len(),
                    "requested_limit": limit,
                })),
        )?;
        self.consume_audit_reporting_grant()?;

        Ok(json!({ "events": events, "limit": limit }))
    }

    pub(super) fn summarize_audit_events_value(&mut self, limit: usize) -> Result<Value, String> {
        self.cleanup_expired_audit_reporting_grant();

        let session_id = self.require_open_session_mut()?.id;
        self.require_audit_reporting_grant(session_id, "summarize_audit_events")?;
        let limit = normalize_audit_event_limit(limit);
        let summary = summarize_recent_audit_events(Path::new(&self.audit_log_path), limit)?;

        self.audit.record(
            AuditEntry::new(session_id, "summarize_audit_events", "read")
                .with_operation("summarize_audit_events")
                .with_details(json!({
                    "grant_mode": "quota",
                    "requested_limit": limit,
                    "events_considered": summary.total_events_considered,
                })),
        )?;
        self.consume_audit_reporting_grant()?;

        Ok(json!({ "summary": summary, "limit": limit }))
    }

    pub(super) fn rotate_master_password_value(&mut self) -> Result<Value, String> {
        let session_id = self.require_open_session_mut()?.id;

        self.wait_for_prompt_stability("rotate_master_password");
        match (self.password_rotation_prompt)()? {
            PasswordRotationPromptDecision::Approved { new_password } => {
                let active_session = self.require_open_session_mut()?;
                active_session
                    .session
                    .change_master_password(new_password)?;

                self.audit.record(
                    AuditEntry::new(session_id, "rotate_master_password", "rotated")
                        .with_operation("rotate_master_password"),
                )?;

                Ok(json!({
                    "status": "rotated",
                    "message": "La contraseña maestra fue rotada y el vault quedó re-cifrado.",
                }))
            }
            PasswordRotationPromptDecision::Denied => {
                let _ = self.audit.record(
                    AuditEntry::new(session_id, "rotate_master_password", "denied")
                        .with_operation("rotate_master_password"),
                );

                Ok(json!({
                    "status": "denied",
                    "message": "El usuario denegó la rotación de la contraseña maestra.",
                }))
            }
        }
    }

    pub(super) fn close_session_value(&mut self) -> Value {
        if let Some(mut active_session) = self.session.take() {
            active_session.session.close();
            let _ = self.audit.record(AuditEntry::new(
                active_session.id,
                "session_close",
                "closed",
            ));
            return json!({ "status": "closed" });
        }

        json!({ "status": "already_closed" })
    }

    fn session_is_open(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|session| session.session.is_unlocked())
    }

    fn require_open_session(&self) -> Result<&AgentSession, String> {
        self.session
            .as_ref()
            .filter(|session| session.session.is_unlocked())
            .map(|session| &session.session)
            .ok_or_else(|| {
                "La sesión MCP está bloqueada. Llama primero a open_session para solicitar el unlock nativo."
                    .to_owned()
            })
    }

    fn require_open_session_mut(&mut self) -> Result<&mut ActiveMcpSession, String> {
        self.session
            .as_mut()
            .filter(|session| session.session.is_unlocked())
            .ok_or_else(|| {
                "La sesión MCP está bloqueada. Llama primero a open_session para solicitar el unlock nativo."
                    .to_owned()
            })
    }

    fn generate_token_policy_value(&self, now: SystemTime) -> Value {
        let active_grant = self
            .session
            .as_ref()
            .and_then(|session| {
                session
                    .token_grant
                    .as_ref()
                    .map(|grant| grant.snapshot_value(now))
            })
            .unwrap_or_else(super::super::grant::empty_grant_snapshot_value);

        json!({
            "grant_required": true,
            "grant_tool": "grant_generate_token",
            "grant_mode": "single_use",
            "grant_ttl_seconds": GENERATE_TOKEN_GRANT_TTL.as_secs(),
            "active_grant": active_grant,
        })
    }

    fn account_provisioning_policy_value(&self, now: SystemTime) -> Value {
        let active_grant = self
            .session
            .as_ref()
            .and_then(|session| {
                session
                    .provisioning_grant
                    .as_ref()
                    .map(|grant| grant.snapshot_value(now))
            })
            .unwrap_or_else(super::super::grant::empty_provisioning_grant_snapshot_value);

        json!({
            "grant_required": true,
            "grant_tool": "grant_account_provisioning",
            "max_accounts_per_grant": ACCOUNT_PROVISIONING_GRANT_MAX_ACCOUNTS,
            "grant_ttl_seconds": ACCOUNT_PROVISIONING_GRANT_TTL.as_secs(),
            "allowed_tools": ACCOUNT_PROVISIONING_ALLOWED_TOOLS,
            "active_grant": active_grant,
        })
    }

    fn audit_reporting_policy_value(&self, now: SystemTime) -> Value {
        let active_grant = self
            .session
            .as_ref()
            .and_then(|session| {
                session
                    .audit_reporting_grant
                    .as_ref()
                    .map(|grant| grant.snapshot_value(now))
            })
            .unwrap_or_else(super::super::grant::empty_audit_reporting_grant_snapshot_value);

        json!({
            "grant_required": true,
            "grant_tool": "grant_audit_reporting",
            "max_reads_per_grant": AUDIT_REPORTING_GRANT_MAX_READS,
            "grant_ttl_seconds": AUDIT_REPORTING_GRANT_TTL.as_secs(),
            "allowed_tools": AUDIT_REPORTING_ALLOWED_TOOLS,
            "active_grant": active_grant,
        })
    }

    fn require_generate_token_grant(
        &mut self,
        session_id: Uuid,
        account: &AccountPublic,
    ) -> Result<(), String> {
        let failure = {
            let active_session = self.require_open_session_mut()?;
            match active_session.token_grant.as_ref() {
                Some(grant) => grant.verify(account.id, SystemTime::now()).err(),
                None => Some(TokenGrantFailure::Missing),
            }
        };

        if let Some(failure) = failure {
            let _ = self.audit.record(
                AuditEntry::new(session_id, "generate_token", failure.audit_result())
                    .with_operation(GENERATE_TOKEN_OPERATION)
                    .with_account(account),
            );
            return Err(failure.user_message().to_owned());
        }

        Ok(())
    }

    fn require_account_provisioning_grant(
        &mut self,
        session_id: Uuid,
        operation: &'static str,
    ) -> Result<(), String> {
        let failure = {
            let active_session = self.require_open_session_mut()?;
            match active_session.provisioning_grant.as_ref() {
                Some(grant) => grant.verify(SystemTime::now()).err(),
                None => Some(ProvisioningGrantFailure::Missing),
            }
        };

        if let Some(failure) = failure {
            let _ = self.audit.record(
                AuditEntry::new(session_id, operation, failure.audit_result())
                    .with_operation(operation),
            );
            return Err(failure.user_message().to_owned());
        }

        Ok(())
    }

    fn require_audit_reporting_grant(
        &mut self,
        session_id: Uuid,
        operation: &'static str,
    ) -> Result<(), String> {
        let failure = {
            let active_session = self.require_open_session_mut()?;
            match active_session.audit_reporting_grant.as_ref() {
                Some(grant) => grant.verify(SystemTime::now()).err(),
                None => Some(AuditReportingGrantFailure::Missing),
            }
        };

        if let Some(failure) = failure {
            let _ = self.audit.record(
                AuditEntry::new(session_id, operation, failure.audit_result())
                    .with_operation(operation),
            );
            return Err(failure.user_message().to_owned());
        }

        Ok(())
    }

    fn consume_generate_token_grant(&mut self) -> Result<(), String> {
        let active_session = self.require_open_session_mut()?;
        let should_clear = {
            let grant = active_session.token_grant.as_mut().ok_or_else(|| {
                "El grant explícito de generate_token ya no está disponible.".to_owned()
            })?;
            grant
                .consume_one()
                .map_err(|failure| failure.user_message().to_owned())?;
            grant.remaining_uses() == 0
        };

        if should_clear {
            active_session.token_grant = None;
        }

        Ok(())
    }

    fn consume_account_provisioning_grant(&mut self) -> Result<(), String> {
        let active_session = self.require_open_session_mut()?;
        let should_clear = {
            let grant = active_session
                .provisioning_grant
                .as_mut()
                .ok_or_else(|| "El provisioning grant ya no está disponible.".to_owned())?;
            grant
                .consume_one()
                .map_err(|failure| failure.user_message().to_owned())?;
            grant.remaining_accounts() == 0
        };

        if should_clear {
            active_session.provisioning_grant = None;
        }

        Ok(())
    }

    fn consume_audit_reporting_grant(&mut self) -> Result<(), String> {
        let active_session = self.require_open_session_mut()?;
        let should_clear = {
            let grant = active_session
                .audit_reporting_grant
                .as_mut()
                .ok_or_else(|| {
                    "El grant de reporting sensible ya no está disponible.".to_owned()
                })?;
            grant
                .consume_one()
                .map_err(|failure| failure.user_message().to_owned())?;
            grant.remaining_reads() == 0
        };

        if should_clear {
            active_session.audit_reporting_grant = None;
        }

        Ok(())
    }

    fn cleanup_expired_token_grant(&mut self) {
        let expired = if let Some(active_session) = self.session.as_mut() {
            if let Some(grant) = active_session.token_grant.as_ref() {
                if grant.has_expired(SystemTime::now()) {
                    let session_id = active_session.id;
                    let account = active_session.session.account_by_id(grant.account_id());
                    let expires_at_epoch_ms = grant.expires_at_epoch_ms();
                    active_session.token_grant = None;
                    Some((session_id, account, expires_at_epoch_ms))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some((session_id, account, expires_at_epoch_ms)) = expired {
            let entry = AuditEntry::new(session_id, "generate_token_grant", "expired")
                .with_operation(GENERATE_TOKEN_OPERATION)
                .with_details(json!({
                    "expires_at_epoch_ms": expires_at_epoch_ms,
                }));

            let entry = if let Some(account) = account {
                entry.with_account(&account)
            } else {
                entry
            };

            let _ = self.audit.record(entry);
        }
    }

    fn cleanup_expired_provisioning_grant(&mut self) {
        let expired = if let Some(active_session) = self.session.as_mut() {
            if let Some(grant) = active_session.provisioning_grant.as_ref() {
                if grant.has_expired(SystemTime::now()) {
                    let session_id = active_session.id;
                    let expires_at_epoch_ms = grant.expires_at_epoch_ms();
                    active_session.provisioning_grant = None;
                    Some((session_id, expires_at_epoch_ms))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some((session_id, expires_at_epoch_ms)) = expired {
            let _ = self.audit.record(
                AuditEntry::new(session_id, "account_provisioning_grant", "expired")
                    .with_operation(ACCOUNT_PROVISIONING_OPERATION)
                    .with_details(json!({
                        "expires_at_epoch_ms": expires_at_epoch_ms,
                        "allowed_tools": ACCOUNT_PROVISIONING_ALLOWED_TOOLS,
                    })),
            );
        }
    }

    fn cleanup_expired_audit_reporting_grant(&mut self) {
        let expired = if let Some(active_session) = self.session.as_mut() {
            if let Some(grant) = active_session.audit_reporting_grant.as_ref() {
                if grant.has_expired(SystemTime::now()) {
                    let session_id = active_session.id;
                    let expires_at_epoch_ms = grant.expires_at_epoch_ms();
                    active_session.audit_reporting_grant = None;
                    Some((session_id, expires_at_epoch_ms))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some((session_id, expires_at_epoch_ms)) = expired {
            let _ = self.audit.record(
                AuditEntry::new(session_id, "audit_reporting_grant", "expired")
                    .with_operation(AUDIT_REPORTING_OPERATION)
                    .with_details(json!({
                        "expires_at_epoch_ms": expires_at_epoch_ms,
                        "allowed_tools": AUDIT_REPORTING_ALLOWED_TOOLS,
                    })),
            );
        }
    }

    fn wait_for_prompt_stability(&self, operation: &str) {
        let Some(active_session) = self.session.as_ref() else {
            return;
        };

        let wait_for = active_session
            .prompt_quiet_until
            .saturating_duration_since(Instant::now());
        if wait_for.is_zero() {
            return;
        }

        diagnostics::log_event(
            "mcp",
            "wait_for_prompt_stability.sleep",
            json!({
                "operation": operation,
                "wait_ms": wait_for.as_millis(),
            }),
        );
        thread::sleep(wait_for);
    }
}

fn validate_requested_read_limit(requested_read_limit: u8) -> Result<u8, String> {
    if (1..=AUDIT_REPORTING_GRANT_MAX_READS).contains(&requested_read_limit) {
        return Ok(requested_read_limit);
    }

    Err(format!(
        "requested_read_limit debe estar entre 1 y {}.",
        AUDIT_REPORTING_GRANT_MAX_READS
    ))
}

fn normalize_audit_event_limit(limit: usize) -> usize {
    if limit == 0 {
        DEFAULT_AUDIT_EVENT_LIMIT.min(MAX_AUDIT_EVENT_LIMIT)
    } else {
        limit.clamp(1, MAX_AUDIT_EVENT_LIMIT)
    }
}

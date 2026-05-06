use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use uuid::Uuid;

pub const ACCOUNT_PROVISIONING_OPERATION: &str = "account_provisioning";
pub const ACCOUNT_PROVISIONING_GRANT_TTL: Duration = Duration::from_secs(600);
pub const ACCOUNT_PROVISIONING_GRANT_MAX_ACCOUNTS: u8 = 10;
pub const ACCOUNT_PROVISIONING_ALLOWED_TOOLS: &[&str] = &[
    "create_account",
    "import_otpauth",
    "update_account",
    "remove_account",
];
pub const AUDIT_REPORTING_OPERATION: &str = "audit_reporting";
pub const AUDIT_REPORTING_GRANT_TTL: Duration = Duration::from_secs(300);
pub const AUDIT_REPORTING_GRANT_MAX_READS: u8 = 10;
pub const AUDIT_REPORTING_ALLOWED_TOOLS: &[&str] = &[
    "list_history",
    "read_audit_events",
    "summarize_audit_events",
];
pub const GENERATE_TOKEN_OPERATION: &str = "generate_token";
pub const GENERATE_TOKEN_GRANT_TTL: Duration = Duration::from_secs(30);
pub const GENERATE_TOKEN_GRANT_MAX_USES: u8 = 1;

#[derive(Clone, Debug)]
pub struct AuditReportingGrant {
    remaining_reads: u8,
    expires_at: SystemTime,
}

#[derive(Clone, Debug)]
pub struct ProvisioningGrant {
    remaining_accounts: u8,
    expires_at: SystemTime,
}

#[derive(Clone, Debug)]
pub struct TokenGrant {
    account_id: Uuid,
    expires_at: SystemTime,
    remaining_uses: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvisioningGrantFailure {
    Missing,
    Expired,
    Exhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditReportingGrantFailure {
    Missing,
    Expired,
    Exhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenGrantFailure {
    Missing,
    Expired,
    WrongAccount,
    Exhausted,
}

impl AuditReportingGrant {
    pub fn new(read_limit: u8) -> Self {
        Self {
            remaining_reads: read_limit,
            expires_at: SystemTime::now() + AUDIT_REPORTING_GRANT_TTL,
        }
    }

    pub fn remaining_reads(&self) -> u8 {
        self.remaining_reads
    }

    pub fn expires_at_epoch_ms(&self) -> u64 {
        epoch_ms(self.expires_at)
    }

    pub fn has_expired(&self, now: SystemTime) -> bool {
        now >= self.expires_at
    }

    pub fn verify(&self, now: SystemTime) -> Result<(), AuditReportingGrantFailure> {
        if self.has_expired(now) {
            return Err(AuditReportingGrantFailure::Expired);
        }

        if self.remaining_reads == 0 {
            return Err(AuditReportingGrantFailure::Exhausted);
        }

        Ok(())
    }

    pub fn consume_one(&mut self) -> Result<(), AuditReportingGrantFailure> {
        if self.remaining_reads == 0 {
            return Err(AuditReportingGrantFailure::Exhausted);
        }

        self.remaining_reads -= 1;
        Ok(())
    }

    pub fn snapshot_value(&self, now: SystemTime) -> Value {
        json!({
            "operation": AUDIT_REPORTING_OPERATION,
            "status": if self.has_expired(now) { "expired" } else { "active" },
            "expires_at_epoch_ms": self.expires_at_epoch_ms(),
            "remaining_reads": if self.has_expired(now) { 0 } else { self.remaining_reads },
            "allowed_tools": AUDIT_REPORTING_ALLOWED_TOOLS,
        })
    }
}

impl ProvisioningGrant {
    pub fn new(account_limit: u8) -> Self {
        Self {
            remaining_accounts: account_limit,
            expires_at: SystemTime::now() + ACCOUNT_PROVISIONING_GRANT_TTL,
        }
    }

    pub fn remaining_accounts(&self) -> u8 {
        self.remaining_accounts
    }

    pub fn expires_at_epoch_ms(&self) -> u64 {
        epoch_ms(self.expires_at)
    }

    pub fn has_expired(&self, now: SystemTime) -> bool {
        now >= self.expires_at
    }

    pub fn verify(&self, now: SystemTime) -> Result<(), ProvisioningGrantFailure> {
        if self.has_expired(now) {
            return Err(ProvisioningGrantFailure::Expired);
        }

        if self.remaining_accounts == 0 {
            return Err(ProvisioningGrantFailure::Exhausted);
        }

        Ok(())
    }

    pub fn consume_one(&mut self) -> Result<(), ProvisioningGrantFailure> {
        if self.remaining_accounts == 0 {
            return Err(ProvisioningGrantFailure::Exhausted);
        }

        self.remaining_accounts -= 1;
        Ok(())
    }

    pub fn snapshot_value(&self, now: SystemTime) -> Value {
        json!({
            "operation": ACCOUNT_PROVISIONING_OPERATION,
            "status": if self.has_expired(now) { "expired" } else { "active" },
            "expires_at_epoch_ms": self.expires_at_epoch_ms(),
            "remaining_accounts": if self.has_expired(now) { 0 } else { self.remaining_accounts },
            "allowed_tools": ACCOUNT_PROVISIONING_ALLOWED_TOOLS,
        })
    }
}

impl TokenGrant {
    pub fn new(account_id: Uuid) -> Self {
        Self {
            account_id,
            expires_at: SystemTime::now() + GENERATE_TOKEN_GRANT_TTL,
            remaining_uses: GENERATE_TOKEN_GRANT_MAX_USES,
        }
    }

    pub fn account_id(&self) -> Uuid {
        self.account_id
    }

    pub fn remaining_uses(&self) -> u8 {
        self.remaining_uses
    }

    pub fn expires_at_epoch_ms(&self) -> u64 {
        epoch_ms(self.expires_at)
    }

    pub fn has_expired(&self, now: SystemTime) -> bool {
        now >= self.expires_at
    }

    pub fn verify(&self, account_id: Uuid, now: SystemTime) -> Result<(), TokenGrantFailure> {
        if self.has_expired(now) {
            return Err(TokenGrantFailure::Expired);
        }

        if self.remaining_uses == 0 {
            return Err(TokenGrantFailure::Exhausted);
        }

        if self.account_id != account_id {
            return Err(TokenGrantFailure::WrongAccount);
        }

        Ok(())
    }

    pub fn consume_one(&mut self) -> Result<(), TokenGrantFailure> {
        if self.remaining_uses == 0 {
            return Err(TokenGrantFailure::Exhausted);
        }

        self.remaining_uses -= 1;
        Ok(())
    }

    pub fn snapshot_value(&self, now: SystemTime) -> Value {
        json!({
            "operation": GENERATE_TOKEN_OPERATION,
            "status": if self.has_expired(now) { "expired" } else { "active" },
            "account_id": self.account_id,
            "expires_at_epoch_ms": self.expires_at_epoch_ms(),
            "remaining_uses": if self.has_expired(now) { 0 } else { self.remaining_uses },
        })
    }
}

impl AuditReportingGrantFailure {
    pub fn audit_result(self) -> &'static str {
        match self {
            Self::Missing => "denied_missing_audit_reporting_grant",
            Self::Expired => "denied_expired_audit_reporting_grant",
            Self::Exhausted => "denied_exhausted_audit_reporting_grant",
        }
    }

    pub fn user_message(self) -> &'static str {
        match self {
            Self::Missing | Self::Exhausted => {
                "Las operaciones list_history, read_audit_events y summarize_audit_events requieren un grant explícito. Llama primero a grant_audit_reporting."
            }
            Self::Expired => {
                "El grant de reporting sensible ya expiró. Solicita uno nuevo con grant_audit_reporting."
            }
        }
    }
}

impl ProvisioningGrantFailure {
    pub fn audit_result(self) -> &'static str {
        match self {
            Self::Missing => "denied_missing_provisioning_grant",
            Self::Expired => "denied_expired_provisioning_grant",
            Self::Exhausted => "denied_exhausted_provisioning_grant",
        }
    }

    pub fn user_message(self) -> &'static str {
        match self {
            Self::Missing | Self::Exhausted => {
                "Las operaciones create_account, import_otpauth, update_account y remove_account requieren un provisioning grant explícito. Llama primero a grant_account_provisioning."
            }
            Self::Expired => {
                "El provisioning grant ya expiró. Solicita uno nuevo con grant_account_provisioning."
            }
        }
    }
}

impl TokenGrantFailure {
    pub fn audit_result(self) -> &'static str {
        match self {
            Self::Missing => "denied_missing_grant",
            Self::Expired => "denied_expired_grant",
            Self::WrongAccount => "denied_account_mismatch",
            Self::Exhausted => "denied_exhausted_grant",
        }
    }

    pub fn user_message(self) -> &'static str {
        match self {
            Self::Missing | Self::Exhausted => {
                "La operación generate_token requiere un grant explícito. Llama primero a grant_generate_token para esa cuenta."
            }
            Self::Expired => {
                "El grant explícito para generate_token ya expiró. Solicita uno nuevo con grant_generate_token."
            }
            Self::WrongAccount => {
                "El grant activo de generate_token cubre otra cuenta. Solicita grant_generate_token para la cuenta pedida."
            }
        }
    }
}

pub fn empty_provisioning_grant_snapshot_value() -> Value {
    json!({
        "operation": ACCOUNT_PROVISIONING_OPERATION,
        "status": "none",
        "expires_at_epoch_ms": Value::Null,
        "remaining_accounts": 0,
        "allowed_tools": ACCOUNT_PROVISIONING_ALLOWED_TOOLS,
    })
}

pub fn empty_audit_reporting_grant_snapshot_value() -> Value {
    json!({
        "operation": AUDIT_REPORTING_OPERATION,
        "status": "none",
        "expires_at_epoch_ms": Value::Null,
        "remaining_reads": 0,
        "allowed_tools": AUDIT_REPORTING_ALLOWED_TOOLS,
    })
}

pub fn empty_grant_snapshot_value() -> Value {
    json!({
        "operation": GENERATE_TOKEN_OPERATION,
        "status": "none",
        "account_id": Value::Null,
        "expires_at_epoch_ms": Value::Null,
        "remaining_uses": 0,
    })
}

fn epoch_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

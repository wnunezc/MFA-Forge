use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use mfa_forge_core::TotpConfig;

pub const PROTOCOL_VERSION: &str = "mfa-forge-agent/v1";

#[derive(Debug, Deserialize)]
pub struct AgentRequest {
    #[serde(default = "default_request_id")]
    pub id: Value,
    #[serde(flatten)]
    pub command: AgentCommand,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum AgentCommand {
    Ping,
    SessionInfo,
    ListAccounts,
    History,
    GenerateToken {
        account_id: Uuid,
    },
    AddAccount {
        service: String,
        user: String,
        secret: String,
        #[serde(default)]
        totp: Option<TotpConfig>,
    },
    ImportOtpauth {
        uri: String,
    },
    UpdateAccount {
        account_id: Uuid,
        #[serde(default)]
        service: Option<String>,
        #[serde(default)]
        user: Option<String>,
        #[serde(default)]
        secret: Option<String>,
        #[serde(default)]
        totp: Option<TotpConfig>,
    },
    RemoveAccount {
        account_id: Uuid,
    },
    ExportMetadata,
    RotateMasterPassword,
    CloseSession,
}

#[derive(Debug, Serialize)]
pub struct AgentSuccessResponse<T> {
    pub id: Value,
    pub ok: bool,
    pub result: T,
}

#[derive(Debug, Serialize)]
pub struct AgentErrorResponse {
    pub id: Value,
    pub ok: bool,
    pub error: String,
}

fn default_request_id() -> Value {
    Value::Null
}

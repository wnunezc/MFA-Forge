use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use mfa_forge_core::TotpConfig;

use super::super::grant::{
    ACCOUNT_PROVISIONING_GRANT_MAX_ACCOUNTS, AUDIT_REPORTING_GRANT_MAX_READS,
};
use super::super::wire;
use super::transport::JsonRpcError;

const DEFAULT_AUDIT_EVENT_LIMIT: usize = 25;

#[derive(Debug, Deserialize)]
pub(super) struct AccountIdArgs {
    pub(super) account_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub(super) struct GenerateTokenArgs {
    pub(super) account_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub(super) struct GrantAccountProvisioningArgs {
    #[serde(default = "default_requested_account_limit")]
    pub(super) requested_account_limit: u8,
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateAccountArgs {
    pub(super) service: String,
    pub(super) user: String,
    pub(super) secret: String,
    #[serde(default)]
    pub(super) totp: Option<TotpConfig>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ImportOtpauthArgs {
    pub(super) uri: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct GrantAuditReportingArgs {
    #[serde(default = "default_requested_read_limit")]
    pub(super) requested_read_limit: u8,
}

#[derive(Debug, Deserialize)]
pub(super) struct AuditEventArgs {
    #[serde(default = "default_audit_event_limit")]
    pub(super) limit: usize,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateAccountArgs {
    pub(super) account_id: Uuid,
    #[serde(default)]
    pub(super) service: Option<String>,
    #[serde(default)]
    pub(super) user: Option<String>,
    #[serde(default)]
    pub(super) secret: Option<String>,
    #[serde(default)]
    pub(super) totp: Option<TotpConfig>,
}

#[derive(Debug, Serialize)]
pub(super) struct ToolListResult {
    pub(super) tools: Vec<McpTool>,
}

#[derive(Debug, Serialize)]
pub(super) struct McpTool {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
    #[serde(rename = "outputSchema")]
    output_schema: Value,
    annotations: ToolAnnotations,
}

#[derive(Debug, Serialize)]
pub(super) struct ToolAnnotations {
    #[serde(rename = "readOnlyHint")]
    read_only_hint: bool,
    #[serde(rename = "destructiveHint", skip_serializing_if = "Option::is_none")]
    destructive_hint: Option<bool>,
    #[serde(rename = "idempotentHint", skip_serializing_if = "Option::is_none")]
    idempotent_hint: Option<bool>,
    #[serde(rename = "openWorldHint")]
    open_world_hint: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct CallToolResult {
    pub(super) content: Vec<TextContent>,
    #[serde(rename = "structuredContent", skip_serializing_if = "Option::is_none")]
    pub(super) structured_content: Option<Value>,
    #[serde(rename = "isError")]
    pub(super) is_error: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct TextContent {
    #[serde(rename = "type")]
    pub(super) kind: &'static str,
    pub(super) text: String,
}

fn default_requested_account_limit() -> u8 {
    ACCOUNT_PROVISIONING_GRANT_MAX_ACCOUNTS
}

fn default_requested_read_limit() -> u8 {
    AUDIT_REPORTING_GRANT_MAX_READS
}

fn default_audit_event_limit() -> usize {
    DEFAULT_AUDIT_EVENT_LIMIT
}

pub(super) fn parse_arguments<T>(arguments: Value) -> Result<T, JsonRpcError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(arguments).map_err(|error| {
        JsonRpcError::invalid_params(
            format!("Los argumentos de la tool no cumplen el esquema esperado: {error}"),
            None,
        )
    })
}

pub(super) fn validate_requested_account_limit(requested_account_limit: u8) -> Result<u8, String> {
    if (1..=ACCOUNT_PROVISIONING_GRANT_MAX_ACCOUNTS).contains(&requested_account_limit) {
        return Ok(requested_account_limit);
    }

    Err(format!(
        "requested_account_limit debe estar entre 1 y {}.",
        ACCOUNT_PROVISIONING_GRANT_MAX_ACCOUNTS
    ))
}

pub(super) fn tool_success(structured: Value) -> CallToolResult {
    CallToolResult {
        content: vec![TextContent {
            kind: "text",
            text: serialize_pretty(&structured),
        }],
        structured_content: Some(structured),
        is_error: false,
    }
}

pub(super) fn tool_error(message: String) -> CallToolResult {
    let structured = json!({ "error": message });
    CallToolResult {
        content: vec![TextContent {
            kind: "text",
            text: serialize_pretty(&structured),
        }],
        structured_content: Some(structured),
        is_error: true,
    }
}

pub(super) fn tool_unreachable_error(message: String) -> JsonRpcError {
    JsonRpcError::invalid_request(message)
}

fn serialize_pretty(value: &Value) -> String {
    wire::to_ascii_safe_json_pretty(value).unwrap_or_else(|_| value.to_string())
}

pub(super) fn supported_tools() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "health",
            title: "Health",
            description: "Devuelve el estado básico del servidor MCP y del vault local.",
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string" },
                    "server": { "type": "string" },
                    "version": { "type": "string" },
                    "protocol_versions": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "vault_initialized": { "type": "boolean" },
                    "session_open": { "type": "boolean" },
                    "audit_log_path": { "type": "string" },
                    "generate_token_grant_required": { "type": "boolean" },
                    "account_provisioning_grant_required": { "type": "boolean" },
                    "audit_reporting_grant_required": { "type": "boolean" }
                },
                "required": ["status", "server", "version", "protocol_versions", "vault_initialized", "session_open", "audit_log_path", "generate_token_grant_required", "account_provisioning_grant_required", "audit_reporting_grant_required"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: None,
                idempotent_hint: Some(true),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "open_session",
            title: "Open Session",
            description: "Abre la ventana nativa de unlock y mantiene una sesión local viva para este proceso MCP.",
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string" },
                    "vault_path": { "type": "string" },
                    "audit_log_path": { "type": "string" },
                    "account_count": { "type": "integer" },
                    "windows_reinforced_unlock": { "type": "string" },
                    "account_provisioning_policy": { "type": "object" },
                    "generate_token_policy": { "type": "object" },
                    "audit_reporting_policy": { "type": "object" },
                    "message": { "type": "string" }
                },
                "required": ["status", "vault_path", "audit_log_path", "account_count", "windows_reinforced_unlock", "account_provisioning_policy", "generate_token_policy", "audit_reporting_policy", "message"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: false,
                destructive_hint: Some(false),
                idempotent_hint: Some(false),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "session_info",
            title: "Session Info",
            description: "Resume si la sesión MCP está bloqueada o abierta y cuántas cuentas tiene visibles.",
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string" },
                    "vault_path": { "type": "string" },
                    "audit_log_path": { "type": "string" },
                    "vault_initialized": { "type": "boolean" },
                    "session_open": { "type": "boolean" },
                    "account_count": { "type": "integer" },
                    "windows_reinforced_unlock": { "type": "string" },
                    "account_provisioning_policy": { "type": "object" },
                    "generate_token_policy": { "type": "object" },
                    "audit_reporting_policy": { "type": "object" }
                },
                "required": ["status", "vault_path", "audit_log_path", "vault_initialized", "session_open", "account_count", "windows_reinforced_unlock", "account_provisioning_policy", "generate_token_policy", "audit_reporting_policy"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: None,
                idempotent_hint: Some(true),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "list_accounts",
            title: "List Accounts",
            description: "Lista la metadata pública de las cuentas MFA disponibles en la sesión actual.",
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "accounts": {
                        "type": "array",
                        "items": { "type": "object" }
                    }
                },
                "required": ["accounts"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: None,
                idempotent_hint: Some(true),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "get_account_metadata",
            title: "Get Account Metadata",
            description: "Devuelve la metadata pública de una cuenta específica ya visible en la sesión actual.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "UUID de la cuenta devuelto por list_accounts."
                    }
                },
                "required": ["account_id"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "account": { "type": "object" }
                },
                "required": ["account"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: None,
                idempotent_hint: Some(true),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "grant_account_provisioning",
            title: "Grant Account Provisioning",
            description: "Solicita una aprobación local temporal para crear, importar, actualizar o eliminar hasta 10 cuentas MFA por MCP sin intervención adicional por cada operación.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "requested_account_limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": ACCOUNT_PROVISIONING_GRANT_MAX_ACCOUNTS,
                        "description": "Cantidad máxima de cuentas nuevas a permitir en este grant temporal. Si se omite, usa 10."
                    }
                }
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string" },
                    "message": { "type": "string" },
                    "grant": {
                        "type": ["object", "null"]
                    }
                },
                "required": ["status", "message", "grant"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: false,
                destructive_hint: Some(false),
                idempotent_hint: Some(false),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "grant_audit_reporting",
            title: "Grant Audit Reporting",
            description: "Solicita una aprobación local temporal para revisar historial público del vault y eventos recientes del audit log sin intervención adicional por cada lectura.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "requested_read_limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": AUDIT_REPORTING_GRANT_MAX_READS,
                        "description": "Cantidad máxima de lecturas sensibles permitidas por este grant temporal. Si se omite, usa 10."
                    }
                }
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string" },
                    "message": { "type": "string" },
                    "grant": {
                        "type": ["object", "null"]
                    }
                },
                "required": ["status", "message", "grant"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: false,
                destructive_hint: Some(false),
                idempotent_hint: Some(false),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "create_account",
            title: "Create Account",
            description: "Crea una cuenta TOTP nueva en el vault actual. Requiere un provisioning grant activo; el secreto solo entra como input y no se devuelve ni se registra en logs.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "service": { "type": "string" },
                    "user": { "type": "string" },
                    "secret": {
                        "type": "string",
                        "description": "Secreto Base32 de la cuenta. Solo se usa para persistirla cifrada; no se devuelve en respuestas ni en auditoría."
                    },
                    "totp": {
                        "type": "object",
                        "properties": {
                            "algorithm": { "type": "string" },
                            "digits": { "type": "integer" },
                            "period_seconds": { "type": "integer" }
                        }
                    }
                },
                "required": ["service", "user", "secret"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "account": { "type": "object" }
                },
                "required": ["account"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: false,
                destructive_hint: Some(false),
                idempotent_hint: Some(false),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "import_otpauth",
            title: "Import Otpauth",
            description: "Importa una cuenta desde un URI otpauth:// en el vault actual. Requiere un provisioning grant activo; el URI nunca se escribe en el audit log.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "uri": {
                        "type": "string",
                        "description": "URI otpauth:// completo a importar."
                    }
                },
                "required": ["uri"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "account": { "type": "object" }
                },
                "required": ["account"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: false,
                destructive_hint: Some(false),
                idempotent_hint: Some(false),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "update_account",
            title: "Update Account",
            description: "Actualiza la metadata pública o el secreto de una cuenta existente. Requiere un provisioning grant activo.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "UUID de la cuenta a modificar."
                    },
                    "service": { "type": "string" },
                    "user": { "type": "string" },
                    "secret": {
                        "type": "string",
                        "description": "Nuevo secreto Base32 opcional. Si se omite, se mantiene el actual."
                    },
                    "totp": {
                        "type": "object",
                        "properties": {
                            "algorithm": { "type": "string" },
                            "digits": { "type": "integer" },
                            "period_seconds": { "type": "integer" }
                        }
                    }
                },
                "required": ["account_id"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "account": { "type": "object" }
                },
                "required": ["account"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: false,
                destructive_hint: Some(false),
                idempotent_hint: Some(false),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "remove_account",
            title: "Remove Account",
            description: "Elimina una cuenta existente del vault actual. Requiere un provisioning grant activo.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "UUID de la cuenta a eliminar."
                    }
                },
                "required": ["account_id"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "account": { "type": "object" }
                },
                "required": ["account"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: false,
                destructive_hint: Some(true),
                idempotent_hint: Some(false),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "list_history",
            title: "List History",
            description: "Devuelve el historial público de cuentas del vault actual. Requiere un audit/reporting grant activo.",
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "entries": {
                        "type": "array",
                        "items": { "type": "object" }
                    }
                },
                "required": ["entries"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: None,
                idempotent_hint: Some(false),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "grant_generate_token",
            title: "Grant Generate Token",
            description: "Solicita una aprobación local explícita, por cuenta y de un solo uso antes de permitir generate_token.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "UUID de la cuenta devuelto por list_accounts."
                    }
                },
                "required": ["account_id"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string" },
                    "message": { "type": "string" },
                    "grant": {
                        "type": ["object", "null"]
                    }
                },
                "required": ["status", "message", "grant"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: false,
                destructive_hint: Some(false),
                idempotent_hint: Some(false),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "read_audit_events",
            title: "Read Audit Events",
            description: "Lee eventos recientes del audit log local saneado. Requiere un audit/reporting grant activo.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 200,
                        "description": "Cantidad máxima de eventos recientes a devolver. Si se omite, usa 25."
                    }
                }
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "events": {
                        "type": "array",
                        "items": { "type": "object" }
                    },
                    "limit": { "type": "integer" }
                },
                "required": ["events", "limit"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: None,
                idempotent_hint: Some(false),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "summarize_audit_events",
            title: "Summarize Audit Events",
            description: "Resume los eventos recientes del audit log por tipo y resultado. Requiere un audit/reporting grant activo.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 200,
                        "description": "Cantidad máxima de eventos recientes a considerar. Si se omite, usa 25."
                    }
                }
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "object" },
                    "limit": { "type": "integer" }
                },
                "required": ["summary", "limit"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: None,
                idempotent_hint: Some(false),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "generate_token",
            title: "Generate Token",
            description: "Genera el TOTP actual para una cuenta específica ya visible en la sesión, pero solo si existe un grant explícito vigente para esa cuenta.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "account_id": {
                        "type": "string",
                        "description": "UUID de la cuenta devuelto por list_accounts."
                    }
                },
                "required": ["account_id"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "token": { "type": "object" }
                },
                "required": ["token"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: None,
                idempotent_hint: Some(false),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "export_metadata",
            title: "Export Metadata",
            description: "Exporta la metadata pública visible de la sesión actual sin incluir secretos ni TOTP.",
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "accounts": {
                        "type": "array",
                        "items": { "type": "object" }
                    }
                },
                "required": ["accounts"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: true,
                destructive_hint: None,
                idempotent_hint: Some(true),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "rotate_master_password",
            title: "Rotate Master Password",
            description: "Solicita una aprobación nativa dedicada para re-cifrar el vault con una nueva contraseña maestra sin enviar esa contraseña por stdio/MCP.",
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string" },
                    "message": { "type": "string" }
                },
                "required": ["status", "message"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: false,
                destructive_hint: Some(false),
                idempotent_hint: Some(false),
                open_world_hint: false,
            },
        },
        McpTool {
            name: "close_session",
            title: "Close Session",
            description: "Cierra la sesión local abierta por MCP y borra el material sensible en memoria.",
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string" }
                },
                "required": ["status"]
            }),
            annotations: ToolAnnotations {
                read_only_hint: false,
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: false,
            },
        },
    ]
}

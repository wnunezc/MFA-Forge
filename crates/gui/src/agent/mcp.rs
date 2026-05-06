use std::io::{self, BufRead};

use serde_json::{Value, json};

mod session_host;
mod tools;
mod transport;

use session_host::{SERVER_NAME, SessionHost};
use tools::{
    AccountIdArgs, AuditEventArgs, CallToolResult, CreateAccountArgs, GenerateTokenArgs,
    GrantAccountProvisioningArgs, GrantAuditReportingArgs, ImportOtpauthArgs, ToolListResult,
    UpdateAccountArgs, parse_arguments, supported_tools, tool_error, tool_success,
    tool_unreachable_error,
};
use transport::{
    ImplementationInfo, IncomingMessage, InitializeParams, InitializeResult, JSON_RPC_VERSION,
    JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
    SUPPORTED_PROTOCOL_VERSIONS, ToolCallParams, parse_params, write_json,
};

pub fn run_mcp_server() -> Result<(), String> {
    crate::runtime::ensure_supported_runtime("El servidor local mfa-forge-mcp")?;

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = io::BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    let mut server = McpServer::bootstrap()?;
    let mut line = String::new();

    loop {
        line.clear();

        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|error| format!("No se pudo leer stdin: {error}"))?;

        if bytes_read == 0 {
            server.close_session();
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let incoming = match serde_json::from_str::<IncomingMessage>(trimmed) {
            Ok(message) => message,
            Err(error) => {
                write_json(
                    &mut writer,
                    &transport::JsonRpcErrorResponse::new(
                        Value::Null,
                        JsonRpcError::parse_error(format!(
                            "La solicitud MCP no es JSON válido: {error}"
                        )),
                    ),
                )?;
                continue;
            }
        };

        match incoming {
            IncomingMessage::Request(request) => {
                let response = server.handle_request(request);
                write_json(&mut writer, &response)?;
            }
            IncomingMessage::Notification(notification) => {
                server.handle_notification(notification)?;
            }
        }
    }

    Ok(())
}

struct McpServer {
    lifecycle: LifecycleState,
    host: SessionHost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifecycleState {
    WaitingInitialize,
    WaitingInitializedNotification,
    Ready,
}

impl McpServer {
    fn bootstrap() -> Result<Self, String> {
        Ok(Self {
            lifecycle: LifecycleState::WaitingInitialize,
            host: SessionHost::bootstrap()?,
        })
    }

    fn handle_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        crate::diagnostics::log_event(
            "mcp",
            "handle_request.received",
            json!({
                "method": &request.method,
                "id": &request.id,
            }),
        );
        if request.jsonrpc != JSON_RPC_VERSION {
            return JsonRpcResponse::error(
                request.id,
                JsonRpcError::invalid_request("Solo se soporta JSON-RPC 2.0.".to_owned()),
            );
        }

        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id, request.params),
            "ping" => JsonRpcResponse::success(request.id, json!({})),
            "tools/list" => {
                if let Err(error) = self.require_ready() {
                    return JsonRpcResponse::error(request.id, error);
                }

                JsonRpcResponse::success(
                    request.id,
                    ToolListResult {
                        tools: supported_tools(),
                    },
                )
            }
            "tools/call" => {
                if let Err(error) = self.require_ready() {
                    return JsonRpcResponse::error(request.id, error);
                }

                let params = match parse_params::<ToolCallParams>(request.params) {
                    Ok(params) => params,
                    Err(error) => return JsonRpcResponse::error(request.id, error),
                };

                match self.handle_tool_call(params) {
                    Ok(result) => JsonRpcResponse::success(request.id, result),
                    Err(error) => JsonRpcResponse::error(request.id, error),
                }
            }
            _ => JsonRpcResponse::error(
                request.id,
                JsonRpcError::method_not_found(format!(
                    "El método MCP '{}' no existe en MFA-Forge.",
                    request.method
                )),
            ),
        }
    }

    fn handle_initialize(&mut self, id: Value, params: Option<Value>) -> JsonRpcResponse {
        if self.lifecycle != LifecycleState::WaitingInitialize {
            return JsonRpcResponse::error(
                id,
                JsonRpcError::invalid_request(
                    "La conexión MCP ya fue inicializada para este proceso.".to_owned(),
                ),
            );
        }

        let params = match parse_params::<InitializeParams>(params) {
            Ok(params) => params,
            Err(error) => return JsonRpcResponse::error(id, error),
        };

        if !SUPPORTED_PROTOCOL_VERSIONS.contains(&params.protocol_version.as_str()) {
            return JsonRpcResponse::error(
                id,
                JsonRpcError::invalid_params(
                    "La versión de protocolo MCP solicitada no es compatible.".to_owned(),
                    Some(json!({
                        "requested": params.protocol_version,
                        "supported": SUPPORTED_PROTOCOL_VERSIONS,
                    })),
                ),
            );
        }

        self.lifecycle = LifecycleState::WaitingInitializedNotification;

        JsonRpcResponse::success(
            id,
            InitializeResult {
                protocol_version: params.protocol_version,
                capabilities: json!({
                    "tools": {
                        "listChanged": false
                    }
                }),
                server_info: ImplementationInfo {
                    name: SERVER_NAME,
                    version: env!("CARGO_PKG_VERSION"),
                },
                instructions: "Usa open_session para solicitar el unlock nativo antes de operar. list_accounts queda disponible tras abrir la sesión. generate_token exige grant_generate_token por cuenta; create_account/import_otpauth/update_account/remove_account exigen grant_account_provisioning con cuota temporal; list_history/read_audit_events/summarize_audit_events exigen grant_audit_reporting. rotate_master_password abre un prompt nativo dedicado y nunca recibe la nueva contraseña por stdio. La sesión es local, vive solo mientras este proceso siga activo y puede cerrarse con close_session.".to_owned(),
            },
        )
    }

    fn handle_notification(&mut self, notification: JsonRpcNotification) -> Result<(), String> {
        crate::diagnostics::log_event(
            "mcp",
            "handle_notification.received",
            json!({
                "method": &notification.method,
            }),
        );
        if notification.jsonrpc != JSON_RPC_VERSION {
            return Ok(());
        }

        match notification.method.as_str() {
            "notifications/initialized" => {
                if self.lifecycle == LifecycleState::WaitingInitializedNotification {
                    self.lifecycle = LifecycleState::Ready;
                }
            }
            "notifications/cancelled" => {}
            _ => {
                let _ = notification.params;
            }
        }

        Ok(())
    }

    fn handle_tool_call(&mut self, params: ToolCallParams) -> Result<CallToolResult, JsonRpcError> {
        let arguments = params.arguments.unwrap_or(Value::Null);

        match params.name.as_str() {
            "health" => Ok(tool_success(self.host.health_value())),
            "open_session" => self
                .host
                .open_session()
                .map(tool_success)
                .map_err(tool_unreachable_error),
            "session_info" => Ok(tool_success(self.host.session_info_value())),
            "list_accounts" => self
                .host
                .list_accounts_value()
                .map(tool_success)
                .or_else(|error| Ok(tool_error(error))),
            "get_account_metadata" => {
                let args = parse_arguments::<AccountIdArgs>(arguments)?;
                self.host
                    .get_account_metadata_value(args.account_id)
                    .map(tool_success)
                    .or_else(|error| Ok(tool_error(error)))
            }
            "grant_account_provisioning" => {
                let args = parse_arguments::<GrantAccountProvisioningArgs>(arguments)?;
                self.host
                    .grant_account_provisioning_value(args.requested_account_limit)
                    .map(tool_success)
                    .or_else(|error| Ok(tool_error(error)))
            }
            "grant_audit_reporting" => {
                let args = parse_arguments::<GrantAuditReportingArgs>(arguments)?;
                self.host
                    .grant_audit_reporting_value(args.requested_read_limit)
                    .map(tool_success)
                    .or_else(|error| Ok(tool_error(error)))
            }
            "create_account" => {
                let args = parse_arguments::<CreateAccountArgs>(arguments)?;
                self.host
                    .create_account_value(args)
                    .map(tool_success)
                    .or_else(|error| Ok(tool_error(error)))
            }
            "import_otpauth" => {
                let args = parse_arguments::<ImportOtpauthArgs>(arguments)?;
                self.host
                    .import_otpauth_value(args.uri)
                    .map(tool_success)
                    .or_else(|error| Ok(tool_error(error)))
            }
            "update_account" => {
                let args = parse_arguments::<UpdateAccountArgs>(arguments)?;
                self.host
                    .update_account_value(args)
                    .map(tool_success)
                    .or_else(|error| Ok(tool_error(error)))
            }
            "remove_account" => {
                let args = parse_arguments::<AccountIdArgs>(arguments)?;
                self.host
                    .remove_account_value(args.account_id)
                    .map(tool_success)
                    .or_else(|error| Ok(tool_error(error)))
            }
            "grant_generate_token" => {
                let args = parse_arguments::<GenerateTokenArgs>(arguments)?;
                self.host
                    .grant_generate_token_value(args.account_id)
                    .map(tool_success)
                    .or_else(|error| Ok(tool_error(error)))
            }
            "list_history" => self
                .host
                .list_history_value()
                .map(tool_success)
                .or_else(|error| Ok(tool_error(error))),
            "read_audit_events" => {
                let args = parse_arguments::<AuditEventArgs>(arguments)?;
                self.host
                    .read_audit_events_value(args.limit)
                    .map(tool_success)
                    .or_else(|error| Ok(tool_error(error)))
            }
            "summarize_audit_events" => {
                let args = parse_arguments::<AuditEventArgs>(arguments)?;
                self.host
                    .summarize_audit_events_value(args.limit)
                    .map(tool_success)
                    .or_else(|error| Ok(tool_error(error)))
            }
            "generate_token" => {
                let args = parse_arguments::<GenerateTokenArgs>(arguments)?;
                self.host
                    .generate_token_value(args.account_id)
                    .map(tool_success)
                    .or_else(|error| Ok(tool_error(error)))
            }
            "export_metadata" => self
                .host
                .export_metadata_value()
                .map(tool_success)
                .or_else(|error| Ok(tool_error(error))),
            "rotate_master_password" => self
                .host
                .rotate_master_password_value()
                .map(tool_success)
                .or_else(|error| Ok(tool_error(error))),
            "close_session" => Ok(tool_success(self.host.close_session_value())),
            _ => Err(JsonRpcError::invalid_params(
                "La tool solicitada no existe.".to_owned(),
                Some(json!({ "name": params.name })),
            )),
        }
    }

    fn require_ready(&self) -> Result<(), JsonRpcError> {
        if self.lifecycle == LifecycleState::Ready {
            return Ok(());
        }

        Err(JsonRpcError::invalid_request(
            "La conexión MCP aún no está lista. Envía initialize y luego notifications/initialized antes de operar.".to_owned(),
        ))
    }

    fn close_session(&mut self) {
        let _ = self.host.close_session_value();
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::Instant};

    use secrecy::SecretString;
    use tempfile::TempDir;
    use uuid::Uuid;

    use mfa_forge_core::{AccountPublic, TotpConfig};
    use mfa_forge_storage::VaultRepository;

    use crate::{
        agent::{
            audit::AuditLogger,
            unlock::{
                AuditReportingGrantPromptDecision, PasswordRotationPromptDecision,
                ProvisioningGrantPromptDecision, TokenGrantPromptDecision,
            },
        },
        vault::VaultFacade,
    };

    use super::*;

    struct UnlockedServerFixture {
        _temp_dir: TempDir,
        audit_path: PathBuf,
        server: McpServer,
    }

    fn approve_grant_prompt(
        _account: &AccountPublic,
        _ttl_seconds: u64,
    ) -> Result<TokenGrantPromptDecision, String> {
        Ok(TokenGrantPromptDecision::Approved)
    }

    fn deny_grant_prompt(
        _account: &AccountPublic,
        _ttl_seconds: u64,
    ) -> Result<TokenGrantPromptDecision, String> {
        Ok(TokenGrantPromptDecision::Denied)
    }

    fn approve_provisioning_grant_prompt(
        _account_limit: u8,
        _ttl_minutes: u64,
    ) -> Result<ProvisioningGrantPromptDecision, String> {
        Ok(ProvisioningGrantPromptDecision::Approved)
    }

    fn deny_provisioning_grant_prompt(
        _account_limit: u8,
        _ttl_minutes: u64,
    ) -> Result<ProvisioningGrantPromptDecision, String> {
        Ok(ProvisioningGrantPromptDecision::Denied)
    }

    fn approve_audit_reporting_grant_prompt(
        _read_limit: u8,
        _ttl_minutes: u64,
    ) -> Result<AuditReportingGrantPromptDecision, String> {
        Ok(AuditReportingGrantPromptDecision::Approved)
    }

    fn deny_audit_reporting_grant_prompt(
        _read_limit: u8,
        _ttl_minutes: u64,
    ) -> Result<AuditReportingGrantPromptDecision, String> {
        Ok(AuditReportingGrantPromptDecision::Denied)
    }

    fn approve_password_rotation_prompt() -> Result<PasswordRotationPromptDecision, String> {
        Ok(PasswordRotationPromptDecision::Approved {
            new_password: SecretString::from("new stronger password".to_owned()),
        })
    }

    fn deny_password_rotation_prompt() -> Result<PasswordRotationPromptDecision, String> {
        Ok(PasswordRotationPromptDecision::Denied)
    }

    fn unlocked_server() -> UnlockedServerFixture {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let repository = VaultRepository::new(temp_dir.path().join("vault.json"));
        let audit_path = temp_dir.path().join("mcp-audit.jsonl");
        let mut vault = VaultFacade::new(repository);
        let password = SecretString::from("correct horse battery staple".to_owned());

        vault
            .initialize_and_unlock(password)
            .expect("vault should initialize");

        vault
            .add_account(
                "GitHub".to_owned(),
                "user@example.com".to_owned(),
                SecretString::from("JBSWY3DPEHPK3PXP".to_owned()),
                TotpConfig::default(),
            )
            .expect("account should be added");

        UnlockedServerFixture {
            _temp_dir: temp_dir,
            audit_path: audit_path.clone(),
            server: McpServer {
                lifecycle: LifecycleState::Ready,
                host: SessionHost {
                    vault_path: vault.path_display().to_owned(),
                    audit_log_path: audit_path.display().to_string(),
                    vault_initialized: true,
                    session: Some(session_host::ActiveMcpSession {
                        id: Uuid::new_v4(),
                        session: crate::agent::session::AgentSession::new(vault),
                        token_grant: None,
                        provisioning_grant: None,
                        audit_reporting_grant: None,
                        prompt_quiet_until: Instant::now(),
                    }),
                    audit: AuditLogger::new(audit_path),
                    token_grant_prompt: approve_grant_prompt,
                    provisioning_grant_prompt: approve_provisioning_grant_prompt,
                    audit_reporting_grant_prompt: approve_audit_reporting_grant_prompt,
                    password_rotation_prompt: approve_password_rotation_prompt,
                },
            },
        }
    }

    fn first_account_id(fixture: &UnlockedServerFixture) -> Uuid {
        fixture
            .server
            .host
            .session
            .as_ref()
            .expect("session")
            .session
            .list_accounts()[0]
            .id
    }

    #[test]
    fn initialize_accepts_supported_protocol() {
        let mut server = McpServer::bootstrap().expect("server should bootstrap");

        let response = server.handle_request(JsonRpcRequest {
            jsonrpc: JSON_RPC_VERSION.to_owned(),
            id: json!(1),
            method: "initialize".to_owned(),
            params: Some(json!({
                "protocolVersion": "2025-06-18"
            })),
        });

        let value = serde_json::to_value(response).expect("response should serialize");
        assert_eq!(value["result"]["protocolVersion"], "2025-06-18");
    }

    #[test]
    fn list_accounts_requires_open_session() {
        let mut server = McpServer::bootstrap().expect("server should bootstrap");
        server.lifecycle = LifecycleState::Ready;

        let result = server
            .handle_tool_call(ToolCallParams {
                name: "list_accounts".to_owned(),
                arguments: None,
            })
            .expect("tool call should succeed with MCP-level result");

        assert!(result.is_error);
        assert_eq!(
            result.structured_content.expect("structured content")["error"],
            "La sesión MCP está bloqueada. Llama primero a open_session para solicitar el unlock nativo."
        );
    }

    #[test]
    fn generate_token_requires_explicit_grant() {
        let mut fixture = unlocked_server();
        let account_id = first_account_id(&fixture);

        let result = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "generate_token".to_owned(),
                arguments: Some(json!({ "account_id": account_id })),
            })
            .expect("tool call should succeed");

        assert!(result.is_error);
        assert_eq!(
            result.structured_content.expect("structured content")["error"],
            "La operación generate_token requiere un grant explícito. Llama primero a grant_generate_token para esa cuenta."
        );
    }

    #[test]
    fn create_account_requires_explicit_provisioning_grant() {
        let mut fixture = unlocked_server();

        let result = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "create_account".to_owned(),
                arguments: Some(json!({
                    "service": "RC8 Test",
                    "user": "qa@example.com",
                    "secret": "JBSWY3DPEHPK3PXP"
                })),
            })
            .expect("tool call should succeed");

        assert!(result.is_error);
        assert_eq!(
            result.structured_content.expect("structured content")["error"],
            "Las operaciones create_account, import_otpauth, update_account y remove_account requieren un provisioning grant explícito. Llama primero a grant_account_provisioning."
        );
    }

    #[test]
    fn get_account_metadata_returns_a_single_visible_account() {
        let mut fixture = unlocked_server();
        let account_id = first_account_id(&fixture);

        let result = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "get_account_metadata".to_owned(),
                arguments: Some(json!({ "account_id": account_id })),
            })
            .expect("tool call should succeed");

        assert!(!result.is_error);
        assert_eq!(
            result.structured_content.expect("structured content")["account"]["id"],
            json!(account_id)
        );
    }

    #[test]
    fn grant_generate_token_is_single_use_and_consumed_after_delivery() {
        let mut fixture = unlocked_server();
        let account_id = first_account_id(&fixture);

        let grant = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "grant_generate_token".to_owned(),
                arguments: Some(json!({ "account_id": account_id })),
            })
            .expect("grant tool should succeed");

        assert!(!grant.is_error);
        assert_eq!(
            grant.structured_content.expect("structured content")["status"],
            "granted"
        );

        let token = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "generate_token".to_owned(),
                arguments: Some(json!({ "account_id": account_id })),
            })
            .expect("generate_token should succeed");

        assert!(!token.is_error);
        assert_eq!(
            token.structured_content.expect("structured content")["token"]["service"],
            "GitHub"
        );

        let second = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "generate_token".to_owned(),
                arguments: Some(json!({ "account_id": account_id })),
            })
            .expect("second generate_token should return MCP-level result");

        assert!(second.is_error);
        assert_eq!(
            second.structured_content.expect("structured content")["error"],
            "La operación generate_token requiere un grant explícito. Llama primero a grant_generate_token para esa cuenta."
        );
    }

    #[test]
    fn denied_grant_returns_structured_denial_without_activating_policy() {
        let mut fixture = unlocked_server();
        fixture.server.host.token_grant_prompt = deny_grant_prompt;
        let account_id = first_account_id(&fixture);

        let grant = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "grant_generate_token".to_owned(),
                arguments: Some(json!({ "account_id": account_id })),
            })
            .expect("grant tool should succeed");

        assert!(!grant.is_error);
        assert_eq!(
            grant.structured_content.expect("structured content")["status"],
            "denied"
        );

        let info = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "session_info".to_owned(),
                arguments: None,
            })
            .expect("session_info should succeed");

        assert_eq!(
            info.structured_content.expect("structured content")["generate_token_policy"]["active_grant"]
                ["status"],
            "none"
        );
    }

    #[test]
    fn denied_provisioning_grant_returns_structured_denial_without_activating_policy() {
        let mut fixture = unlocked_server();
        fixture.server.host.provisioning_grant_prompt = deny_provisioning_grant_prompt;

        let grant = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "grant_account_provisioning".to_owned(),
                arguments: Some(json!({ "requested_account_limit": 2 })),
            })
            .expect("grant tool should succeed");

        assert!(!grant.is_error);
        assert_eq!(
            grant.structured_content.expect("structured content")["status"],
            "denied"
        );

        let info = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "session_info".to_owned(),
                arguments: None,
            })
            .expect("session_info should succeed");

        assert_eq!(
            info.structured_content.expect("structured content")["account_provisioning_policy"]["active_grant"]
                ["status"],
            "none"
        );
    }

    #[test]
    fn denied_audit_reporting_grant_returns_structured_denial_without_activating_policy() {
        let mut fixture = unlocked_server();
        fixture.server.host.audit_reporting_grant_prompt = deny_audit_reporting_grant_prompt;

        let grant = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "grant_audit_reporting".to_owned(),
                arguments: Some(json!({ "requested_read_limit": 2 })),
            })
            .expect("grant tool should succeed");

        assert!(!grant.is_error);
        assert_eq!(
            grant.structured_content.expect("structured content")["status"],
            "denied"
        );

        let info = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "session_info".to_owned(),
                arguments: None,
            })
            .expect("session_info should succeed");

        assert_eq!(
            info.structured_content.expect("structured content")["audit_reporting_policy"]["active_grant"]
                ["status"],
            "none"
        );
    }

    #[test]
    fn provisioning_grant_is_consumed_by_create_account_and_import_otpauth() {
        let mut fixture = unlocked_server();

        let grant = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "grant_account_provisioning".to_owned(),
                arguments: Some(json!({ "requested_account_limit": 2 })),
            })
            .expect("provisioning grant should succeed");

        assert!(!grant.is_error);
        assert_eq!(
            grant.structured_content.expect("structured content")["grant"]["remaining_accounts"],
            2
        );

        let created = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "create_account".to_owned(),
                arguments: Some(json!({
                    "service": "RC8 Test Create",
                    "user": "qa1@example.com",
                    "secret": "KRSXG5DSNFXGOIDB"
                })),
            })
            .expect("create_account should succeed");

        assert!(!created.is_error);
        assert_eq!(
            created.structured_content.expect("structured content")["account"]["service"],
            "RC8 Test Create"
        );

        let imported = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "import_otpauth".to_owned(),
                arguments: Some(json!({
                    "uri": "otpauth://totp/RC8%20Test%20Import:qa2%40example.com?secret=JBSWY3DPEHPK3PXP&issuer=RC8%20Test%20Import"
                })),
            })
            .expect("import_otpauth should succeed");

        assert!(!imported.is_error);
        assert_eq!(
            imported.structured_content.expect("structured content")["account"]["service"],
            "RC8 Test Import"
        );

        let third = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "create_account".to_owned(),
                arguments: Some(json!({
                    "service": "RC8 Test Exhausted",
                    "user": "qa3@example.com",
                    "secret": "JBSWY3DPEHPK3PXP"
                })),
            })
            .expect("third create_account should return MCP-level result");

        assert!(third.is_error);
        assert_eq!(
            third.structured_content.expect("structured content")["error"],
            "Las operaciones create_account, import_otpauth, update_account y remove_account requieren un provisioning grant explícito. Llama primero a grant_account_provisioning."
        );
    }

    #[test]
    fn update_and_remove_account_require_and_consume_provisioning_grant() {
        let mut fixture = unlocked_server();
        let account_id = first_account_id(&fixture);

        fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "grant_account_provisioning".to_owned(),
                arguments: Some(json!({ "requested_account_limit": 2 })),
            })
            .expect("grant tool should succeed");

        let updated = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "update_account".to_owned(),
                arguments: Some(json!({
                    "account_id": account_id,
                    "service": "GitHub Updated",
                    "user": "updated@example.com"
                })),
            })
            .expect("update_account should succeed");

        assert!(!updated.is_error);
        let updated_structured = updated.structured_content.expect("structured content");
        assert_eq!(updated_structured["account"]["service"], "GitHub Updated");
        assert_eq!(updated_structured["account"]["user"], "updated@example.com");

        let removed = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "remove_account".to_owned(),
                arguments: Some(json!({ "account_id": account_id })),
            })
            .expect("remove_account should succeed");

        assert!(!removed.is_error);
        assert_eq!(
            removed.structured_content.expect("structured content")["account"]["id"],
            json!(account_id)
        );

        let exhausted = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "remove_account".to_owned(),
                arguments: Some(json!({ "account_id": account_id })),
            })
            .expect("third tool call should return MCP-level result");

        assert!(exhausted.is_error);
        assert_eq!(
            exhausted.structured_content.expect("structured content")["error"],
            "Las operaciones create_account, import_otpauth, update_account y remove_account requieren un provisioning grant explícito. Llama primero a grant_account_provisioning."
        );
    }

    #[test]
    fn export_metadata_returns_public_accounts_only() {
        let mut fixture = unlocked_server();

        let exported = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "export_metadata".to_owned(),
                arguments: None,
            })
            .expect("export_metadata should succeed");

        assert!(!exported.is_error);
        let structured = exported.structured_content.expect("structured content");
        let first = structured["accounts"][0].clone();
        assert_eq!(first["service"], "GitHub");
        assert!(first.get("secret").is_none());
    }

    #[test]
    fn list_history_requires_audit_reporting_grant() {
        let mut fixture = unlocked_server();

        let result = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "list_history".to_owned(),
                arguments: None,
            })
            .expect("tool call should succeed");

        assert!(result.is_error);
        assert_eq!(
            result.structured_content.expect("structured content")["error"],
            "Las operaciones list_history, read_audit_events y summarize_audit_events requieren un grant explícito. Llama primero a grant_audit_reporting."
        );
    }

    #[test]
    fn audit_reporting_grant_allows_history_and_summary_reads() {
        let mut fixture = unlocked_server();
        let account_id = first_account_id(&fixture);

        fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "grant_account_provisioning".to_owned(),
                arguments: Some(json!({ "requested_account_limit": 1 })),
            })
            .expect("provisioning grant should succeed");
        fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "update_account".to_owned(),
                arguments: Some(json!({
                    "account_id": account_id,
                    "service": "GitHub History",
                })),
            })
            .expect("update_account should succeed");

        let grant = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "grant_audit_reporting".to_owned(),
                arguments: Some(json!({ "requested_read_limit": 2 })),
            })
            .expect("audit grant should succeed");

        assert!(!grant.is_error);
        assert_eq!(
            grant.structured_content.expect("structured content")["grant"]["remaining_reads"],
            2
        );

        let history = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "list_history".to_owned(),
                arguments: None,
            })
            .expect("list_history should succeed");

        assert!(!history.is_error);
        assert_eq!(
            history.structured_content.expect("structured content")["entries"][0]["event"],
            "updated"
        );

        let summary = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "summarize_audit_events".to_owned(),
                arguments: Some(json!({ "limit": 10 })),
            })
            .expect("summary should succeed");

        assert!(!summary.is_error);
        assert_eq!(
            summary.structured_content.expect("structured content")["summary"]["counts_by_event"]["update_account"],
            1
        );

        let exhausted = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "read_audit_events".to_owned(),
                arguments: Some(json!({ "limit": 10 })),
            })
            .expect("read_audit_events should return MCP-level result");

        assert!(exhausted.is_error);
        assert_eq!(
            exhausted.structured_content.expect("structured content")["error"],
            "Las operaciones list_history, read_audit_events y summarize_audit_events requieren un grant explícito. Llama primero a grant_audit_reporting."
        );
    }

    #[test]
    fn read_audit_events_returns_recent_entries_without_secret_fields() {
        let mut fixture = unlocked_server();
        let account_id = first_account_id(&fixture);

        fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "grant_generate_token".to_owned(),
                arguments: Some(json!({ "account_id": account_id })),
            })
            .expect("grant should succeed");
        fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "generate_token".to_owned(),
                arguments: Some(json!({ "account_id": account_id })),
            })
            .expect("token should succeed");
        fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "grant_audit_reporting".to_owned(),
                arguments: Some(json!({ "requested_read_limit": 1 })),
            })
            .expect("audit grant should succeed");

        let audit = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "read_audit_events".to_owned(),
                arguments: Some(json!({ "limit": 10 })),
            })
            .expect("audit read should succeed");

        assert!(!audit.is_error);
        let structured = audit.structured_content.expect("structured content");
        let events = structured["events"]
            .as_array()
            .expect("events should be an array");
        assert!(
            events
                .iter()
                .any(|event| event["event"] == "generate_token" && event["result"] == "delivered")
        );
        assert!(events.iter().all(|event| event.get("token").is_none()));
        assert!(events.iter().all(|event| event.get("secret").is_none()));
    }

    #[test]
    fn rotate_master_password_uses_dedicated_prompt_and_reencrypts_vault() {
        let mut fixture = unlocked_server();

        let rotated = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "rotate_master_password".to_owned(),
                arguments: None,
            })
            .expect("rotation should succeed");

        assert!(!rotated.is_error);
        assert_eq!(
            rotated.structured_content.expect("structured content")["status"],
            "rotated"
        );

        let token = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "list_accounts".to_owned(),
                arguments: None,
            })
            .expect("session should remain unlocked");
        assert!(!token.is_error);
    }

    #[test]
    fn denied_password_rotation_returns_structured_denial() {
        let mut fixture = unlocked_server();
        fixture.server.host.password_rotation_prompt = deny_password_rotation_prompt;

        let denied = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "rotate_master_password".to_owned(),
                arguments: None,
            })
            .expect("rotation should succeed");

        assert!(!denied.is_error);
        assert_eq!(
            denied.structured_content.expect("structured content")["status"],
            "denied"
        );
    }

    #[test]
    fn successful_grant_and_token_append_audit_entries_without_token_values() {
        let mut fixture = unlocked_server();
        let account_id = first_account_id(&fixture);

        fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "grant_generate_token".to_owned(),
                arguments: Some(json!({ "account_id": account_id })),
            })
            .expect("grant tool should succeed");
        fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "generate_token".to_owned(),
                arguments: Some(json!({ "account_id": account_id })),
            })
            .expect("generate_token should succeed");

        let contents = fs::read_to_string(&fixture.audit_path).expect("audit file should exist");
        let lines = contents.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);

        let grant_entry: Value = serde_json::from_str(lines[0]).expect("grant entry should parse");
        let token_entry: Value = serde_json::from_str(lines[1]).expect("token entry should parse");

        assert_eq!(grant_entry["event"], "generate_token_grant");
        assert_eq!(grant_entry["result"], "granted");
        assert_eq!(token_entry["event"], "generate_token");
        assert_eq!(token_entry["result"], "delivered");
        assert!(grant_entry.get("token").is_none());
        assert!(token_entry.get("token").is_none());
    }

    #[test]
    fn successful_provisioning_grant_and_create_account_append_audit_entries_without_secret_fields()
    {
        let mut fixture = unlocked_server();

        fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "grant_account_provisioning".to_owned(),
                arguments: Some(json!({ "requested_account_limit": 1 })),
            })
            .expect("grant tool should succeed");
        fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "create_account".to_owned(),
                arguments: Some(json!({
                    "service": "RC8 Audit Create",
                    "user": "audit@example.com",
                    "secret": "JBSWY3DPEHPK3PXP"
                })),
            })
            .expect("create_account should succeed");

        let contents = fs::read_to_string(&fixture.audit_path).expect("audit file should exist");
        let lines = contents.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);

        let grant_entry: Value = serde_json::from_str(lines[0]).expect("grant entry should parse");
        let create_entry: Value =
            serde_json::from_str(lines[1]).expect("create entry should parse");

        assert_eq!(grant_entry["event"], "account_provisioning_grant");
        assert_eq!(grant_entry["result"], "granted");
        assert_eq!(create_entry["event"], "create_account");
        assert_eq!(create_entry["result"], "created");
        assert!(grant_entry.get("secret").is_none());
        assert!(create_entry.get("secret").is_none());
        assert!(grant_entry.get("uri").is_none());
        assert!(create_entry.get("uri").is_none());
    }

    #[test]
    fn update_and_remove_append_audit_entries_without_secret_fields() {
        let mut fixture = unlocked_server();
        let account_id = first_account_id(&fixture);

        fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "grant_account_provisioning".to_owned(),
                arguments: Some(json!({ "requested_account_limit": 2 })),
            })
            .expect("grant tool should succeed");
        fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "update_account".to_owned(),
                arguments: Some(json!({
                    "account_id": account_id,
                    "service": "Audit Updated",
                })),
            })
            .expect("update_account should succeed");
        fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "remove_account".to_owned(),
                arguments: Some(json!({ "account_id": account_id })),
            })
            .expect("remove_account should succeed");

        let contents = fs::read_to_string(&fixture.audit_path).expect("audit file should exist");
        let lines = contents.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 3);

        let grant_entry: Value = serde_json::from_str(lines[0]).expect("grant entry should parse");
        let update_entry: Value =
            serde_json::from_str(lines[1]).expect("update entry should parse");
        let remove_entry: Value =
            serde_json::from_str(lines[2]).expect("remove entry should parse");

        assert_eq!(grant_entry["event"], "account_provisioning_grant");
        assert_eq!(update_entry["event"], "update_account");
        assert_eq!(update_entry["result"], "updated");
        assert_eq!(remove_entry["event"], "remove_account");
        assert_eq!(remove_entry["result"], "removed");
        assert!(update_entry.get("secret").is_none());
        assert!(remove_entry.get("secret").is_none());
        assert!(update_entry.get("uri").is_none());
        assert!(remove_entry.get("uri").is_none());
    }

    #[test]
    fn provisioning_grant_rejects_limits_above_ten() {
        let mut fixture = unlocked_server();

        let result = fixture
            .server
            .handle_tool_call(ToolCallParams {
                name: "grant_account_provisioning".to_owned(),
                arguments: Some(json!({ "requested_account_limit": 11 })),
            })
            .expect("tool call should succeed");

        assert!(result.is_error);
        assert_eq!(
            result.structured_content.expect("structured content")["error"],
            "requested_account_limit debe estar entre 1 y 10."
        );
    }

    #[test]
    fn write_json_escapes_non_ascii_for_pipe_clients() {
        let mut buffer = Vec::new();
        let payload = json!({
            "message": "La sesion quedo abierta",
            "instructions": "La sesion es local y valida."
        });

        write_json(&mut buffer, &payload).expect("json should be written");

        let written = String::from_utf8(buffer).expect("buffer should be utf8");
        assert!(written.is_ascii());
        let parsed: Value = serde_json::from_str(written.trim()).expect("json should parse");
        assert_eq!(parsed["message"], "La sesion quedo abierta");
    }
}

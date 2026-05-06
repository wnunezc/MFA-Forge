use std::io::{self, BufRead, Write};

use serde::Serialize;
use serde_json::{Value, json};

use mfa_forge_core::AccountPublic;

use super::{
    prompt_helper,
    protocol::{
        AgentCommand, AgentErrorResponse, AgentRequest, AgentSuccessResponse, PROTOCOL_VERSION,
    },
    session::AgentSession,
    unlock, wire,
};

const AGENT_CAPABILITIES: &[&str] = &[
    "ping",
    "session_info",
    "list_accounts",
    "history",
    "generate_token",
    "add_account",
    "import_otpauth",
    "update_account",
    "remove_account",
    "export_metadata",
    "rotate_master_password",
    "close_session",
];

pub fn run_stdio_session() -> Result<(), String> {
    crate::runtime::ensure_supported_runtime("La sesión local mfa-forge-agent")?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();

    write_json(
        &mut writer,
        &json!({
            "event": "unlock_prompt_opened",
            "status": "waiting_user_action",
            "protocol": PROTOCOL_VERSION,
            "message": "MFA-Forge abrió una ventana nativa temporal para solicitar la contraseña del vault."
        }),
    )?;

    let vault = match unlock::run_unlock_window() {
        Ok(vault) => vault,
        Err(error) => {
            write_json(
                &mut writer,
                &json!({
                    "event": "startup_error",
                    "status": "access_denied",
                    "protocol": PROTOCOL_VERSION,
                    "error": error,
                }),
            )?;
            return Err("No se pudo abrir la sesión de agente.".to_owned());
        }
    };

    write_json(
        &mut writer,
        &json!({
            "event": "session_ready",
            "status": "access_granted",
            "protocol": PROTOCOL_VERSION,
            "vault_path": vault.path_display(),
            "capabilities": AGENT_CAPABILITIES,
            "windows_reinforced_unlock": "in_review",
            "message": "La sesión queda abierta mientras este proceso siga vivo o hasta recibir close_session."
        }),
    )?;

    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin.lock());
    let mut session = AgentSession::new(vault);
    let mut line = String::new();

    loop {
        line.clear();

        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|error| format!("No se pudo leer stdin: {error}"))?;

        if bytes_read == 0 {
            session.close();
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request = match serde_json::from_str::<AgentRequest>(trimmed) {
            Ok(request) => request,
            Err(error) => {
                write_json(
                    &mut writer,
                    &AgentErrorResponse {
                        id: Value::Null,
                        ok: false,
                        error: format!("La solicitud JSON es inválida: {error}"),
                    },
                )?;
                continue;
            }
        };

        match session.handle(request.command) {
            Ok(outcome) => {
                write_json(
                    &mut writer,
                    &AgentSuccessResponse {
                        id: request.id,
                        ok: true,
                        result: outcome.result,
                    },
                )?;

                if outcome.should_close {
                    break;
                }
            }
            Err(error) => {
                write_json(
                    &mut writer,
                    &AgentErrorResponse {
                        id: request.id,
                        ok: false,
                        error,
                    },
                )?;
            }
        }
    }

    Ok(())
}

struct CommandOutcome {
    result: Value,
    should_close: bool,
}

impl AgentSession {
    fn handle(&mut self, command: AgentCommand) -> Result<CommandOutcome, String> {
        match command {
            AgentCommand::Ping => Ok(CommandOutcome {
                result: json!({ "status": "ok" }),
                should_close: false,
            }),
            AgentCommand::SessionInfo => Ok(CommandOutcome {
                result: json!({
                    "status": "access_granted",
                    "vault_path": self.path_display(),
                    "account_count": self.account_count(),
                    "windows_reinforced_unlock": "in_review",
                }),
                should_close: false,
            }),
            AgentCommand::ListAccounts => Ok(CommandOutcome {
                result: json!({ "accounts": self.list_accounts() }),
                should_close: false,
            }),
            AgentCommand::History => Ok(CommandOutcome {
                result: json!({ "entries": self.history()? }),
                should_close: false,
            }),
            AgentCommand::GenerateToken { account_id } => {
                let token = self.generate_token(account_id)?;

                Ok(CommandOutcome {
                    result: serde_json::to_value(token)
                        .map_err(|error| format!("No se pudo serializar el token: {error}"))?,
                    should_close: false,
                })
            }
            AgentCommand::AddAccount {
                service,
                user,
                secret,
                totp,
            } => {
                let account = self.add_account(service, user, secret, totp.unwrap_or_default())?;

                Ok(CommandOutcome {
                    result: account_value("account", account)?,
                    should_close: false,
                })
            }
            AgentCommand::ImportOtpauth { uri } => {
                let account = self.import_otpauth(&uri)?;

                Ok(CommandOutcome {
                    result: account_value("account", account)?,
                    should_close: false,
                })
            }
            AgentCommand::UpdateAccount {
                account_id,
                service,
                user,
                secret,
                totp,
            } => {
                let updated = self.update_account(account_id, service, user, secret, totp)?;

                Ok(CommandOutcome {
                    result: account_value("account", updated)?,
                    should_close: false,
                })
            }
            AgentCommand::RemoveAccount { account_id } => {
                let removed = self.remove_account(account_id)?;

                Ok(CommandOutcome {
                    result: account_value("account", removed)?,
                    should_close: false,
                })
            }
            AgentCommand::ExportMetadata => {
                let accounts = self.export_metadata()?;
                Ok(CommandOutcome {
                    result: json!({ "accounts": accounts }),
                    should_close: false,
                })
            }
            AgentCommand::RotateMasterPassword => {
                self.rotate_master_password_with(prompt_helper::request_master_password_rotation)
            }
            AgentCommand::CloseSession => {
                self.close();
                Ok(CommandOutcome {
                    result: json!({ "status": "closing" }),
                    should_close: true,
                })
            }
        }
    }

    fn rotate_master_password_with<F>(&mut self, prompt: F) -> Result<CommandOutcome, String>
    where
        F: FnOnce() -> Result<crate::agent::PasswordRotationPromptDecision, String>,
    {
        match prompt()? {
            crate::agent::PasswordRotationPromptDecision::Approved { new_password } => {
                self.change_master_password(new_password)?;
                Ok(CommandOutcome {
                    result: json!({ "status": "rotated" }),
                    should_close: false,
                })
            }
            crate::agent::PasswordRotationPromptDecision::Denied => Ok(CommandOutcome {
                result: json!({ "status": "denied" }),
                should_close: false,
            }),
        }
    }
}

fn write_json<T>(writer: &mut impl Write, payload: &T) -> Result<(), String>
where
    T: Serialize,
{
    let json = wire::to_ascii_safe_json(payload)
        .map_err(|error| format!("No se pudo serializar la respuesta JSON: {error}"))?;
    writer
        .write_all(json.as_bytes())
        .map_err(|error| format!("No se pudo escribir la respuesta JSON: {error}"))?;
    writer
        .write_all(b"\n")
        .map_err(|error| format!("No se pudo escribir la respuesta JSON: {error}"))?;
    writer
        .flush()
        .map_err(|error| format!("No se pudo vaciar stdout: {error}"))
}

fn account_value(key: &str, account: AccountPublic) -> Result<Value, String> {
    let value = serde_json::to_value(account)
        .map_err(|error| format!("No se pudo serializar la cuenta: {error}"))?;
    Ok(json!({ key: value }))
}
#[cfg(test)]
mod tests {
    use secrecy::SecretString;
    use tempfile::TempDir;
    use uuid::Uuid;

    use mfa_forge_core::TotpConfig;
    use mfa_forge_storage::VaultRepository;

    use super::*;
    use crate::{
        agent::{PasswordRotationPromptDecision, session::AgentSession},
        vault::VaultFacade,
    };

    struct SessionFixture {
        _temp_dir: TempDir,
        session: AgentSession,
    }

    fn session_fixture() -> SessionFixture {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let repository = VaultRepository::new(temp_dir.path().join("vault.json"));
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

        SessionFixture {
            _temp_dir: temp_dir,
            session: AgentSession::new(vault),
        }
    }

    #[test]
    fn list_accounts_returns_accounts() {
        let mut fixture = session_fixture();

        let outcome = fixture
            .session
            .handle(AgentCommand::ListAccounts)
            .expect("command should succeed");

        let accounts = outcome
            .result
            .get("accounts")
            .and_then(Value::as_array)
            .expect("accounts array should exist");

        assert_eq!(accounts.len(), 1);
        assert!(!outcome.should_close);
    }

    #[test]
    fn close_session_locks_and_requests_exit() {
        let mut fixture = session_fixture();

        let outcome = fixture
            .session
            .handle(AgentCommand::CloseSession)
            .expect("command should succeed");

        assert_eq!(outcome.result["status"], "closing");
        assert!(outcome.should_close);
    }

    #[test]
    fn update_account_keeps_existing_totp_when_missing() {
        let mut fixture = session_fixture();
        let account_id = fixture
            .session
            .list_accounts()
            .first()
            .map(|account| account.id)
            .unwrap_or_else(Uuid::nil);

        let outcome = fixture
            .session
            .handle(AgentCommand::UpdateAccount {
                account_id,
                service: Some("GitHub Enterprise".to_owned()),
                user: None,
                secret: None,
                totp: None,
            })
            .expect("command should succeed");

        assert_eq!(outcome.result["account"]["service"], "GitHub Enterprise");
        assert_eq!(outcome.result["account"]["totp"]["digits"], 6);
    }

    #[test]
    fn history_returns_public_entries() {
        let mut fixture = session_fixture();
        let account_id = fixture
            .session
            .list_accounts()
            .first()
            .map(|account| account.id)
            .unwrap_or_else(Uuid::nil);

        fixture
            .session
            .handle(AgentCommand::UpdateAccount {
                account_id,
                service: Some("GitHub Audit".to_owned()),
                user: None,
                secret: None,
                totp: None,
            })
            .expect("update should succeed");

        let outcome = fixture
            .session
            .handle(AgentCommand::History)
            .expect("history should succeed");

        let entries = outcome
            .result
            .get("entries")
            .and_then(Value::as_array)
            .expect("entries array should exist");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["event"], "updated");
    }

    #[test]
    fn rotate_master_password_reencrypts_the_current_session_vault() {
        let mut fixture = session_fixture();

        let outcome = fixture
            .session
            .rotate_master_password_with(|| {
                Ok(PasswordRotationPromptDecision::Approved {
                    new_password: SecretString::from("new stronger password".to_owned()),
                })
            })
            .expect("rotation should succeed");

        assert_eq!(outcome.result["status"], "rotated");
        assert_eq!(fixture.session.account_count(), 1);
    }

    #[test]
    fn write_json_escapes_non_ascii_for_pipe_clients() {
        let mut buffer = Vec::new();
        let payload = serde_json::json!({
            "message": "sesion válida"
        });

        write_json(&mut buffer, &payload).expect("json should be written");

        let written = String::from_utf8(buffer).expect("buffer should be utf8");
        assert!(written.is_ascii());
        let parsed: Value = serde_json::from_str(written.trim()).expect("json should parse");
        assert_eq!(parsed["message"], "sesion válida");
    }
}

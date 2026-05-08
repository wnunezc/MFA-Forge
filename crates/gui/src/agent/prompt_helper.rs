use std::{
    io::{self, Write},
    process::{Command, Stdio},
    thread,
};

use mfa_forge_core::AccountPublic;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::diagnostics;

use super::unlock::{
    self, AuditReportingGrantPromptDecision, PasswordRotationPromptDecision,
    ProvisioningGrantPromptDecision, TokenGrantPromptDecision,
};

pub fn request_generate_token_grant(
    account: &AccountPublic,
    ttl_seconds: u64,
) -> Result<TokenGrantPromptDecision, String> {
    diagnostics::log_event(
        "prompt-helper-client",
        "request_generate_token_grant.start",
        json!({
            "ttl_seconds": ttl_seconds,
            "request_redacted": true,
        }),
    );
    let decision = map_token_grant_decision(request_grant_via_helper_subprocess(
        NativeGrantPromptRequest::Token {
            account: account.clone(),
            ttl_seconds,
        },
    )?);

    diagnostics::log_event(
        "prompt-helper-client",
        "request_generate_token_grant.completed",
        json!({
            "decision": summarize_decision_for_trace(decision),
        }),
    );
    Ok(decision)
}

pub fn request_account_provisioning_grant(
    account_limit: u8,
    ttl_minutes: u64,
) -> Result<ProvisioningGrantPromptDecision, String> {
    diagnostics::log_event(
        "prompt-helper-client",
        "request_account_provisioning_grant.start",
        json!({
            "account_limit": account_limit,
            "ttl_minutes": ttl_minutes,
        }),
    );
    let decision = map_provisioning_grant_decision(request_grant_via_helper_subprocess(
        NativeGrantPromptRequest::Provisioning {
            account_limit,
            ttl_minutes,
        },
    )?);

    diagnostics::log_event(
        "prompt-helper-client",
        "request_account_provisioning_grant.completed",
        json!({
            "decision": summarize_provisioning_decision_for_trace(decision),
        }),
    );
    Ok(decision)
}

pub fn request_audit_reporting_grant(
    read_limit: u8,
    ttl_minutes: u64,
) -> Result<AuditReportingGrantPromptDecision, String> {
    diagnostics::log_event(
        "prompt-helper-client",
        "request_audit_reporting_grant.start",
        json!({
            "read_limit": read_limit,
            "ttl_minutes": ttl_minutes,
        }),
    );
    let decision = map_audit_reporting_grant_decision(request_grant_via_helper_subprocess(
        NativeGrantPromptRequest::AuditReporting {
            read_limit,
            ttl_minutes,
        },
    )?);

    diagnostics::log_event(
        "prompt-helper-client",
        "request_audit_reporting_grant.completed",
        json!({
            "decision": summarize_audit_reporting_decision_for_trace(decision),
        }),
    );
    Ok(decision)
}

pub fn request_master_password_rotation() -> Result<PasswordRotationPromptDecision, String> {
    diagnostics::log_event(
        "prompt-helper-client",
        "request_master_password_rotation.start",
        json!({
            "input_redacted": true,
        }),
    );
    let decision = run_prompt_on_ui_thread(
        "mfa-forge-password-rotation-ui",
        unlock::run_password_rotation_window,
    )?;

    diagnostics::log_event(
        "prompt-helper-client",
        "request_master_password_rotation.completed",
        json!({
            "decision": summarize_password_rotation_decision_for_trace(&decision),
        }),
    );
    Ok(decision)
}

const NATIVE_GRANT_PROMPT_FLAG: &str = "--mfa-forge-native-grant-prompt";

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum NativeGrantPromptRequest {
    Token {
        account: AccountPublic,
        ttl_seconds: u64,
    },
    Provisioning {
        account_limit: u8,
        ttl_minutes: u64,
    },
    AuditReporting {
        read_limit: u8,
        ttl_minutes: u64,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum HelperGrantDecision {
    Approved,
    Denied,
}

#[derive(Debug, Deserialize, Serialize)]
struct NativeGrantPromptResponse {
    status: HelperResponseStatus,
    #[serde(default)]
    decision: Option<HelperGrantDecision>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum HelperResponseStatus {
    Ok,
    Error,
}

pub fn maybe_run_native_grant_prompt_from_env() -> Result<bool, String> {
    let Some(request) = parse_native_grant_prompt_args(std::env::args().skip(1))? else {
        return Ok(false);
    };

    let response = match run_native_grant_prompt_request(request) {
        Ok(decision) => NativeGrantPromptResponse {
            status: HelperResponseStatus::Ok,
            decision: Some(decision),
            error: None,
        },
        Err(error) => NativeGrantPromptResponse {
            status: HelperResponseStatus::Error,
            decision: None,
            error: Some(error),
        },
    };

    write_native_grant_prompt_response(&response)?;
    Ok(true)
}

fn request_grant_via_helper_subprocess(
    request: NativeGrantPromptRequest,
) -> Result<HelperGrantDecision, String> {
    let request_json = serde_json::to_string(&request).map_err(|error| {
        format!("No se pudo serializar la solicitud del prompt nativo: {error}")
    })?;
    let current_exe = std::env::current_exe()
        .map_err(|error| format!("No se pudo resolver el ejecutable actual: {error}"))?;
    let mut command = Command::new(current_exe);
    command
        .arg(NATIVE_GRANT_PROMPT_FLAG)
        .arg(request_json)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_hidden_process_flags(&mut command);

    let output = command
        .output()
        .map_err(|error| format!("No se pudo abrir el helper del prompt nativo: {error}"))?;

    if output.stdout.is_empty() && !output.status.success() {
        let stderr = String::from_utf8(output.stderr).unwrap_or_default();
        return Err(format!(
            "El helper del prompt nativo terminó con estado {}. {}",
            output.status,
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8(output.stdout).map_err(|error| {
        format!("El helper del prompt nativo devolvió stdout inválido: {error}")
    })?;
    let stderr = String::from_utf8(output.stderr).unwrap_or_default();
    let response: NativeGrantPromptResponse = serde_json::from_str(stdout.trim()).map_err(|error| {
        format!(
            "No se pudo interpretar la respuesta del helper del prompt nativo: {error}. stderr={}",
            stderr.trim()
        )
    })?;

    match response.status {
        HelperResponseStatus::Ok => response
            .decision
            .ok_or_else(|| "El helper del prompt nativo no devolvió una decisión.".to_owned()),
        HelperResponseStatus::Error => Err(response
            .error
            .unwrap_or_else(|| "El helper del prompt nativo falló sin detalle.".to_owned())),
    }
}

fn parse_native_grant_prompt_args<I>(
    mut args: I,
) -> Result<Option<NativeGrantPromptRequest>, String>
where
    I: Iterator<Item = String>,
{
    let Some(first_arg) = args.next() else {
        return Ok(None);
    };

    if first_arg != NATIVE_GRANT_PROMPT_FLAG {
        return Ok(None);
    }

    let request_json = args
        .next()
        .ok_or_else(|| "Falta el payload JSON del helper del prompt nativo.".to_owned())?;

    if args.next().is_some() {
        return Err("El helper del prompt nativo recibió argumentos extra inesperados.".to_owned());
    }

    let request = serde_json::from_str(&request_json).map_err(|error| {
        format!("El payload JSON del helper del prompt nativo no es válido: {error}")
    })?;
    Ok(Some(request))
}

fn run_native_grant_prompt_request(
    request: NativeGrantPromptRequest,
) -> Result<HelperGrantDecision, String> {
    match request {
        NativeGrantPromptRequest::Token {
            account,
            ttl_seconds,
        } => unlock::run_generate_token_grant_window(&account, ttl_seconds).map(|decision| {
            match decision {
                TokenGrantPromptDecision::Approved => HelperGrantDecision::Approved,
                TokenGrantPromptDecision::Denied => HelperGrantDecision::Denied,
            }
        }),
        NativeGrantPromptRequest::Provisioning {
            account_limit,
            ttl_minutes,
        } => unlock::run_account_provisioning_grant_window(account_limit, ttl_minutes).map(
            |decision| match decision {
                ProvisioningGrantPromptDecision::Approved => HelperGrantDecision::Approved,
                ProvisioningGrantPromptDecision::Denied => HelperGrantDecision::Denied,
            },
        ),
        NativeGrantPromptRequest::AuditReporting {
            read_limit,
            ttl_minutes,
        } => unlock::run_audit_reporting_grant_window(read_limit, ttl_minutes).map(|decision| {
            match decision {
                AuditReportingGrantPromptDecision::Approved => HelperGrantDecision::Approved,
                AuditReportingGrantPromptDecision::Denied => HelperGrantDecision::Denied,
            }
        }),
    }
}

fn write_native_grant_prompt_response(response: &NativeGrantPromptResponse) -> Result<(), String> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer(&mut handle, response).map_err(|error| {
        format!("No se pudo serializar la respuesta del helper nativo: {error}")
    })?;
    handle
        .write_all(b"\n")
        .map_err(|error| format!("No se pudo escribir la respuesta del helper nativo: {error}"))
}

fn apply_hidden_process_flags(command: &mut Command) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt as _;

        command.creation_flags(0x08000000);
    }
}

fn map_token_grant_decision(decision: HelperGrantDecision) -> TokenGrantPromptDecision {
    match decision {
        HelperGrantDecision::Approved => TokenGrantPromptDecision::Approved,
        HelperGrantDecision::Denied => TokenGrantPromptDecision::Denied,
    }
}

fn map_provisioning_grant_decision(
    decision: HelperGrantDecision,
) -> ProvisioningGrantPromptDecision {
    match decision {
        HelperGrantDecision::Approved => ProvisioningGrantPromptDecision::Approved,
        HelperGrantDecision::Denied => ProvisioningGrantPromptDecision::Denied,
    }
}

fn map_audit_reporting_grant_decision(
    decision: HelperGrantDecision,
) -> AuditReportingGrantPromptDecision {
    match decision {
        HelperGrantDecision::Approved => AuditReportingGrantPromptDecision::Approved,
        HelperGrantDecision::Denied => AuditReportingGrantPromptDecision::Denied,
    }
}

fn summarize_decision_for_trace(decision: TokenGrantPromptDecision) -> &'static str {
    match decision {
        TokenGrantPromptDecision::Approved => "approved",
        TokenGrantPromptDecision::Denied => "denied",
    }
}

fn summarize_provisioning_decision_for_trace(
    decision: ProvisioningGrantPromptDecision,
) -> &'static str {
    match decision {
        ProvisioningGrantPromptDecision::Approved => "approved",
        ProvisioningGrantPromptDecision::Denied => "denied",
    }
}

fn summarize_audit_reporting_decision_for_trace(
    decision: AuditReportingGrantPromptDecision,
) -> &'static str {
    match decision {
        AuditReportingGrantPromptDecision::Approved => "approved",
        AuditReportingGrantPromptDecision::Denied => "denied",
    }
}

fn summarize_password_rotation_decision_for_trace(
    decision: &PasswordRotationPromptDecision,
) -> &'static str {
    match decision {
        PasswordRotationPromptDecision::Approved { .. } => "approved",
        PasswordRotationPromptDecision::Denied => "denied",
    }
}

fn run_prompt_on_ui_thread<T, F>(thread_name: &str, prompt: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    thread::Builder::new()
        .name(thread_name.to_owned())
        .spawn(prompt)
        .map_err(|error| format!("No se pudo iniciar el prompt nativo '{thread_name}': {error}"))?
        .join()
        .map_err(|panic| {
            format!(
                "El prompt nativo '{thread_name}' terminó por panic: {}",
                diagnostics::panic_payload_message(panic)
            )
        })?
}

#[cfg(test)]
mod tests {
    use super::{
        HelperGrantDecision, HelperResponseStatus, NativeGrantPromptRequest,
        NativeGrantPromptResponse, parse_native_grant_prompt_args, run_prompt_on_ui_thread,
        summarize_audit_reporting_decision_for_trace, summarize_decision_for_trace,
        summarize_password_rotation_decision_for_trace, summarize_provisioning_decision_for_trace,
    };
    use crate::agent::unlock::{
        AuditReportingGrantPromptDecision, PasswordRotationPromptDecision,
        ProvisioningGrantPromptDecision, TokenGrantPromptDecision,
    };
    use mfa_forge_core::{AccountMetadata, AccountPublic, FactorKind, TotpConfig};
    use secrecy::SecretString;
    use uuid::Uuid;

    #[test]
    fn summarize_decision_for_trace_only_returns_safe_values() {
        assert_eq!(
            summarize_decision_for_trace(TokenGrantPromptDecision::Approved),
            "approved"
        );
        assert_eq!(
            summarize_decision_for_trace(TokenGrantPromptDecision::Denied),
            "denied"
        );
        assert_eq!(
            summarize_provisioning_decision_for_trace(ProvisioningGrantPromptDecision::Approved),
            "approved"
        );
        assert_eq!(
            summarize_provisioning_decision_for_trace(ProvisioningGrantPromptDecision::Denied),
            "denied"
        );
        assert_eq!(
            summarize_audit_reporting_decision_for_trace(
                AuditReportingGrantPromptDecision::Approved
            ),
            "approved"
        );
        assert_eq!(
            summarize_audit_reporting_decision_for_trace(AuditReportingGrantPromptDecision::Denied),
            "denied"
        );
        assert_eq!(
            summarize_password_rotation_decision_for_trace(
                &PasswordRotationPromptDecision::Approved {
                    new_password: SecretString::from("rotated".to_owned()),
                }
            ),
            "approved"
        );
        assert_eq!(
            summarize_password_rotation_decision_for_trace(&PasswordRotationPromptDecision::Denied),
            "denied"
        );
    }

    #[test]
    fn run_prompt_on_ui_thread_executes_on_a_dedicated_worker() {
        let caller_thread = format!("{:?}", std::thread::current().id());
        let worker_thread = run_prompt_on_ui_thread("prompt-helper-test", || {
            Ok::<_, String>(format!("{:?}", std::thread::current().id()))
        })
        .expect("prompt helper should return the worker thread id");

        assert_ne!(worker_thread, caller_thread);
    }

    #[test]
    fn run_prompt_on_ui_thread_propagates_prompt_errors() {
        let error = run_prompt_on_ui_thread::<(), _>("prompt-helper-test-error", || {
            Err("synthetic prompt failure".to_owned())
        })
        .expect_err("prompt helper should propagate prompt errors");

        assert_eq!(error, "synthetic prompt failure");
    }

    #[test]
    fn parse_native_grant_prompt_args_ignores_normal_process_startup() {
        let parsed = parse_native_grant_prompt_args(["--some-other-flag".to_owned()].into_iter())
            .expect("non-helper args should not fail");

        assert!(parsed.is_none());
    }

    #[test]
    fn parse_native_grant_prompt_args_parses_token_request() {
        let account = sample_account();
        let payload = serde_json::to_string(&NativeGrantPromptRequest::Token {
            account: account.clone(),
            ttl_seconds: 30,
        })
        .expect("request should serialize");

        let parsed = parse_native_grant_prompt_args(
            ["--mfa-forge-native-grant-prompt".to_owned(), payload].into_iter(),
        )
        .expect("helper args should parse")
        .expect("helper args should be detected");

        assert_eq!(
            parsed,
            NativeGrantPromptRequest::Token {
                account,
                ttl_seconds: 30,
            }
        );
    }

    #[test]
    fn native_grant_prompt_response_serializes_safe_status_values() {
        let approved = serde_json::to_value(NativeGrantPromptResponse {
            status: HelperResponseStatus::Ok,
            decision: Some(HelperGrantDecision::Approved),
            error: None,
        })
        .expect("approved response should serialize");
        let denied = serde_json::to_value(NativeGrantPromptResponse {
            status: HelperResponseStatus::Ok,
            decision: Some(HelperGrantDecision::Denied),
            error: None,
        })
        .expect("denied response should serialize");

        assert_eq!(approved["status"], "ok");
        assert_eq!(approved["decision"], "approved");
        assert_eq!(denied["decision"], "denied");
    }

    fn sample_account() -> AccountPublic {
        AccountPublic {
            id: Uuid::nil(),
            service: "GitHub".to_owned(),
            user: "user@example.com".to_owned(),
            kind: FactorKind::Totp,
            totp: TotpConfig::default(),
            created_at: 0,
            metadata: AccountMetadata::default(),
        }
    }
}

use mfa_forge_core::AccountPublic;
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
    let decision = unlock::run_generate_token_grant_window(account, ttl_seconds)?;

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
    let decision = unlock::run_account_provisioning_grant_window(account_limit, ttl_minutes)?;

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
    let decision = unlock::run_audit_reporting_grant_window(read_limit, ttl_minutes)?;

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
    let decision = unlock::run_password_rotation_window()?;

    diagnostics::log_event(
        "prompt-helper-client",
        "request_master_password_rotation.completed",
        json!({
            "decision": summarize_password_rotation_decision_for_trace(&decision),
        }),
    );
    Ok(decision)
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

#[cfg(test)]
mod tests {
    use super::{
        summarize_audit_reporting_decision_for_trace, summarize_decision_for_trace,
        summarize_password_rotation_decision_for_trace, summarize_provisioning_decision_for_trace,
    };
    use crate::agent::unlock::{
        AuditReportingGrantPromptDecision, PasswordRotationPromptDecision,
        ProvisioningGrantPromptDecision, TokenGrantPromptDecision,
    };
    use secrecy::SecretString;

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
}

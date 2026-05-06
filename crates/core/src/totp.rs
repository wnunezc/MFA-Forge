use serde::{Deserialize, Serialize};
use totp_rs::{Algorithm, Secret, TOTP};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::{AccountPublic, CoreError, TotpAlgorithm, TotpConfig};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TotpToken {
    pub account_id: Uuid,
    pub service: String,
    pub user: String,
    pub code: String,
    pub generated_at: u64,
    pub expires_at: u64,
    pub seconds_remaining: u64,
}

pub(crate) struct ImportedTotpAccount {
    pub service: String,
    pub user: String,
    pub secret: String,
    pub config: TotpConfig,
}

pub fn validate_secret(secret: &str) -> Result<String, CoreError> {
    let normalized = normalize_secret(secret);
    if normalized.is_empty() {
        return Err(CoreError::EmptySecret);
    }

    let mut bytes = Secret::Encoded(normalized.clone())
        .to_bytes()
        .map_err(|_| CoreError::InvalidTotpSecret)?;
    bytes.zeroize();

    Ok(normalized)
}

pub(crate) fn parse_otpauth_uri(uri: &str) -> Result<ImportedTotpAccount, CoreError> {
    let trimmed = uri.trim();
    if trimmed.is_empty() {
        return Err(CoreError::InvalidOtpAuthUri(
            "the URI cannot be empty".to_owned(),
        ));
    }

    let totp = TOTP::from_url_unchecked(trimmed)
        .map_err(|error| CoreError::InvalidOtpAuthUri(error.to_string()))?;
    let service = totp.issuer.clone().ok_or(CoreError::MissingOtpAuthIssuer)?;
    let digits = u32::try_from(totp.digits).map_err(|_| CoreError::InvalidTotpDigits)?;
    let encoded_secret = Secret::Raw(totp.secret.clone()).to_encoded();
    let secret = match &encoded_secret {
        Secret::Encoded(value) => value.clone(),
        Secret::Raw(_) => unreachable!("raw secrets always encode into base32"),
    };

    Ok(ImportedTotpAccount {
        service,
        user: totp.account_name.clone(),
        secret,
        config: TotpConfig {
            algorithm: from_totp_rs_algorithm(totp.algorithm),
            digits,
            period_seconds: totp.step,
        },
    })
}

pub(crate) fn build_otpauth_uri(
    account: &AccountPublic,
    secret: &str,
) -> Result<String, CoreError> {
    account.totp.validate()?;

    let secret_bytes = Secret::Encoded(secret.to_owned())
        .to_bytes()
        .map_err(|_| CoreError::InvalidTotpSecret)?;

    let totp = TOTP::new_unchecked(
        to_totp_rs_algorithm(account.totp.algorithm),
        account.totp.digits as usize,
        1,
        account.totp.period_seconds,
        secret_bytes,
        Some(account.service.clone()),
        account.user.clone(),
    );

    Ok(totp.get_url())
}

/// Generate a TOTP code for a specific Unix timestamp.
pub fn generate_token_at(
    account: &AccountPublic,
    secret: &str,
    timestamp: u64,
) -> Result<TotpToken, CoreError> {
    account.totp.validate()?;

    let secret_bytes = Secret::Encoded(secret.to_owned())
        .to_bytes()
        .map_err(|_| CoreError::InvalidTotpSecret)?;

    // MFA-Forge accepts valid Base32 secrets commonly found in otpauth:// URIs,
    // including shorter shared secrets that some providers still emit in practice.
    let totp = TOTP::new_unchecked(
        to_totp_rs_algorithm(account.totp.algorithm),
        account.totp.digits as usize,
        1,
        account.totp.period_seconds,
        secret_bytes,
        None,
        String::new(),
    );

    let code = totp.generate(timestamp);
    let expires_at = totp.next_step(timestamp);

    Ok(TotpToken {
        account_id: account.id,
        service: account.service.clone(),
        user: account.user.clone(),
        code,
        generated_at: timestamp,
        expires_at,
        seconds_remaining: expires_at.saturating_sub(timestamp),
    })
}

fn normalize_secret(secret: &str) -> String {
    secret
        .chars()
        .filter(|character| !character.is_ascii_whitespace() && *character != '-')
        .flat_map(char::to_uppercase)
        .collect()
}

fn to_totp_rs_algorithm(algorithm: TotpAlgorithm) -> Algorithm {
    match algorithm {
        TotpAlgorithm::Sha1 => Algorithm::SHA1,
        TotpAlgorithm::Sha256 => Algorithm::SHA256,
        TotpAlgorithm::Sha512 => Algorithm::SHA512,
    }
}

fn from_totp_rs_algorithm(algorithm: Algorithm) -> TotpAlgorithm {
    match algorithm {
        Algorithm::SHA1 => TotpAlgorithm::Sha1,
        Algorithm::SHA256 => TotpAlgorithm::Sha256,
        Algorithm::SHA512 => TotpAlgorithm::Sha512,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AccountRecord, TotpConfig, test_support::base32_secret_from_bytes,
        test_support::base32_secret_from_seed, test_support::secret_string_from_seed,
    };

    #[test]
    fn validates_base32_secret() {
        let secret = base32_secret_from_seed("validate-secret").to_lowercase();
        let normalized = super::validate_secret(&format!("{}  ", secret.replace("p", "p-")))
            .expect("secret should be valid");
        assert_eq!(normalized, base32_secret_from_seed("validate-secret"));
    }

    #[test]
    fn generates_rfc6238_vector() {
        let rfc_secret = base32_secret_from_bytes(b"12345678901234567890");
        let account = AccountRecord::new(
            "RFC",
            "vector@example.com",
            secrecy::SecretString::from(rfc_secret),
            TotpConfig {
                digits: 8,
                ..TotpConfig::default()
            },
        )
        .expect("account should be valid");

        let token = account
            .generate_token_at(59)
            .expect("token generation should work");

        assert_eq!(token.code, "94287082");
        assert_eq!(token.expires_at, 60);
        assert_eq!(token.seconds_remaining, 1);
    }

    #[test]
    fn generates_token_for_short_but_valid_base32_secret() {
        let secret = secret_string_from_seed("short-secret");
        let account = AccountRecord::new(
            "Example",
            "short@example.com",
            secret,
            TotpConfig::default(),
        )
        .expect("short secret should still be accepted");

        let token = account
            .generate_token_at(1_700_000_000)
            .expect("token generation should work for common short secrets");

        assert_eq!(token.code.len(), 6);
        assert_eq!(token.service, "Example");
        assert_eq!(token.user, "short@example.com");
    }

    #[test]
    fn parses_otpauth_uri_into_account_fields() {
        let secret = base32_secret_from_seed("parse-uri");
        let uri = AccountRecord::new(
            "GitHub",
            "user@example.com",
            secrecy::SecretString::from(secret.clone()),
            TotpConfig {
                algorithm: crate::TotpAlgorithm::Sha256,
                digits: 8,
                period_seconds: 60,
            },
        )
        .expect("account should be valid")
        .otpauth_uri()
        .expect("uri should build");
        let imported = super::parse_otpauth_uri(&uri).expect("otpauth URI should parse");

        assert_eq!(imported.service, "GitHub");
        assert_eq!(imported.user, "user@example.com");
        assert_eq!(imported.secret, secret);
        assert_eq!(imported.config.algorithm, crate::TotpAlgorithm::Sha256);
        assert_eq!(imported.config.digits, 8);
        assert_eq!(imported.config.period_seconds, 60);
    }

    #[test]
    fn imported_otpauth_account_can_generate_token_with_common_short_secret() {
        let uri = AccountRecord::new(
            "Agent-Test-Import",
            "agent.test@opszone.local",
            secret_string_from_seed("imported-account"),
            TotpConfig::default(),
        )
        .expect("account should be valid")
        .otpauth_uri()
        .expect("uri should build");
        let account = AccountRecord::from_otpauth_uri(&uri).expect("otpauth URI should import");

        let token = account
            .generate_token_at(1_700_000_000)
            .expect("imported account should generate a token");

        assert_eq!(token.code.len(), 6);
        assert_eq!(token.service, "Agent-Test-Import");
        assert_eq!(token.user, "agent.test@opszone.local");
    }

    #[test]
    fn exported_otpauth_uri_round_trips_account_identity() {
        let secret = base32_secret_from_seed("round-trip");
        let account = AccountRecord::new(
            "GitHub",
            "user@example.com",
            secrecy::SecretString::from(secret.clone()),
            TotpConfig::default(),
        )
        .expect("account should be valid");

        let uri = super::build_otpauth_uri(account.public(), &secret).expect("uri should build");
        let imported = super::parse_otpauth_uri(&uri).expect("uri should re-import");

        assert_eq!(imported.service, "GitHub");
        assert_eq!(imported.user, "user@example.com");
        assert_eq!(imported.secret, secret);
    }
}

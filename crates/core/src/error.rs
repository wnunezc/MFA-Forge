use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("{0} cannot be empty")]
    EmptyField(&'static str),
    #[error("{0} contains unsupported control characters")]
    InvalidTextField(&'static str),
    #[error("TOTP secret cannot be empty")]
    EmptySecret,
    #[error("TOTP secret is not a valid Base32 value")]
    InvalidTotpSecret,
    #[error("TOTP digits must be between 6 and 10")]
    InvalidTotpDigits,
    #[error("TOTP period must be between 15 and 300 seconds")]
    InvalidTotpPeriod,
    #[error("account selector did not match any stored account")]
    AccountNotFound,
    #[error("account selector matched multiple stored accounts; pass --user to disambiguate")]
    AmbiguousAccount,
    #[error("an account for this service and user already exists")]
    DuplicateAccount,
    #[error("otpauth URI is invalid: {0}")]
    InvalidOtpAuthUri(String),
    #[error("otpauth URI must include an issuer/service")]
    MissingOtpAuthIssuer,
    #[error("failed to construct the TOTP generator: {0}")]
    TotpBuild(String),
    #[error("system clock is invalid")]
    InvalidSystemTime(#[from] std::time::SystemTimeError),
}

use anyhow::{Result, bail};
use secrecy::{ExposeSecret, SecretString};

pub fn prompt_master_password(confirm: bool) -> Result<SecretString> {
    let password = SecretString::from(rpassword::prompt_password("Master password: ")?);
    if password.expose_secret().trim().is_empty() {
        bail!("master password cannot be empty");
    }

    if confirm {
        let confirmation = SecretString::from(rpassword::prompt_password("Confirm password: ")?);
        if password.expose_secret() != confirmation.expose_secret() {
            bail!("master password confirmation does not match");
        }
    }

    Ok(password)
}

pub fn prompt_totp_secret(secret: Option<String>) -> Result<SecretString> {
    match secret {
        Some(value) => {
            if value.trim().is_empty() {
                bail!("TOTP secret cannot be empty");
            }
            Ok(SecretString::from(value))
        }
        None => {
            let value = SecretString::from(rpassword::prompt_password("TOTP secret: ")?);
            if value.expose_secret().trim().is_empty() {
                bail!("TOTP secret cannot be empty");
            }
            Ok(value)
        }
    }
}

pub fn prompt_otpauth_uri(uri: Option<String>) -> Result<SecretString> {
    match uri {
        Some(value) => {
            if value.trim().is_empty() {
                bail!("otpauth URI cannot be empty");
            }
            Ok(SecretString::from(value))
        }
        None => {
            let value = SecretString::from(rpassword::prompt_password("otpauth URI: ")?);
            if value.expose_secret().trim().is_empty() {
                bail!("otpauth URI cannot be empty");
            }
            Ok(value)
        }
    }
}

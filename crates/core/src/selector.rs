use serde::{Deserialize, Serialize};

use crate::{
    AccountPublic, CoreError,
    account::{canonical_identity, normalize_text_field},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountSelector {
    service: String,
    user: Option<String>,
}

impl AccountSelector {
    /// Build a validated selector from CLI-facing inputs.
    pub fn new(service: impl Into<String>, user: Option<String>) -> Result<Self, CoreError> {
        Ok(Self {
            service: normalize_text_field(service.into(), "service")?,
            user: user
                .map(|value| normalize_text_field(value, "user"))
                .transpose()?,
        })
    }

    pub fn service(&self) -> &str {
        &self.service
    }

    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    pub fn matches(&self, account: &AccountPublic) -> bool {
        canonical_identity(self.service()) == canonical_identity(&account.service)
            && match self.user() {
                Some(user) => canonical_identity(user) == canonical_identity(&account.user),
                None => true,
            }
    }
}

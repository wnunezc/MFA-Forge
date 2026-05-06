use raw_window_handle::HasWindowHandle;
use serde_json::json;

pub use mfa_forge_platform_windows::{OwnerWindow, PendingVerification, VerificationPoll};

use crate::diagnostics;

pub fn capture_owner_window(source: &impl HasWindowHandle) -> Result<OwnerWindow, String> {
    mfa_forge_platform_windows::capture_owner_window(source)
}

pub fn begin_verify_unlock(owner_window: OwnerWindow) -> Result<PendingVerification, String> {
    diagnostics::log_event("platform-auth", "begin_verify_unlock.start", json!({}));
    let result = mfa_forge_platform_windows::begin_verify_unlock(owner_window);
    match &result {
        Ok(_) => diagnostics::log_event("platform-auth", "begin_verify_unlock.ok", json!({})),
        Err(error) => diagnostics::log_event(
            "platform-auth",
            "begin_verify_unlock.error",
            json!({ "error": error }),
        ),
    }
    result
}

pub fn settle_closed_prompt_window() {
    mfa_forge_platform_windows::settle_closed_prompt_window()
}

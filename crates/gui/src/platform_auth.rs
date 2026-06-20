use raw_window_handle::HasWindowHandle;
use serde_json::json;
use std::path::PathBuf;

pub use mfa_forge_platform_windows::{OwnerWindow, PendingVerification, VerificationPoll};

use crate::diagnostics;

pub fn capture_owner_window(source: &impl HasWindowHandle) -> Result<OwnerWindow, String> {
    mfa_forge_platform_windows::capture_owner_window(source)
}

pub fn configure_process_dpi_awareness() -> Result<(), String> {
    mfa_forge_platform_windows::configure_process_dpi_awareness()
}

pub fn initialize_main_window(owner_window: OwnerWindow) -> Result<(), String> {
    mfa_forge_platform_windows::initialize_main_window(owner_window, &main_window_state_path())
}

pub fn save_main_window(owner_window: OwnerWindow) -> Result<(), String> {
    mfa_forge_platform_windows::save_main_window(owner_window, &main_window_state_path())
}

fn main_window_state_path() -> PathBuf {
    mfa_forge_storage::app_data::data_local_file("main-window.json")
        .unwrap_or_else(|_| PathBuf::from("main-window.json"))
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

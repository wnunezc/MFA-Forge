use std::path::PathBuf;

use directories::ProjectDirs;

/// Returns a file path inside MFA-Forge's local application data directory.
pub fn data_local_file(file_name: &str) -> Result<PathBuf, String> {
    ProjectDirs::from("dev", "OpsZone", "MFA-Forge")
        .map(|dirs| dirs.data_local_dir().join(file_name))
        .ok_or_else(|| "The MFA-Forge local data directory could not be resolved.".to_owned())
}

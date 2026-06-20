use std::{
    ffi::OsString,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::StorageError;

pub(super) fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    let mut file = fs::File::create(path).map_err(StorageError::WriteFile)?;
    file.write_all(bytes).map_err(StorageError::WriteFile)?;
    file.sync_all().map_err(StorageError::WriteFile)?;
    Ok(())
}

pub(super) fn write_bytes_at(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(StorageError::CreateDir)?;
    }

    let temp_path = sibling_path_with_suffix(path, "tmp");
    remove_if_exists(&temp_path)?;
    write_bytes(&temp_path, bytes)?;

    if let Err(error) = (|| {
        remove_if_exists(path)?;
        fs::rename(&temp_path, path).map_err(StorageError::PersistVault)
    })() {
        let _ = remove_if_exists(&temp_path);
        return Err(error);
    }

    Ok(())
}

pub(super) fn remove_if_exists(path: &Path) -> Result<(), StorageError> {
    if path.exists() {
        fs::remove_file(path).map_err(StorageError::RemoveFile)?;
    }

    Ok(())
}

pub(super) fn sibling_path_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut file_name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| OsString::from("vault"));
    file_name.push(format!(".{suffix}"));
    path.with_file_name(file_name)
}

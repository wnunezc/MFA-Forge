use std::collections::BTreeMap;

use mfa_forge_core::{ProjectDirectory, normalize_project_path_value};

use crate::{StorageError, types::VaultData};

pub(super) fn normalize_directory_registry(vault: &mut VaultData) -> Result<bool, StorageError> {
    let mut directories_by_path = BTreeMap::<String, (u64, u64)>::new();

    for directory in &vault.directories {
        let normalized_path = normalize_project_path_value(directory.path.clone())?;
        let entry = directories_by_path
            .entry(normalized_path)
            .or_insert((directory.created_at, directory.updated_at));
        entry.0 = entry.0.min(directory.created_at);
        entry.1 = entry.1.max(directory.updated_at.max(directory.created_at));
    }

    for account in &vault.accounts {
        let Some(project_path) = account.public.metadata.project_path.clone() else {
            continue;
        };

        let normalized_path = normalize_project_path_value(project_path)?;
        let entry = directories_by_path.entry(normalized_path).or_insert((
            account.public.created_at,
            account
                .public
                .metadata
                .updated_at
                .max(account.public.created_at),
        ));
        entry.0 = entry.0.min(account.public.created_at);
        entry.1 = entry.1.max(
            account
                .public
                .metadata
                .updated_at
                .max(account.public.created_at),
        );
    }

    let mut normalized_directories = directories_by_path
        .into_iter()
        .map(|(path, (created_at, updated_at))| {
            ProjectDirectory::with_timestamps(path, created_at, updated_at)
        })
        .collect::<Result<Vec<_>, _>>()?;
    sort_directories(&mut normalized_directories);

    let changed = vault.directories != normalized_directories;
    vault.directories = normalized_directories;
    Ok(changed)
}

fn sort_directories(directories: &mut [ProjectDirectory]) {
    directories.sort_by_key(|directory| directory.path.to_ascii_lowercase());
}

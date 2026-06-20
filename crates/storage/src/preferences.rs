use std::{fs, path::Path};

use serde::{Serialize, de::DeserializeOwned};

/// Loads a JSON preferences file, returning `T::default()` when the file is
/// missing or unreadable.
pub fn load_or_default<T>(path: &Path) -> T
where
    T: Default + DeserializeOwned,
{
    let Ok(bytes) = fs::read(path) else {
        return T::default();
    };

    serde_json::from_slice::<T>(&bytes).unwrap_or_default()
}

/// Persists a JSON preferences file under the user's local app data directory.
pub fn save<T>(path: &Path, preferences: &T) -> Result<(), String>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("The preferences directory could not be created: {error}"))?;
    }

    let payload = serde_json::to_vec_pretty(preferences)
        .map_err(|error| format!("The preferences could not be serialized: {error}"))?;

    fs::write(path, payload).map_err(|error| format!("The preferences could not be saved: {error}"))
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    use super::{load_or_default, save};

    #[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct ExamplePreferences {
        value: String,
    }

    #[test]
    fn missing_preferences_return_default() {
        let temp_dir = TempDir::new().expect("temp dir should exist");

        let loaded: ExamplePreferences = load_or_default(&temp_dir.path().join("missing.json"));

        assert_eq!(loaded, ExamplePreferences::default());
    }

    #[test]
    fn preferences_round_trip_through_json() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let path = temp_dir.path().join("nested").join("preferences.json");

        save(
            &path,
            &ExamplePreferences {
                value: "dark".to_owned(),
            },
        )
        .expect("preferences should save");
        let loaded: ExamplePreferences = load_or_default(&path);

        assert_eq!(
            loaded,
            ExamplePreferences {
                value: "dark".to_owned()
            }
        );
    }
}

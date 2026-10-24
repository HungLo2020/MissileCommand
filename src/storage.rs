//! Versioned user data persisted through Quasar's Basalt v1 storage helpers.
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{io, path::Path};

/// Missing files return `None`; malformed or unsupported data remains the caller's concern.
pub fn load<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    quasar_storage::read_json(path)
}

/// Atomic temporary-file-plus-rename replacement in the destination directory.
pub fn atomic_save<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    quasar_storage::write_json(path, value)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Records {
    pub version: u32,
    pub high_score: u64,
    pub best_wave: u32,
}

impl Default for Records {
    fn default() -> Self {
        Self {
            version: 1,
            high_score: 0,
            best_wave: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub version: u32,
    pub muted: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            muted: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn older_records_receive_defaults() {
        let records: Records = serde_json::from_str(r#"{"high_score":7500}"#).unwrap();
        assert_eq!(
            (records.version, records.high_score, records.best_wave),
            (1, 7500, 0)
        );
    }

    #[test]
    fn quasar_storage_replaces_atomically_and_preserves_malformed_files() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("data/save/records.json");
        atomic_save(
            &path,
            &Records {
                high_score: 12345,
                ..Records::default()
            },
        )
        .unwrap();
        atomic_save(
            &path,
            &Records {
                high_score: 23456,
                ..Records::default()
            },
        )
        .unwrap();
        assert_eq!(load::<Records>(&path).unwrap().unwrap().high_score, 23456);
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);

        fs::write(&path, b"broken").unwrap();
        assert!(load::<Records>(&path).is_err());
        assert_eq!(fs::read(path).unwrap(), b"broken");
    }
}

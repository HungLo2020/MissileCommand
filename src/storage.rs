//! Basalt standard v1 paths and versioned, atomically replaced user data.
use serde::{Deserialize, Serialize};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub save: PathBuf,
    pub config: PathBuf,
    pub shared: PathBuf,
    pub cache: PathBuf,
}

impl Paths {
    pub fn discover() -> io::Result<Self> {
        let executable = env::current_exe()?;
        let root = env::var_os("BASALT_GAME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| Self::root_from_executable(&executable));
        let location = |key: &str, fallback: &str| {
            env::var_os(key)
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join(fallback))
        };
        Ok(Self {
            save: location("BASALT_SAVE_DIR", "data/save"),
            config: location("BASALT_CONFIG_DIR", "data/config"),
            shared: location("BASALT_SHARED_CONFIG_DIR", "data/config/shared"),
            cache: location("BASALT_CACHE_DIR", "data/cache"),
            root,
        })
    }

    /// A packaged executable lives at <root>/app/bin; cargo builds use the repo root.
    pub fn root_from_executable(executable: &Path) -> PathBuf {
        let bin = executable.parent().unwrap_or(Path::new("."));
        if bin.file_name().is_some_and(|n| n == "bin") {
            if let Some(app) = bin
                .parent()
                .filter(|p| p.file_name().is_some_and(|n| n == "app"))
            {
                return app.parent().unwrap_or(app).to_path_buf();
            }
        }
        if bin
            .file_name()
            .is_some_and(|n| n == "debug" || n == "release")
        {
            if let Some(target) = bin
                .parent()
                .filter(|p| p.file_name().is_some_and(|n| n == "target"))
            {
                return target.parent().unwrap_or(target).to_path_buf();
            }
        }
        bin.to_path_buf()
    }

    pub fn create(&self) -> io::Result<()> {
        for path in [&self.save, &self.config, &self.shared, &self.cache] {
            fs::create_dir_all(path)?;
        }
        Ok(())
    }
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

pub fn load<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> io::Result<T> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(e),
    }
}

/// The temporary file is in the destination directory, so commit is a filesystem rename.
/// A synced directory makes the rename durable on Unix as well as the file contents.
pub fn atomic_save<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    use io::Write;
    let parent = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_path_is_independent_of_working_directory() {
        assert_eq!(
            Paths::root_from_executable(Path::new(
                "/games with spaces/MissileCommand/app/bin/missile-command"
            )),
            Path::new("/games with spaces/MissileCommand")
        );
    }

    #[test]
    fn saves_replace_atomically_and_older_fields_migrate() {
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
        assert_eq!(load::<Records>(&path).unwrap().high_score, 23456);
        fs::write(&path, br#"{"high_score":7500}"#).unwrap();
        let migrated = load::<Records>(&path).unwrap();
        assert_eq!(
            (migrated.version, migrated.high_score, migrated.best_wave),
            (1, 7500, 0)
        );
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn malformed_save_is_reported_without_overwriting() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("records.json");
        fs::write(&path, b"broken").unwrap();
        assert_eq!(
            load::<Records>(&path).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(fs::read(path).unwrap(), b"broken");
    }
}

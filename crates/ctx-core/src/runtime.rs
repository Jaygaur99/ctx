use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeState {
    pub version: u32,
    pub active_workspace: Option<String>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            version: 1,
            active_workspace: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("failed to read runtime state at {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("invalid runtime state at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("unsupported runtime state version {found}; expected version 1")]
    UnsupportedVersion { found: u32 },

    #[error("failed to create runtime directory at {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to serialize runtime state: {source}")]
    Serialize {
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to write runtime state at {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl RuntimeState {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let path = path.as_ref();
        let json = match fs::read_to_string(path) {
            Ok(json) => json,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(RuntimeError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        let state: Self = serde_json::from_str(&json).map_err(|source| RuntimeError::Parse {
            path: path.to_path_buf(),
            source,
        })?;

        if state.version != 1 {
            return Err(RuntimeError::UnsupportedVersion {
                found: state.version,
            });
        }

        Ok(state)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), RuntimeError> {
        let path = path.as_ref();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| RuntimeError::CreateDirectory {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let temporary_path = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(self)
            .map_err(|source| RuntimeError::Serialize { source })?;

        fs::write(&temporary_path, format!("{json}\n")).map_err(|source| RuntimeError::Write {
            path: temporary_path.clone(),
            source,
        })?;
        fs::rename(&temporary_path, path).map_err(|source| RuntimeError::Write {
            path: path.to_path_buf(),
            source,
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_runtime_file_returns_default_state() {
        let directory = tempdir().unwrap();

        let state = RuntimeState::load(directory.path().join("runtime.json")).unwrap();

        assert_eq!(state, RuntimeState::default());
    }

    #[test]
    fn saves_and_loads_active_workspace() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("nested/runtime.json");
        let state = RuntimeState {
            version: 1,
            active_workspace: Some("coding".to_string()),
        };

        state.save(&path).unwrap();

        assert_eq!(RuntimeState::load(path).unwrap(), state);
    }

    #[test]
    fn rejects_unsupported_runtime_version() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("runtime.json");
        fs::write(&path, r#"{"version":2,"active_workspace":null}"#).unwrap();

        let error = RuntimeState::load(path).unwrap_err();

        assert!(matches!(
            error,
            RuntimeError::UnsupportedVersion { found: 2 }
        ));
    }
}

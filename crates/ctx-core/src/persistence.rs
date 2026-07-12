use std::{
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::{Config, RuntimeState};

#[derive(Debug, Error)]
pub enum SwitchPersistenceError {
    #[error("failed to serialize workspace config: {0}")]
    SerializeConfig(#[from] serde_yaml::Error),

    #[error("failed to serialize runtime state: {0}")]
    SerializeRuntime(#[from] serde_json::Error),

    #[error("failed to create transaction directory at {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to read existing transaction file at {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to prepare transaction file at {path}: {source}")]
    Prepare {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error(
        "failed to commit transaction file at {path}: {source}{rollback}",
        rollback = rollback_suffix(.rollback_error)
    )]
    Commit {
        path: PathBuf,
        #[source]
        source: io::Error,
        rollback_error: Option<String>,
    },
}

pub fn save_switch_transaction(
    config: &Config,
    config_path: impl AsRef<Path>,
    state: &RuntimeState,
    runtime_path: impl AsRef<Path>,
) -> Result<(), SwitchPersistenceError> {
    save_switch_transaction_with_hook(config, config_path, state, runtime_path, || Ok(()))
}

fn save_switch_transaction_with_hook(
    config: &Config,
    config_path: impl AsRef<Path>,
    state: &RuntimeState,
    runtime_path: impl AsRef<Path>,
    before_runtime_commit: impl FnOnce() -> io::Result<()>,
) -> Result<(), SwitchPersistenceError> {
    let config_path = config_path.as_ref();
    let runtime_path = runtime_path.as_ref();
    create_parent(config_path)?;
    create_parent(runtime_path)?;

    let original_config = read_optional(config_path)?;
    let config_yaml = serde_yaml::to_string(config)?;
    let runtime_json = format!("{}\n", serde_json::to_string_pretty(state)?);
    let config_temporary = transaction_path(config_path);
    let runtime_temporary = transaction_path(runtime_path);

    prepare(&config_temporary, config_yaml.as_bytes())?;
    if let Err(error) = prepare(&runtime_temporary, runtime_json.as_bytes()) {
        remove_if_present(&config_temporary);
        return Err(error);
    }

    if let Err(source) = fs::rename(&config_temporary, config_path) {
        remove_if_present(&config_temporary);
        remove_if_present(&runtime_temporary);
        return Err(SwitchPersistenceError::Commit {
            path: config_path.to_path_buf(),
            source,
            rollback_error: None,
        });
    }

    let runtime_commit =
        before_runtime_commit().and_then(|()| fs::rename(&runtime_temporary, runtime_path));
    if let Err(source) = runtime_commit {
        remove_if_present(&runtime_temporary);
        let rollback_error = rollback(config_path, original_config.as_deref())
            .err()
            .map(|error| error.to_string());
        return Err(SwitchPersistenceError::Commit {
            path: runtime_path.to_path_buf(),
            source,
            rollback_error,
        });
    }

    Ok(())
}

fn create_parent(path: &Path) -> Result<(), SwitchPersistenceError> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    fs::create_dir_all(parent).map_err(|source| SwitchPersistenceError::CreateDirectory {
        path: parent.to_path_buf(),
        source,
    })
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, SwitchPersistenceError> {
    match fs::read(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(SwitchPersistenceError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn prepare(path: &Path, contents: &[u8]) -> Result<(), SwitchPersistenceError> {
    fs::write(path, contents).map_err(|source| SwitchPersistenceError::Prepare {
        path: path.to_path_buf(),
        source,
    })
}

fn rollback(path: &Path, original: Option<&[u8]>) -> io::Result<()> {
    match original {
        Some(contents) => {
            let temporary = transaction_path(path);
            fs::write(&temporary, contents)?;
            fs::rename(temporary, path)
        }
        None => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        },
    }
}

fn transaction_path(path: &Path) -> PathBuf {
    let mut file_name = path.file_name().unwrap_or_default().to_os_string();
    file_name.push(".ctx-switch.tmp");
    path.with_file_name(file_name)
}

fn remove_if_present(path: &Path) {
    let _ = fs::remove_file(path);
}

fn rollback_suffix(error: &Option<String>) -> String {
    error
        .as_ref()
        .map(|error| format!("; config rollback also failed: {error}"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tempfile::tempdir;

    use super::*;

    fn config(name: &str) -> Config {
        let mut config = Config {
            version: 1,
            ignored_windows: Vec::new(),
            workspaces: BTreeMap::new(),
        };
        config.add_workspace(name, Vec::new()).unwrap();
        config
    }

    fn state(name: &str) -> RuntimeState {
        RuntimeState {
            version: 1,
            active_workspace: Some(name.to_string()),
        }
    }

    #[test]
    fn commits_config_and_runtime_together() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("config/workspaces.yaml");
        let runtime_path = directory.path().join("runtime/runtime.json");
        let config = config("coding");
        let state = state("coding");

        save_switch_transaction(&config, &config_path, &state, &runtime_path).unwrap();

        assert_eq!(Config::load(config_path).unwrap(), config);
        assert_eq!(RuntimeState::load(runtime_path).unwrap(), state);
    }

    #[test]
    fn runtime_commit_failure_restores_previous_config() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("workspaces.yaml");
        let runtime_path = directory.path().join("runtime.json");
        let previous_config = config("previous");
        let previous_state = state("previous");
        previous_config.save(&config_path).unwrap();
        previous_state.save(&runtime_path).unwrap();
        let previous_config_bytes = fs::read(&config_path).unwrap();
        let previous_runtime_bytes = fs::read(&runtime_path).unwrap();

        let error = save_switch_transaction_with_hook(
            &config("target"),
            &config_path,
            &state("target"),
            &runtime_path,
            || Err(io::Error::other("simulated runtime commit failure")),
        )
        .unwrap_err();

        assert!(matches!(error, SwitchPersistenceError::Commit { .. }));
        assert_eq!(fs::read(config_path).unwrap(), previous_config_bytes);
        assert_eq!(fs::read(runtime_path).unwrap(), previous_runtime_bytes);
    }
}

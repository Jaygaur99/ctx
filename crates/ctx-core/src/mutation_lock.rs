use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use fs2::FileExt;
use thiserror::Error;

pub const DEFAULT_MUTATION_LOCK_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Error)]
pub enum MutationLockError {
    #[error("failed to create mutation-lock directory at {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to open mutation lock at {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("another Ctx operation is updating {path}; try again in a moment")]
    Busy { path: PathBuf },

    #[error("failed to acquire mutation lock at {path}: {source}")]
    Lock {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Debug)]
pub struct MutationGuard {
    file: File,
}

impl Drop for MutationGuard {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

pub fn acquire_mutation_lock(
    config_path: &Path,
    timeout: Duration,
) -> Result<MutationGuard, MutationLockError> {
    let mut lock_name = config_path.as_os_str().to_os_string();
    lock_name.push(".lock");
    let lock_path = PathBuf::from(lock_name);
    if let Some(parent) = lock_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| MutationLockError::CreateDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|source| MutationLockError::Open {
            path: lock_path.clone(),
            source,
        })?;
    let started = Instant::now();

    loop {
        match FileExt::try_lock_exclusive(&file) {
            Ok(()) => return Ok(MutationGuard { file }),
            Err(source) if source.kind() == io::ErrorKind::WouldBlock => {
                if started.elapsed() >= timeout {
                    return Err(MutationLockError::Busy { path: lock_path });
                }
                thread::sleep(Duration::from_millis(40));
            }
            Err(source) => {
                return Err(MutationLockError::Lock {
                    path: lock_path,
                    source,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use tempfile::tempdir;

    #[test]
    fn competing_mutation_returns_busy_and_released_lock_can_be_reacquired() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("workspaces.yaml");
        let guard = acquire_mutation_lock(&config_path, Duration::from_millis(20)).unwrap();

        let error = acquire_mutation_lock(&config_path, Duration::from_millis(20)).unwrap_err();
        assert!(matches!(error, MutationLockError::Busy { .. }));

        drop(guard);
        acquire_mutation_lock(&config_path, Duration::from_millis(20)).unwrap();
    }

    #[test]
    fn different_config_paths_have_independent_locks() {
        let directory = tempdir().unwrap();
        let first = acquire_mutation_lock(
            &directory.path().join("one/workspaces.yaml"),
            Duration::from_millis(20),
        )
        .unwrap();

        let second = acquire_mutation_lock(
            &directory.path().join("two/workspaces.yaml"),
            Duration::from_millis(20),
        )
        .unwrap();

        drop((first, second));
    }

    #[test]
    fn waiting_mutation_acquires_lock_after_competing_guard_is_released() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("workspaces.yaml");
        let first = acquire_mutation_lock(&config_path, Duration::from_millis(20)).unwrap();
        let waiting_path = config_path.clone();
        let (started_tx, started_rx) = mpsc::channel();

        let waiting = thread::spawn(move || {
            started_tx.send(()).unwrap();
            acquire_mutation_lock(&waiting_path, Duration::from_secs(1))
        });
        started_rx.recv().unwrap();
        thread::sleep(Duration::from_millis(80));
        drop(first);

        waiting.join().unwrap().unwrap();
    }

    #[test]
    fn guard_is_released_when_a_mutation_returns_an_error() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("workspaces.yaml");

        let result: Result<(), &'static str> = {
            let _guard = acquire_mutation_lock(&config_path, Duration::from_millis(20)).unwrap();
            Err("mutation failed")
        };
        assert_eq!(result, Err("mutation failed"));

        acquire_mutation_lock(&config_path, Duration::from_millis(20)).unwrap();
    }
}

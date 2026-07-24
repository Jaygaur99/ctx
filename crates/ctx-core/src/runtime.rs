use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeState {
    pub version: u32,
    pub active_workspace: Option<String>,

    #[serde(default, skip_serializing_if = "UrlSessionState::is_empty")]
    pub url_session: UrlSessionState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrlSessionState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot_id: Option<String>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub opened: BTreeMap<String, BTreeSet<String>>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub failures: BTreeMap<String, BTreeMap<String, String>>,
}

impl UrlSessionState {
    pub fn is_empty(&self) -> bool {
        self.boot_id.is_none() && self.opened.is_empty() && self.failures.is_empty()
    }
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            version: 1,
            active_workspace: None,
            url_session: UrlSessionState::default(),
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
    pub fn ensure_url_boot_session(&mut self, boot_id: &str) {
        if self.url_session.boot_id.as_deref() != Some(boot_id) {
            self.url_session = UrlSessionState {
                boot_id: Some(boot_id.to_string()),
                ..UrlSessionState::default()
            };
        }
    }

    pub fn url_was_opened(&self, boot_id: &str, workspace: &str, url: &str) -> bool {
        self.url_session.boot_id.as_deref() == Some(boot_id)
            && self
                .url_session
                .opened
                .get(workspace)
                .is_some_and(|urls| urls.contains(url))
    }

    pub fn url_failure(&self, boot_id: &str, workspace: &str, url: &str) -> Option<&str> {
        (self.url_session.boot_id.as_deref() == Some(boot_id))
            .then(|| self.url_session.failures.get(workspace)?.get(url))
            .flatten()
            .map(String::as_str)
    }

    pub fn mark_url_opened(&mut self, workspace: &str, url: &str) {
        self.url_session
            .opened
            .entry(workspace.to_string())
            .or_default()
            .insert(url.to_string());
        self.clear_url_failure(workspace, url);
    }

    pub fn mark_url_failed(&mut self, workspace: &str, url: &str, error: &str) {
        self.url_session
            .failures
            .entry(workspace.to_string())
            .or_default()
            .insert(url.to_string(), error.to_string());
    }

    pub fn clear_url_failure(&mut self, workspace: &str, url: &str) {
        if let Some(failures) = self.url_session.failures.get_mut(workspace) {
            failures.remove(url);
            if failures.is_empty() {
                self.url_session.failures.remove(workspace);
            }
        }
    }

    pub fn clear_workspace_url(&mut self, workspace: &str, url: &str) {
        if let Some(opened) = self.url_session.opened.get_mut(workspace) {
            opened.remove(url);
            if opened.is_empty() {
                self.url_session.opened.remove(workspace);
            }
        }
        self.clear_url_failure(workspace, url);
    }

    pub fn clear_workspace_urls(&mut self, workspace: &str) {
        self.url_session.opened.remove(workspace);
        self.url_session.failures.remove(workspace);
    }

    pub fn rename_workspace(&mut self, previous_name: &str, new_name: &str) {
        if previous_name == new_name {
            return;
        }
        if self.active_workspace.as_deref() == Some(previous_name) {
            self.active_workspace = Some(new_name.to_string());
        }
        if let Some(opened) = self.url_session.opened.remove(previous_name) {
            self.url_session
                .opened
                .entry(new_name.to_string())
                .or_default()
                .extend(opened);
        }
        if let Some(failures) = self.url_session.failures.remove(previous_name) {
            self.url_session
                .failures
                .entry(new_name.to_string())
                .or_default()
                .extend(failures);
        }
    }

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
            url_session: UrlSessionState::default(),
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

    #[test]
    fn legacy_runtime_defaults_url_session_state() {
        let state: RuntimeState =
            serde_json::from_str(r#"{"version":1,"active_workspace":"coding"}"#).unwrap();

        assert_eq!(state.active_workspace.as_deref(), Some("coding"));
        assert!(state.url_session.is_empty());
    }

    #[test]
    fn changing_boot_session_clears_opened_urls_and_failures() {
        let mut state = RuntimeState::default();
        state.ensure_url_boot_session("boot-1");
        state.mark_url_opened("coding", "https://example.com/");
        state.mark_url_failed("coding", "https://failed.example/", "failed");

        state.ensure_url_boot_session("boot-2");

        assert_eq!(state.url_session.boot_id.as_deref(), Some("boot-2"));
        assert!(state.url_session.opened.is_empty());
        assert!(state.url_session.failures.is_empty());
    }

    #[test]
    fn clears_individual_and_workspace_url_markers() {
        let mut state = RuntimeState::default();
        state.ensure_url_boot_session("boot-1");
        state.mark_url_opened("coding", "https://one.example/");
        state.mark_url_opened("coding", "https://two.example/");
        state.mark_url_failed("coding", "https://failed.example/", "failed");

        state.clear_workspace_url("coding", "https://one.example/");
        assert!(!state.url_was_opened("boot-1", "coding", "https://one.example/"));
        assert!(state.url_was_opened("boot-1", "coding", "https://two.example/"));

        state.clear_workspace_urls("coding");
        assert!(!state.url_was_opened("boot-1", "coding", "https://two.example/"));
        assert!(state.url_session.failures.is_empty());
    }

    #[test]
    fn renaming_workspace_moves_active_state_and_url_markers() {
        let mut state = RuntimeState {
            active_workspace: Some("coding".to_string()),
            ..RuntimeState::default()
        };
        state.ensure_url_boot_session("boot");
        state.mark_url_opened("coding", "https://one.example/");
        state.mark_url_failed("coding", "https://two.example/", "offline");

        state.rename_workspace("coding", "deep-work");

        assert_eq!(state.active_workspace.as_deref(), Some("deep-work"));
        assert!(state.url_was_opened("boot", "deep-work", "https://one.example/"));
        assert_eq!(
            state.url_failure("boot", "deep-work", "https://two.example/"),
            Some("offline")
        );
        assert!(!state.url_session.opened.contains_key("coding"));
        assert!(!state.url_session.failures.contains_key("coding"));
    }
}

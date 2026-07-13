use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::WindowInfo;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("workspace name cannot be empty")]
    EmptyWorkspaceName,

    #[error("workspace '{name}' already exists")]
    WorkspaceAlreadyExists { name: String },

    #[error("workspace '{name}' does not exist")]
    WorkspaceMissing { name: String },

    #[error("config already exists at {path}")]
    AlreadyExists { path: PathBuf },

    #[error("failed to create config directory at {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to create config at {path}: {source}")]
    Create {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to read config at {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("invalid YAML at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    #[error("unsupported config version {found}; expected version 1")]
    UnsupportedVersion { found: u32 },

    #[error("failed to serialize config: {source}")]
    Serialize {
        #[source]
        source: serde_yaml::Error,
    },

    #[error("failed to write config at {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored_windows: Vec<WindowInfo>,

    pub workspaces: BTreeMap<String, Workspace>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<Service>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<WindowInfo>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Service {
    Process {
        name: String,
        run: String,
    },

    Managed {
        name: String,
        start: String,
        stop: String,
    },
}

impl Config {
    pub fn init(path: impl AsRef<Path>) -> Result<(), ConfigError> {
        const DEFAULT_CONFIG: &str = "version: 1\nworkspaces: {}\n";

        let path = path.as_ref();

        if path.exists() {
            return Err(ConfigError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| ConfigError::CreateDirectory {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|source| {
                if source.kind() == io::ErrorKind::AlreadyExists {
                    ConfigError::AlreadyExists {
                        path: path.to_path_buf(),
                    }
                } else {
                    ConfigError::Create {
                        path: path.to_path_buf(),
                        source,
                    }
                }
            })?;

        file.write_all(DEFAULT_CONFIG.as_bytes())
            .map_err(|source| ConfigError::Create {
                path: path.to_path_buf(),
                source,
            })?;

        Ok(())
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();

        let yaml = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;

        let config = Self::from_yaml(&yaml).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;

        if config.version != 1 {
            return Err(ConfigError::UnsupportedVersion {
                found: config.version,
            });
        }

        Ok(config)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        let path = path.as_ref();
        let temporary_path = path.with_extension("yaml.tmp");
        let yaml =
            serde_yaml::to_string(self).map_err(|source| ConfigError::Serialize { source })?;

        fs::write(&temporary_path, yaml).map_err(|source| ConfigError::Write {
            path: temporary_path.clone(),
            source,
        })?;
        fs::rename(&temporary_path, path).map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })?;

        Ok(())
    }

    pub fn add_workspace(
        &mut self,
        name: impl Into<String>,
        windows: Vec<WindowInfo>,
    ) -> Result<(), ConfigError> {
        let name = name.into();

        if name.trim().is_empty() {
            return Err(ConfigError::EmptyWorkspaceName);
        }

        if self.workspaces.contains_key(&name) {
            return Err(ConfigError::WorkspaceAlreadyExists { name });
        }

        self.workspaces.insert(
            name,
            Workspace {
                path: None,
                services: Vec::new(),
                urls: Vec::new(),
                windows,
            },
        );

        Ok(())
    }

    pub fn remove_workspace(&mut self, name: &str) -> Result<Workspace, ConfigError> {
        self.workspaces
            .remove(name)
            .ok_or_else(|| ConfigError::WorkspaceMissing {
                name: name.to_string(),
            })
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    pub fn workspace(&self, name: &str) -> Option<&Workspace> {
        self.workspaces.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BrowserTabState, RecoveryState};
    use tempfile::tempdir;

    const YAML: &str = r#"
version: 1

workspaces:
  devlayout:
    path: /Users/jay/git-work/devLayout
    services:
      - name: web
        run: pnpm dev
      - name: containers
        start: docker compose up -d
        stop: docker compose down
    urls:
      - http://localhost:3000
"#;

    #[test]
    fn parses_workspace_config() {
        let config = Config::from_yaml(YAML).unwrap();
        let workspace = config.workspace("devlayout").unwrap();

        assert_eq!(config.version, 1);
        assert!(config.ignored_windows.is_empty());
        assert_eq!(
            workspace.path,
            Some(PathBuf::from("/Users/jay/git-work/devLayout"))
        );
        assert_eq!(workspace.services.len(), 2);
        assert_eq!(workspace.urls, ["http://localhost:3000"]);
    }

    #[test]
    fn missing_services_and_urls_default_to_empty() {
        let config = Config::from_yaml(
            r#"
version: 1
workspaces:
  minimal:
    path: /tmp/minimal
"#,
        )
        .unwrap();

        let workspace = config.workspace("minimal").unwrap();

        assert!(workspace.services.is_empty());
        assert!(workspace.urls.is_empty());
        assert!(config.ignored_windows.is_empty());
    }

    #[test]
    fn old_window_yaml_loads_without_recovery_fields() {
        let config = Config::from_yaml(
            r#"
version: 1
workspaces:
  legacy:
    windows:
      - id: 42
        pid: 100
        owner: Visual Studio Code
        title: devLayout
"#,
        )
        .unwrap();

        let window = &config.workspace("legacy").unwrap().windows[0];
        assert_eq!(window.id, 42);
        assert!(window.bundle_id.is_none());
        assert!(window.application_path.is_none());
        assert!(window.recovery.is_none());
        assert!(window.recovery_warning.is_none());
        assert!(window.placement.is_none());
        assert!(window.placement_warning.is_none());
    }

    #[test]
    fn parses_ignored_windows() {
        let config = Config::from_yaml(
            r#"
version: 1
ignored_windows:
  - id: 42
    pid: 7
    owner: AltTab
    title: AltTab Pro
workspaces: {}
"#,
        )
        .unwrap();

        assert_eq!(config.ignored_windows.len(), 1);
        assert_eq!(config.ignored_windows[0].id, 42);
    }

    #[test]
    fn rejects_invalid_service_shape() {
        let result = Config::from_yaml(
            r#"
version: 1
workspaces:
  broken:
    path: /tmp/broken
    services:
      - name: web
"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn loads_config_from_file() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("workspaces.yaml");

        fs::write(&path, YAML).unwrap();

        let config = Config::load(&path).unwrap();

        assert!(config.workspace("devlayout").is_some());
    }

    #[test]
    fn reports_missing_config_file() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("missing.yaml");

        let error = Config::load(&path).unwrap_err();

        assert!(matches!(error, ConfigError::Read { .. }));
    }

    #[test]
    fn rejects_unsupported_config_version() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("workspaces.yaml");

        fs::write(
            &path,
            r#"
version: 2
workspaces: {}
"#,
        )
        .unwrap();

        let error = Config::load(&path).unwrap_err();

        assert!(matches!(
            error,
            ConfigError::UnsupportedVersion { found: 2 }
        ));
    }

    #[test]
    fn initializes_default_config() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("nested/workspaces.yaml");

        Config::init(&path).unwrap();

        let config = Config::load(&path).unwrap();
        assert_eq!(config.version, 1);
        assert!(config.workspaces.is_empty());
    }

    #[test]
    fn init_does_not_overwrite_existing_config() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("workspaces.yaml");
        fs::write(&path, YAML).unwrap();

        let error = Config::init(&path).unwrap_err();

        assert!(matches!(error, ConfigError::AlreadyExists { .. }));
        assert_eq!(fs::read_to_string(path).unwrap(), YAML);
    }

    #[test]
    fn saves_window_workspace_and_loads_it_again() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("workspaces.yaml");
        let mut config = Config::from_yaml("version: 1\nworkspaces: {}\n").unwrap();

        config
            .add_workspace(
                "coding",
                vec![WindowInfo {
                    id: 42,
                    pid: 100,
                    owner: "Visual Studio Code".to_string(),
                    title: Some("devLayout".to_string()),
                    bounds: None,
                    bundle_id: None,
                    application_path: None,
                    recovery: None,
                    recovery_warning: None,
                    placement: Some(crate::DesktopPlacement {
                        display_uuid: "Main".to_string(),
                        desktop_ordinal: 2,
                    }),
                    placement_warning: Some("using saved display mapping".to_string()),
                }],
            )
            .unwrap();
        config.save(&path).unwrap();

        let loaded = Config::load(path).unwrap();
        let workspace = loaded.workspace("coding").unwrap();

        assert_eq!(workspace.windows.len(), 1);
        assert_eq!(workspace.windows[0].id, 42);
    }

    #[test]
    fn recovery_state_round_trips_through_yaml() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("workspaces.yaml");
        let mut config = Config::from_yaml("version: 1\nworkspaces: {}\n").unwrap();

        config
            .add_workspace(
                "research",
                vec![WindowInfo {
                    id: 7,
                    pid: 200,
                    owner: "Firefox".to_string(),
                    title: Some("Ctx design".to_string()),
                    bounds: None,
                    bundle_id: Some("org.mozilla.firefox".to_string()),
                    application_path: Some(PathBuf::from("/Applications/Firefox.app")),
                    recovery: Some(RecoveryState::Browser {
                        tabs: vec![BrowserTabState {
                            url: "https://example.com/ctx".to_string(),
                            title: Some("Ctx".to_string()),
                        }],
                        active_tab: Some(0),
                    }),
                    recovery_warning: Some("one pinned tab was unavailable".to_string()),
                    placement: None,
                    placement_warning: None,
                }],
            )
            .unwrap();
        config.save(&path).unwrap();

        let loaded = Config::load(path).unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn failed_save_does_not_create_a_partial_config() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("missing/workspaces.yaml");
        let config = Config::from_yaml("version: 1\nworkspaces: {}\n").unwrap();

        let error = config.save(&path).unwrap_err();

        assert!(matches!(error, ConfigError::Write { .. }));
        assert!(!path.exists());
    }

    #[test]
    fn refuses_duplicate_workspace_name() {
        let mut config = Config::from_yaml("version: 1\nworkspaces: {}\n").unwrap();
        config.add_workspace("coding", Vec::new()).unwrap();

        let error = config.add_workspace("coding", Vec::new()).unwrap_err();

        assert!(matches!(error, ConfigError::WorkspaceAlreadyExists { .. }));
    }

    #[test]
    fn removes_workspace() {
        let mut config = Config::from_yaml("version: 1\nworkspaces: {}\n").unwrap();
        config.add_workspace("coding", Vec::new()).unwrap();

        config.remove_workspace("coding").unwrap();

        assert!(config.workspace("coding").is_none());
    }
}

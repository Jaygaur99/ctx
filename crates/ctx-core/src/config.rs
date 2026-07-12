use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
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
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Config {
    pub version: u32,
    pub workspaces: BTreeMap<String, Workspace>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Workspace {
    pub path: PathBuf,

    #[serde(default)]
    pub services: Vec<Service>,

    #[serde(default)]
    pub urls: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
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
        assert_eq!(
            workspace.path,
            PathBuf::from("/Users/jay/git-work/devLayout")
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
}

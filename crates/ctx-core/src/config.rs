use std::{collections::BTreeMap, path::PathBuf};

use serde::Deserialize;

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
}

use std::path::{Path, PathBuf};

use directories::BaseDirs;
use thiserror::Error;

#[derive(Debug, PartialEq, Eq)]
pub struct AppPaths {
    pub config_file: PathBuf,
    pub runtime_file: PathBuf,
    pub logs_dir: PathBuf,
}

#[derive(Debug, Error)]
pub enum PathsError {
    #[error("could not determine the current user's home directory")]
    HomeDirectoryUnavailable,
}

impl AppPaths {
    pub fn discover() -> Result<Self, PathsError> {
        let base_dirs = BaseDirs::new().ok_or(PathsError::HomeDirectoryUnavailable)?;

        Ok(Self::from_home(base_dirs.home_dir()))
    }

    fn from_home(home: &Path) -> Self {
        let support_dir = home.join("Library/Application Support/Ctx");

        Self {
            config_file: support_dir.join("config/workspaces.yaml"),
            runtime_file: support_dir.join("data/runtime.json"),
            logs_dir: home.join("Library/Logs/Ctx"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_standard_macos_paths() {
        let paths = AppPaths::from_home(Path::new("/Users/jay"));

        assert_eq!(
            paths.config_file,
            PathBuf::from("/Users/jay/Library/Application Support/Ctx/config/workspaces.yaml")
        );
        assert_eq!(
            paths.runtime_file,
            PathBuf::from("/Users/jay/Library/Application Support/Ctx/data/runtime.json")
        );
        assert_eq!(paths.logs_dir, PathBuf::from("/Users/jay/Library/Logs/Ctx"));
    }
}

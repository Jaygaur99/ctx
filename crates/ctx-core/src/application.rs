use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use crate::{
    AppPaths, Config, ConfigError, PathsError, RuntimeError, RuntimeState, Service, SwitchError,
    SwitchPersistenceError, SwitchReport, SystemUrlOpener, UrlError, UrlLaunchReport, UrlOpener,
    WindowError, WindowInfo, WindowStatus, WorkspaceUrlStatus, current_boot_id, inspect_windows,
    launch_workspace_urls, list_all_windows, list_windows, save_switch_transaction,
    switch_workspace, workspace_url_statuses,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CtxOverview {
    pub config_path: PathBuf,
    pub active_workspace: Option<String>,
    pub workspaces: Vec<WorkspaceOverview>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceOverview {
    pub name: String,
    pub active: bool,
    pub path: Option<PathBuf>,
    pub services: Vec<Service>,
    pub windows: Vec<WindowStatus>,
    pub urls: Vec<String>,
    pub url_statuses: Vec<WorkspaceUrlStatus>,
}

#[derive(Debug, Error)]
pub enum CtxAppError {
    #[error(transparent)]
    Paths(#[from] PathsError),

    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Runtime(#[from] RuntimeError),

    #[error(transparent)]
    Window(#[from] WindowError),

    #[error(transparent)]
    Url(#[from] UrlError),

    #[error(transparent)]
    Switch(#[from] SwitchError),

    #[error(transparent)]
    Persistence(#[from] SwitchPersistenceError),

    #[error("workspace '{name}' does not exist")]
    WorkspaceMissing { name: String },
}

impl CtxAppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Paths(_) => "paths",
            Self::Config(_) => "config",
            Self::Runtime(_) => "runtime",
            Self::Window(WindowError::ScreenRecordingPermissionRequired) => "permission",
            Self::Window(_) => "window_discovery",
            Self::Url(_) => "url",
            Self::Switch(SwitchError::Accessibility(
                crate::AccessibilityError::PermissionRequired,
            )) => "permission",
            Self::Switch(_) => "switch",
            Self::Persistence(_) => "persistence",
            Self::WorkspaceMissing { .. } => "workspace_missing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CtxApp {
    config_path: PathBuf,
    runtime_path: PathBuf,
}

impl CtxApp {
    pub fn discover(config_override: Option<PathBuf>) -> Result<Self, CtxAppError> {
        let paths = AppPaths::discover()?;
        Ok(Self {
            config_path: config_override.unwrap_or(paths.config_file),
            runtime_path: paths.runtime_file,
        })
    }

    pub fn from_paths(config_path: impl Into<PathBuf>, runtime_path: impl Into<PathBuf>) -> Self {
        Self {
            config_path: config_path.into(),
            runtime_path: runtime_path.into(),
        }
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn runtime_path(&self) -> &Path {
        &self.runtime_path
    }

    pub fn overview(&self) -> Result<CtxOverview, CtxAppError> {
        let config = Config::load(&self.config_path)?;
        let state = RuntimeState::load(&self.runtime_path)?;
        let boot_id = current_boot_id()?;
        let all_windows = list_all_windows()?;
        let visible_windows = list_windows()?;

        Ok(build_overview(
            &self.config_path,
            &config,
            &state,
            &boot_id,
            &all_windows,
            &visible_windows,
        ))
    }

    pub fn active_workspace(&self) -> Result<Option<String>, CtxAppError> {
        Ok(RuntimeState::load(&self.runtime_path)?.active_workspace)
    }

    pub fn switch_workspace(&self, name: &str) -> Result<SwitchReport, CtxAppError> {
        self.switch_workspace_with(name, |config, state, name| {
            Ok(switch_workspace(config, state, name)?)
        })
    }

    pub fn open_workspace_urls(
        &self,
        name: &str,
        force: bool,
    ) -> Result<UrlLaunchReport, CtxAppError> {
        let boot_id = current_boot_id()?;
        let mut opener = SystemUrlOpener;
        self.open_workspace_urls_with(name, force, &boot_id, &mut opener)
    }

    fn switch_workspace_with<F>(
        &self,
        name: &str,
        operation: F,
    ) -> Result<SwitchReport, CtxAppError>
    where
        F: FnOnce(&mut Config, &mut RuntimeState, &str) -> Result<SwitchReport, CtxAppError>,
    {
        let mut config = Config::load(&self.config_path)?;
        let mut state = RuntimeState::load(&self.runtime_path)?;
        let report = operation(&mut config, &mut state, name)?;
        save_switch_transaction(&config, &self.config_path, &state, &self.runtime_path)?;
        Ok(report)
    }

    fn open_workspace_urls_with(
        &self,
        name: &str,
        force: bool,
        boot_id: &str,
        opener: &mut dyn UrlOpener,
    ) -> Result<UrlLaunchReport, CtxAppError> {
        let config = Config::load(&self.config_path)?;
        let mut state = RuntimeState::load(&self.runtime_path)?;
        let workspace = config
            .workspace(name)
            .ok_or_else(|| CtxAppError::WorkspaceMissing {
                name: name.to_string(),
            })?;
        let report = launch_workspace_urls(name, workspace, &mut state, boot_id, force, opener);
        state.save(&self.runtime_path)?;
        Ok(report)
    }
}

fn build_overview(
    config_path: &Path,
    config: &Config,
    state: &RuntimeState,
    boot_id: &str,
    all_windows: &[WindowInfo],
    visible_windows: &[WindowInfo],
) -> CtxOverview {
    let workspaces = config
        .workspaces
        .iter()
        .map(|(name, workspace)| WorkspaceOverview {
            name: name.clone(),
            active: state.active_workspace.as_deref() == Some(name),
            path: workspace.path.clone(),
            services: workspace.services.clone(),
            windows: inspect_windows(&workspace.windows, all_windows, visible_windows),
            urls: workspace.urls.clone(),
            url_statuses: workspace_url_statuses(name, workspace, state, boot_id),
        })
        .collect();

    CtxOverview {
        config_path: config_path.to_path_buf(),
        active_workspace: state.active_workspace.clone(),
        workspaces,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tempfile::tempdir;

    use super::*;
    use crate::{
        DesktopPlacement, RecoveryState, UrlSessionState, WindowBounds, WindowState, Workspace,
        WorkspaceUrlState,
    };

    struct RecordingOpener {
        opened: Vec<String>,
        fail: bool,
    }

    impl UrlOpener for RecordingOpener {
        fn open(&mut self, url: &str) -> Result<(), UrlError> {
            self.opened.push(url.to_string());
            if self.fail {
                Err(UrlError::Open {
                    url: url.to_string(),
                    message: "test failure".to_string(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn overview_combines_runtime_window_recovery_placement_and_url_state() {
        let saved = WindowInfo {
            id: 42,
            pid: 7,
            owner: "Code".to_string(),
            title: Some("Ctx".to_string()),
            bounds: Some(WindowBounds {
                x: 1,
                y: 2,
                width: 800,
                height: 600,
            }),
            bundle_id: Some("com.microsoft.VSCode".to_string()),
            application_path: None,
            recovery: Some(RecoveryState::Editor {
                project_path: PathBuf::from("/tmp/ctx"),
            }),
            recovery_warning: Some("degraded".to_string()),
            placement: Some(DesktopPlacement {
                display_uuid: "main".to_string(),
                desktop_ordinal: 2,
            }),
            placement_warning: None,
        };
        let mut workspaces = BTreeMap::new();
        workspaces.insert(
            "coding".to_string(),
            Workspace {
                path: Some(PathBuf::from("/tmp/ctx")),
                services: Vec::new(),
                urls: vec!["https://example.com/".to_string()],
                windows: vec![saved.clone()],
            },
        );
        let config = Config {
            version: 1,
            ignored_windows: Vec::new(),
            workspaces,
        };
        let mut state = RuntimeState {
            version: 1,
            active_workspace: Some("coding".to_string()),
            url_session: UrlSessionState::default(),
        };
        state.ensure_url_boot_session("boot");
        state.mark_url_opened("coding", "https://example.com/");

        let overview = build_overview(
            Path::new("/tmp/config.yaml"),
            &config,
            &state,
            "boot",
            std::slice::from_ref(&saved),
            std::slice::from_ref(&saved),
        );

        assert_eq!(overview.active_workspace.as_deref(), Some("coding"));
        let workspace = &overview.workspaces[0];
        assert!(workspace.active);
        assert_eq!(workspace.windows[0].state, WindowState::Visible);
        assert!(workspace.windows[0].recovery_ready);
        assert!(workspace.windows[0].recovery_degraded);
        assert_eq!(
            workspace.windows[0]
                .placement
                .as_ref()
                .unwrap()
                .desktop_ordinal,
            2
        );
        assert_eq!(workspace.url_statuses[0].state, WorkspaceUrlState::Opened);
    }

    #[test]
    fn switch_facade_persists_config_and_runtime_together() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("workspaces.yaml");
        let runtime_path = directory.path().join("runtime.json");
        Config::from_yaml("version: 1\nworkspaces:\n  coding: {}\n")
            .unwrap()
            .save(&config_path)
            .unwrap();
        let app = CtxApp::from_paths(&config_path, &runtime_path);

        app.switch_workspace_with("coding", |_, state, name| {
            state.active_workspace = Some(name.to_string());
            Ok(SwitchReport::default())
        })
        .unwrap();

        assert_eq!(
            RuntimeState::load(runtime_path)
                .unwrap()
                .active_workspace
                .as_deref(),
            Some("coding")
        );
    }

    #[test]
    fn overview_reports_a_stale_active_workspace_without_mutating_state() {
        let config = Config::from_yaml("version: 1\nworkspaces: {}\n").unwrap();
        let state = RuntimeState {
            version: 1,
            active_workspace: Some("removed".to_string()),
            url_session: UrlSessionState::default(),
        };

        let overview = build_overview(
            Path::new("/tmp/config.yaml"),
            &config,
            &state,
            "boot",
            &[],
            &[],
        );

        assert_eq!(overview.active_workspace.as_deref(), Some("removed"));
        assert!(overview.workspaces.is_empty());
    }

    #[test]
    fn explicit_paths_are_exposed_for_cli_and_ui_adapters() {
        let app = CtxApp::from_paths("/tmp/config.yaml", "/tmp/runtime.json");

        assert_eq!(app.config_path(), Path::new("/tmp/config.yaml"));
        assert_eq!(app.runtime_path(), Path::new("/tmp/runtime.json"));
    }

    #[test]
    fn forced_url_open_persists_successes_and_partial_failures() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("workspaces.yaml");
        let runtime_path = directory.path().join("runtime.json");
        Config::from_yaml(
            "version: 1\nworkspaces:\n  coding:\n    urls:\n      - https://example.com\n",
        )
        .unwrap()
        .save(&config_path)
        .unwrap();
        let app = CtxApp::from_paths(&config_path, &runtime_path);
        let mut success = RecordingOpener {
            opened: Vec::new(),
            fail: false,
        };

        app.open_workspace_urls_with("coding", true, "boot", &mut success)
            .unwrap();
        app.open_workspace_urls_with("coding", true, "boot", &mut success)
            .unwrap();
        assert_eq!(success.opened.len(), 2);

        let mut failure = RecordingOpener {
            opened: Vec::new(),
            fail: true,
        };
        let report = app
            .open_workspace_urls_with("coding", true, "boot", &mut failure)
            .unwrap();
        assert_eq!(report.failed.len(), 1);
        assert!(
            RuntimeState::load(runtime_path)
                .unwrap()
                .url_failure("boot", "coding", "https://example.com/")
                .is_some()
        );
    }
}

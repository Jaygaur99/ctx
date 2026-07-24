use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use serde::Serialize;
use thiserror::Error;

use crate::{
    AppPaths, Config, ConfigError, DEFAULT_MUTATION_LOCK_TIMEOUT, DesktopPlacement, MutationGuard,
    MutationLockError, PathsError, RuntimeError, RuntimeState, Service, SpaceError, SwitchError,
    SwitchPersistenceError, SwitchReport, SystemUrlOpener, UrlError, UrlLaunchReport, UrlOpener,
    WindowBounds, WindowError, WindowInfo, WindowResolution, WindowStatus, WorkspaceUrlStatus,
    acquire_mutation_lock, capture_desktop_placement, current_boot_id, inspect_windows,
    launch_workspace_urls, list_all_windows, list_windows, normalize_url, normalize_urls,
    resolve_window, save_switch_transaction, switch_workspace, workspace_url_statuses,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WindowCandidate {
    pub id: u32,
    pub pid: i32,
    pub application: String,
    pub title: Option<String>,
    pub bounds: Option<WindowBounds>,
    pub assigned_to: Vec<String>,
    pub already_in_workspace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WindowPickerOverview {
    pub workspace: String,
    pub windows: Vec<WindowCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AddWindowsReport {
    pub workspace: String,
    pub added: Vec<WindowInfo>,
    pub already_tracked: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateWorkspaceReport {
    pub workspace: String,
    pub config_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeleteWorkspacesReport {
    pub deleted: Vec<String>,
    pub active_workspace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EditWorkspaceReport {
    pub previous_name: String,
    pub workspace: String,
    pub urls: Vec<String>,
    pub removed_windows: Vec<u32>,
    pub already_absent_windows: Vec<u32>,
    pub active_workspace: Option<String>,
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

    #[error(transparent)]
    Mutation(#[from] MutationLockError),

    #[error("workspace '{name}' does not exist")]
    WorkspaceMissing { name: String },

    #[error("select at least one window")]
    NoWindowsSelected,

    #[error("window {id} is not selectable; refresh the window picker and try again")]
    WindowNotSelectable { id: u32 },
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
            Self::Mutation(MutationLockError::Busy { .. }) => "busy",
            Self::Mutation(_) => "mutation_lock",
            Self::WorkspaceMissing { .. } => "workspace_missing",
            Self::NoWindowsSelected => "no_windows_selected",
            Self::WindowNotSelectable { .. } => "window_not_selectable",
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

    pub fn lock_mutations(&self) -> Result<MutationGuard, CtxAppError> {
        Ok(acquire_mutation_lock(
            &self.config_path,
            DEFAULT_MUTATION_LOCK_TIMEOUT,
        )?)
    }

    pub fn switch_workspace(&self, name: &str) -> Result<SwitchReport, CtxAppError> {
        let _guard = self.lock_mutations()?;
        self.switch_workspace_with(name, |config, state, name| {
            Ok(switch_workspace(config, state, name)?)
        })
    }

    pub fn open_workspace_urls(
        &self,
        name: &str,
        force: bool,
    ) -> Result<UrlLaunchReport, CtxAppError> {
        let _guard = self.lock_mutations()?;
        let boot_id = current_boot_id()?;
        let mut opener = SystemUrlOpener;
        self.open_workspace_urls_with(name, force, &boot_id, &mut opener)
    }

    pub fn window_candidates(&self, workspace: &str) -> Result<WindowPickerOverview, CtxAppError> {
        let config = Config::load(&self.config_path)?;
        let windows = list_all_windows()?;
        build_window_picker(
            &config,
            workspace,
            &windows,
            Some(std::process::id() as i32),
        )
    }

    pub fn add_windows_to_workspace(
        &self,
        workspace: &str,
        window_ids: &[u32],
    ) -> Result<AddWindowsReport, CtxAppError> {
        if window_ids.is_empty() {
            return Err(CtxAppError::NoWindowsSelected);
        }
        let _guard = self.lock_mutations()?;
        let windows = list_all_windows()?;
        self.add_windows_to_workspace_with(workspace, window_ids, &windows, |id| {
            capture_desktop_placement(id)
        })
    }

    pub fn create_workspace(&self, name: &str) -> Result<CreateWorkspaceReport, CtxAppError> {
        let _guard = self.lock_mutations()?;
        let mut config = Config::load(&self.config_path)?;
        let state = RuntimeState::load(&self.runtime_path)?;
        config.add_workspace(name, Vec::new())?;
        save_switch_transaction(&config, &self.config_path, &state, &self.runtime_path)?;
        Ok(CreateWorkspaceReport {
            workspace: name.to_string(),
            config_path: self.config_path.clone(),
        })
    }

    pub fn delete_workspace(&self, name: &str) -> Result<DeleteWorkspacesReport, CtxAppError> {
        let _guard = self.lock_mutations()?;
        let mut config = Config::load(&self.config_path)?;
        let mut state = RuntimeState::load(&self.runtime_path)?;
        config.remove_workspace(name)?;
        if state.active_workspace.as_deref() == Some(name) {
            state.active_workspace = None;
        }
        state.clear_workspace_urls(name);
        save_switch_transaction(&config, &self.config_path, &state, &self.runtime_path)?;
        Ok(DeleteWorkspacesReport {
            deleted: vec![name.to_string()],
            active_workspace: state.active_workspace,
        })
    }

    pub fn delete_all_workspaces(&self) -> Result<DeleteWorkspacesReport, CtxAppError> {
        let _guard = self.lock_mutations()?;
        let mut config = Config::load(&self.config_path)?;
        let mut state = RuntimeState::load(&self.runtime_path)?;
        let deleted: Vec<_> = config.workspaces.keys().cloned().collect();
        config.workspaces.clear();
        state.active_workspace = None;
        for name in &deleted {
            state.clear_workspace_urls(name);
        }
        save_switch_transaction(&config, &self.config_path, &state, &self.runtime_path)?;
        Ok(DeleteWorkspacesReport {
            deleted,
            active_workspace: None,
        })
    }

    pub fn edit_workspace(
        &self,
        name: &str,
        new_name: &str,
        urls: &[String],
        remove_window_ids: &[u32],
    ) -> Result<EditWorkspaceReport, CtxAppError> {
        let _guard = self.lock_mutations()?;
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return Err(ConfigError::EmptyWorkspaceName.into());
        }

        let normalized_urls = normalize_urls(urls)?;
        let mut config = Config::load(&self.config_path)?;
        let mut state = RuntimeState::load(&self.runtime_path)?;
        if !config.workspaces.contains_key(name) {
            return Err(CtxAppError::WorkspaceMissing {
                name: name.to_string(),
            });
        }
        if name != new_name && config.workspaces.contains_key(new_name) {
            return Err(ConfigError::WorkspaceAlreadyExists {
                name: new_name.to_string(),
            }
            .into());
        }
        let mut workspace = config
            .workspaces
            .remove(name)
            .expect("workspace existence was checked above");

        let requested: BTreeSet<u32> = remove_window_ids.iter().copied().collect();
        let mut removed_windows = Vec::new();
        workspace.windows.retain(|window| {
            if requested.contains(&window.id) {
                removed_windows.push(window.id);
                false
            } else {
                true
            }
        });
        let removed_set: BTreeSet<_> = removed_windows.iter().copied().collect();
        let already_absent_windows = requested
            .difference(&removed_set)
            .copied()
            .collect::<Vec<_>>();
        let previous_urls: BTreeSet<_> = workspace
            .urls
            .iter()
            .filter_map(|url| normalize_url(url).ok())
            .collect();
        let retained_urls: BTreeSet<_> = normalized_urls.iter().cloned().collect();
        workspace.urls = normalized_urls.clone();
        config.workspaces.insert(new_name.to_string(), workspace);

        state.rename_workspace(name, new_name);
        for removed_url in previous_urls.difference(&retained_urls) {
            state.clear_workspace_url(new_name, removed_url);
        }
        save_switch_transaction(&config, &self.config_path, &state, &self.runtime_path)?;

        Ok(EditWorkspaceReport {
            previous_name: name.to_string(),
            workspace: new_name.to_string(),
            urls: normalized_urls,
            removed_windows,
            already_absent_windows,
            active_workspace: state.active_workspace,
        })
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

    fn add_windows_to_workspace_with<F>(
        &self,
        workspace: &str,
        window_ids: &[u32],
        current_windows: &[WindowInfo],
        mut capture_placement: F,
    ) -> Result<AddWindowsReport, CtxAppError>
    where
        F: FnMut(u32) -> Result<DesktopPlacement, SpaceError>,
    {
        if window_ids.is_empty() {
            return Err(CtxAppError::NoWindowsSelected);
        }

        let mut config = Config::load(&self.config_path)?;
        let existing = config
            .workspace(workspace)
            .ok_or_else(|| CtxAppError::WorkspaceMissing {
                name: workspace.to_string(),
            })?
            .windows
            .clone();
        let available: BTreeMap<_, _> = current_windows
            .iter()
            .map(|window| (window.id, window))
            .collect();
        let mut seen = BTreeSet::new();
        let mut added = Vec::new();
        let mut already_tracked = Vec::new();

        for id in window_ids.iter().copied().filter(|id| seen.insert(*id)) {
            let current = available
                .get(&id)
                .ok_or(CtxAppError::WindowNotSelectable { id })?;
            let tracked = existing.iter().chain(added.iter()).any(|saved| {
                matches!(
                    resolve_window(saved, current_windows),
                    WindowResolution::Resolved(resolved) if resolved.id == id
                )
            });
            if tracked {
                already_tracked.push(id);
                continue;
            }

            let mut selected = (*current).clone();
            match capture_placement(selected.id) {
                Ok(placement) => selected.placement = Some(placement),
                Err(error) => {
                    selected.placement_warning =
                        Some(format!("Desktop placement capture failed: {error}"));
                }
            }
            added.push(selected);
        }

        if !added.is_empty() {
            config
                .workspaces
                .get_mut(workspace)
                .expect("workspace existence was checked above")
                .windows
                .extend(added.iter().cloned());
            config.save(&self.config_path)?;
        }

        Ok(AddWindowsReport {
            workspace: workspace.to_string(),
            added,
            already_tracked,
        })
    }
}

fn build_window_picker(
    config: &Config,
    workspace: &str,
    current_windows: &[WindowInfo],
    excluded_pid: Option<i32>,
) -> Result<WindowPickerOverview, CtxAppError> {
    if config.workspace(workspace).is_none() {
        return Err(CtxAppError::WorkspaceMissing {
            name: workspace.to_string(),
        });
    }

    let assignments = assignment_map(config, current_windows);
    let mut windows: Vec<_> = current_windows
        .iter()
        .filter(|window| excluded_pid != Some(window.pid))
        .map(|window| {
            let assigned_to = assignments.get(&window.id).cloned().unwrap_or_default();
            WindowCandidate {
                id: window.id,
                pid: window.pid,
                application: window.owner.clone(),
                title: window.title.clone(),
                bounds: window.bounds,
                already_in_workspace: assigned_to.iter().any(|name| name == workspace),
                assigned_to,
            }
        })
        .collect();
    windows.sort_by(|first, second| {
        first
            .application
            .to_lowercase()
            .cmp(&second.application.to_lowercase())
            .then_with(|| first.title.cmp(&second.title))
            .then_with(|| first.id.cmp(&second.id))
    });

    Ok(WindowPickerOverview {
        workspace: workspace.to_string(),
        windows,
    })
}

fn assignment_map(config: &Config, current_windows: &[WindowInfo]) -> BTreeMap<u32, Vec<String>> {
    let mut assignments: BTreeMap<u32, Vec<String>> = BTreeMap::new();

    for (name, workspace) in &config.workspaces {
        for saved in &workspace.windows {
            if let WindowResolution::Resolved(current) = resolve_window(saved, current_windows) {
                assignments
                    .entry(current.id)
                    .or_default()
                    .push(name.clone());
            }
        }
    }

    for saved in &config.ignored_windows {
        if let WindowResolution::Resolved(current) = resolve_window(saved, current_windows) {
            assignments
                .entry(current.id)
                .or_default()
                .push("<ignored>".to_string());
        }
    }

    assignments
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

    fn test_window(id: u32, pid: i32, application: &str, title: &str) -> WindowInfo {
        WindowInfo {
            id,
            pid,
            owner: application.to_string(),
            title: Some(title.to_string()),
            bounds: Some(WindowBounds {
                x: 10,
                y: 20,
                width: 900,
                height: 700,
            }),
            bundle_id: Some(format!("com.example.{}", application.to_lowercase())),
            application_path: None,
            recovery: None,
            recovery_warning: None,
            placement: None,
            placement_warning: None,
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

    #[test]
    fn window_picker_reports_assignments_and_excludes_the_calling_process() {
        let tracked = test_window(10, 100, "Code", "Ctx");
        let own_panel = test_window(20, 200, "Ctx", "Add windows");
        let mut workspaces = BTreeMap::new();
        workspaces.insert(
            "coding".to_string(),
            Workspace {
                path: None,
                services: Vec::new(),
                urls: Vec::new(),
                windows: vec![tracked.clone()],
            },
        );
        let config = Config {
            version: 1,
            ignored_windows: Vec::new(),
            workspaces,
        };

        let picker = build_window_picker(&config, "coding", &[tracked], Some(200)).unwrap();
        assert_eq!(picker.windows.len(), 1);
        assert_eq!(picker.windows[0].assigned_to, vec!["coding"]);
        assert!(picker.windows[0].already_in_workspace);

        let picker = build_window_picker(&config, "coding", &[own_panel], Some(200)).unwrap();
        assert!(picker.windows.is_empty());
    }

    #[test]
    fn adding_windows_captures_placement_and_persists_idempotently() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("workspaces.yaml");
        let runtime_path = directory.path().join("runtime.json");
        Config::from_yaml("version: 1\nworkspaces:\n  coding: {}\n")
            .unwrap()
            .save(&config_path)
            .unwrap();
        let app = CtxApp::from_paths(&config_path, &runtime_path);
        let window = test_window(42, 7, "Code", "Ctx");

        let first = app
            .add_windows_to_workspace_with(
                "coding",
                &[42, 42],
                std::slice::from_ref(&window),
                |_| {
                    Ok(DesktopPlacement {
                        display_uuid: "main".to_string(),
                        desktop_ordinal: 2,
                    })
                },
            )
            .unwrap();
        assert_eq!(first.added.len(), 1);
        assert_eq!(
            first.added[0].placement.as_ref().unwrap().desktop_ordinal,
            2
        );

        let second = app
            .add_windows_to_workspace_with(
                "coding",
                &[42],
                std::slice::from_ref(&window),
                |_| unreachable!(),
            )
            .unwrap();
        assert!(second.added.is_empty());
        assert_eq!(second.already_tracked, vec![42]);
        assert_eq!(
            Config::load(config_path)
                .unwrap()
                .workspace("coding")
                .unwrap()
                .windows
                .len(),
            1
        );
    }

    #[test]
    fn stale_picker_selection_does_not_partially_update_the_workspace() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("workspaces.yaml");
        Config::from_yaml("version: 1\nworkspaces:\n  coding: {}\n")
            .unwrap()
            .save(&config_path)
            .unwrap();
        let app = CtxApp::from_paths(&config_path, directory.path().join("runtime.json"));
        let window = test_window(42, 7, "Code", "Ctx");

        let error = app
            .add_windows_to_workspace_with("coding", &[42, 99], &[window], |_| {
                Ok(DesktopPlacement {
                    display_uuid: "main".to_string(),
                    desktop_ordinal: 1,
                })
            })
            .unwrap_err();
        assert!(matches!(error, CtxAppError::WindowNotSelectable { id: 99 }));
        assert!(
            Config::load(config_path)
                .unwrap()
                .workspace("coding")
                .unwrap()
                .windows
                .is_empty()
        );
    }

    #[test]
    fn create_and_delete_workspace_mutations_persist_runtime_cleanup() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("workspaces.yaml");
        let runtime_path = directory.path().join("runtime.json");
        Config::from_yaml("version: 1\nworkspaces: {}\n")
            .unwrap()
            .save(&config_path)
            .unwrap();
        let app = CtxApp::from_paths(&config_path, &runtime_path);

        app.create_workspace("coding").unwrap();
        assert!(
            Config::load(&config_path)
                .unwrap()
                .workspace("coding")
                .is_some()
        );

        let mut state = RuntimeState {
            active_workspace: Some("coding".to_string()),
            ..RuntimeState::default()
        };
        state.ensure_url_boot_session("boot");
        state.mark_url_opened("coding", "https://example.com/");
        state.save(&runtime_path).unwrap();

        let report = app.delete_workspace("coding").unwrap();
        assert_eq!(report.deleted, vec!["coding"]);
        assert!(report.active_workspace.is_none());
        assert!(Config::load(&config_path).unwrap().workspaces.is_empty());
        let state = RuntimeState::load(&runtime_path).unwrap();
        assert!(state.active_workspace.is_none());
        assert!(state.url_session.opened.is_empty());
    }

    #[test]
    fn delete_all_workspaces_clears_every_definition_and_runtime_marker() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("workspaces.yaml");
        let runtime_path = directory.path().join("runtime.json");
        Config::from_yaml("version: 1\nworkspaces:\n  coding: {}\n  research: {}\n")
            .unwrap()
            .save(&config_path)
            .unwrap();
        let mut state = RuntimeState {
            active_workspace: Some("research".to_string()),
            ..RuntimeState::default()
        };
        state.ensure_url_boot_session("boot");
        state.mark_url_failed("research", "https://example.com/", "offline");
        state.save(&runtime_path).unwrap();
        let app = CtxApp::from_paths(&config_path, &runtime_path);

        let report = app.delete_all_workspaces().unwrap();

        assert_eq!(report.deleted, vec!["coding", "research"]);
        assert!(Config::load(config_path).unwrap().workspaces.is_empty());
        let state = RuntimeState::load(runtime_path).unwrap();
        assert!(state.active_workspace.is_none());
        assert!(state.url_session.failures.is_empty());
    }

    #[test]
    fn editing_workspace_renames_runtime_and_replaces_windows_and_urls_transactionally() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("workspaces.yaml");
        let runtime_path = directory.path().join("runtime.json");
        let mut config = Config::from_yaml(
            "version: 1\nworkspaces:\n  coding:\n    urls:\n      - https://one.example\n      - https://remove.example\n",
        )
        .unwrap();
        config
            .workspaces
            .get_mut("coding")
            .unwrap()
            .windows
            .push(test_window(42, 7, "Code", "Ctx"));
        config.save(&config_path).unwrap();
        let mut state = RuntimeState {
            active_workspace: Some("coding".to_string()),
            ..RuntimeState::default()
        };
        state.ensure_url_boot_session("boot");
        state.mark_url_opened("coding", "https://one.example/");
        state.mark_url_failed("coding", "https://remove.example/", "offline");
        state.save(&runtime_path).unwrap();
        let app = CtxApp::from_paths(&config_path, &runtime_path);

        let report = app
            .edit_workspace(
                "coding",
                "deep-work",
                &[
                    " https://new.example ".to_string(),
                    "https://one.example".to_string(),
                    "https://new.example/".to_string(),
                ],
                &[42, 99],
            )
            .unwrap();

        assert_eq!(report.removed_windows, vec![42]);
        assert_eq!(report.already_absent_windows, vec![99]);
        assert_eq!(
            report.urls,
            ["https://new.example/", "https://one.example/"]
        );
        let config = Config::load(&config_path).unwrap();
        assert!(config.workspace("coding").is_none());
        let workspace = config.workspace("deep-work").unwrap();
        assert!(workspace.windows.is_empty());
        assert_eq!(
            workspace.urls,
            ["https://new.example/", "https://one.example/"]
        );
        let state = RuntimeState::load(&runtime_path).unwrap();
        assert_eq!(state.active_workspace.as_deref(), Some("deep-work"));
        assert!(state.url_was_opened("boot", "deep-work", "https://one.example/"));
        assert!(
            state
                .url_failure("boot", "deep-work", "https://remove.example/")
                .is_none()
        );

        let repeated = app
            .edit_workspace("deep-work", "deep-work", &workspace.urls, &[42])
            .unwrap();
        assert!(repeated.removed_windows.is_empty());
        assert_eq!(repeated.already_absent_windows, vec![42]);
    }

    #[test]
    fn editing_workspace_rejects_missing_sources_before_rename_collisions() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("workspaces.yaml");
        let runtime_path = directory.path().join("runtime.json");
        Config::from_yaml("version: 1\nworkspaces:\n  research: {}\n")
            .unwrap()
            .save(&config_path)
            .unwrap();
        let app = CtxApp::from_paths(&config_path, &runtime_path);

        let error = app
            .edit_workspace("missing", "research", &[], &[])
            .unwrap_err();

        assert!(matches!(
            error,
            CtxAppError::WorkspaceMissing { name } if name == "missing"
        ));
    }

    #[test]
    fn editing_workspace_validation_failures_do_not_change_persisted_files() {
        let directory = tempdir().unwrap();
        let config_path = directory.path().join("workspaces.yaml");
        let runtime_path = directory.path().join("runtime.json");
        Config::from_yaml("version: 1\nworkspaces:\n  coding: {}\n  research: {}\n")
            .unwrap()
            .save(&config_path)
            .unwrap();
        RuntimeState {
            active_workspace: Some("coding".to_string()),
            ..RuntimeState::default()
        }
        .save(&runtime_path)
        .unwrap();
        let config_before = std::fs::read(&config_path).unwrap();
        let runtime_before = std::fs::read(&runtime_path).unwrap();
        let app = CtxApp::from_paths(&config_path, &runtime_path);

        let collision = app
            .edit_workspace("coding", "research", &[], &[])
            .unwrap_err();
        assert!(matches!(
            collision,
            CtxAppError::Config(ConfigError::WorkspaceAlreadyExists { .. })
        ));

        let invalid_url = app
            .edit_workspace(
                "coding",
                "deep-work",
                &["file:///tmp/not-allowed".to_string()],
                &[],
            )
            .unwrap_err();
        assert!(matches!(
            invalid_url,
            CtxAppError::Url(UrlError::UnsupportedScheme { .. })
        ));

        assert_eq!(std::fs::read(&config_path).unwrap(), config_before);
        assert_eq!(std::fs::read(&runtime_path).unwrap(), runtime_before);
    }
}

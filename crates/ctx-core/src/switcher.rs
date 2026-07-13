use std::{collections::BTreeSet, thread, time::Duration};

use thiserror::Error;

use crate::{
    AccessibilityError, Config, GenericAppAdapter, RecoveryAdapter, RecoveryError,
    RecoveryRegistry, RecoveryState, RuntimeState, WindowInfo, WindowResolution, WindowState,
    default_recovery_registry, list_all_windows, minimize_windows, minimize_windows_best_effort,
    reconcile_windows, resolve_window, restore_windows, windows::refresh_window_fingerprint,
};

const RECOVERY_POLL_INTERVAL: Duration = Duration::from_millis(250);
const RECOVERY_POLL_ATTEMPTS: usize = 80;

#[derive(Debug, Error)]
pub enum SwitchError {
    #[error("workspace '{name}' does not exist")]
    WorkspaceMissing { name: String },

    #[error("active workspace '{name}' no longer exists in the configuration")]
    ActiveWorkspaceMissing { name: String },

    #[error(transparent)]
    Accessibility(#[from] AccessibilityError),

    #[error(transparent)]
    Discovery(#[from] crate::WindowError),

    #[error("workspace '{workspace}' window {id} is {state:?}")]
    WindowUnavailable {
        workspace: String,
        id: u32,
        state: WindowState,
    },

    #[error("workspace '{workspace}' window {id} has no usable recovery adapter")]
    RecoveryUnavailable { workspace: String, id: u32 },

    #[error("failed to recover workspace '{workspace}' window {id}: {source}")]
    RecoveryFailed {
        workspace: String,
        id: u32,
        #[source]
        source: RecoveryError,
    },

    #[error("workspace '{workspace}' recovery timed out for windows {ids:?}")]
    RecoveryTimedOut { workspace: String, ids: Vec<u32> },
}

trait SwitchPlatform {
    fn list_windows(&mut self) -> Result<Vec<WindowInfo>, SwitchError>;
    fn minimize(&mut self, windows: &[WindowInfo]) -> Result<(), SwitchError>;
    fn minimize_best_effort(&mut self, windows: &[WindowInfo]);
    fn restore(&mut self, windows: &[WindowInfo]) -> Result<(), SwitchError>;
    fn wait(&mut self, duration: Duration);
}

struct MacOsSwitchPlatform;

impl SwitchPlatform for MacOsSwitchPlatform {
    fn list_windows(&mut self) -> Result<Vec<WindowInfo>, SwitchError> {
        Ok(list_all_windows()?)
    }

    fn minimize(&mut self, windows: &[WindowInfo]) -> Result<(), SwitchError> {
        if windows.is_empty() {
            return Ok(());
        }
        minimize_windows(windows)?;
        Ok(())
    }

    fn minimize_best_effort(&mut self, windows: &[WindowInfo]) {
        if !windows.is_empty() {
            let _ = minimize_windows_best_effort(windows);
        }
    }

    fn restore(&mut self, windows: &[WindowInfo]) -> Result<(), SwitchError> {
        if windows.is_empty() {
            return Ok(());
        }
        restore_windows(windows)?;
        Ok(())
    }

    fn wait(&mut self, duration: Duration) {
        thread::sleep(duration);
    }
}

pub fn switch_workspace(
    config: &mut Config,
    state: &mut RuntimeState,
    target_name: &str,
) -> Result<(), SwitchError> {
    switch_workspace_with(
        config,
        state,
        target_name,
        &default_recovery_registry(),
        &GenericAppAdapter,
        &mut MacOsSwitchPlatform,
    )
}

fn switch_workspace_with(
    config: &mut Config,
    state: &mut RuntimeState,
    target_name: &str,
    registry: &RecoveryRegistry,
    generic: &dyn RecoveryAdapter,
    platform: &mut dyn SwitchPlatform,
) -> Result<(), SwitchError> {
    if !config.workspaces.contains_key(target_name) {
        return Err(SwitchError::WorkspaceMissing {
            name: target_name.to_string(),
        });
    }
    if let Some(active) = state.active_workspace.as_deref()
        && !config.workspaces.contains_key(active)
    {
        return Err(SwitchError::ActiveWorkspaceMissing {
            name: active.to_string(),
        });
    }

    let before = platform.list_windows()?;
    for workspace in config.workspaces.values_mut() {
        reconcile_windows(&mut workspace.windows, &before);
    }

    let previous_name = state.active_workspace.clone();
    let previous_windows = previous_name
        .as_deref()
        .and_then(|name| config.workspace(name))
        .map(|workspace| resolved_windows(&workspace.windows, &before))
        .unwrap_or_default();

    let target = config
        .workspaces
        .get_mut(target_name)
        .expect("target existence checked");
    let mut missing_indexes = Vec::new();
    for (index, window) in target.windows.iter().enumerate() {
        match resolve_window(window, &before) {
            WindowResolution::Resolved(_) => {}
            WindowResolution::Ambiguous(_) => {
                return Err(SwitchError::WindowUnavailable {
                    workspace: target_name.to_string(),
                    id: window.id,
                    state: WindowState::Ambiguous,
                });
            }
            WindowResolution::Missing => missing_indexes.push(index),
        }
    }

    let before_ids: BTreeSet<_> = before.iter().map(|window| window.id).collect();
    for &index in &missing_indexes {
        let window = &target.windows[index];
        let Some(recovery) = window.recovery.as_ref() else {
            return Err(SwitchError::WindowUnavailable {
                workspace: target_name.to_string(),
                id: window.id,
                state: WindowState::Missing,
            });
        };
        if recovery_adapter(window, recovery, registry, generic).is_none() {
            return Err(SwitchError::RecoveryUnavailable {
                workspace: target_name.to_string(),
                id: window.id,
            });
        }
    }

    let mut generic_bundles_launched = BTreeSet::new();
    for &index in &missing_indexes {
        let window = &target.windows[index];
        let recovery = window.recovery.as_ref().expect("validated before launch");
        let adapter =
            recovery_adapter(window, recovery, registry, generic).expect("validated before launch");

        let should_launch = if matches!(recovery, RecoveryState::Generic) {
            window
                .bundle_id
                .as_deref()
                .map(|bundle_id| generic_bundles_launched.insert(bundle_id.to_ascii_lowercase()))
                .unwrap_or(true)
        } else {
            true
        };
        if should_launch && let Err(source) = adapter.restore(window, recovery) {
            rollback_recovery(platform, &before_ids, &previous_windows);
            return Err(SwitchError::RecoveryFailed {
                workspace: target_name.to_string(),
                id: window.id,
                source,
            });
        }
    }

    let after = if missing_indexes.is_empty() {
        before.clone()
    } else {
        match poll_for_recovered_windows(
            target,
            target_name,
            &missing_indexes,
            registry,
            generic,
            platform,
        ) {
            Ok(windows) => windows,
            Err(error) => {
                rollback_recovery(platform, &before_ids, &previous_windows);
                return Err(error);
            }
        }
    };

    let target_windows = resolved_windows(&target.windows, &after);
    if previous_name.as_deref() == Some(target_name) {
        platform.restore(&target_windows)?;
        return Ok(());
    }

    if let Err(error) = platform.minimize(&previous_windows) {
        rollback_recovery(platform, &before_ids, &previous_windows);
        return Err(error);
    }
    if let Err(error) = platform.restore(&target_windows) {
        platform.restore(&previous_windows).ok();
        rollback_recovery(platform, &before_ids, &previous_windows);
        return Err(error);
    }

    state.active_workspace = Some(target_name.to_string());
    Ok(())
}

fn poll_for_recovered_windows(
    target: &mut crate::Workspace,
    target_name: &str,
    missing_indexes: &[usize],
    registry: &RecoveryRegistry,
    generic: &dyn RecoveryAdapter,
    platform: &mut dyn SwitchPlatform,
) -> Result<Vec<WindowInfo>, SwitchError> {
    let mut unresolved_ids = Vec::new();
    for _ in 0..RECOVERY_POLL_ATTEMPTS {
        platform.wait(RECOVERY_POLL_INTERVAL);
        let latest = platform.list_windows()?;
        let mut used_ids: BTreeSet<_> = target
            .windows
            .iter()
            .enumerate()
            .filter(|(index, _)| !missing_indexes.contains(index))
            .filter_map(|(_, window)| match resolve_window(window, &latest) {
                WindowResolution::Resolved(current) => Some(current.id),
                _ => None,
            })
            .collect();

        unresolved_ids.clear();
        for &index in missing_indexes {
            let saved = &target.windows[index];
            let recovery = saved.recovery.as_ref().expect("validated before launch");
            let adapter = recovery_adapter(saved, recovery, registry, generic)
                .expect("validated before launch");
            let mut matches: Vec<_> = latest
                .iter()
                .filter(|candidate| !used_ids.contains(&candidate.id))
                .filter(|candidate| adapter.matches(saved, candidate))
                .cloned()
                .collect();
            if matches.is_empty() && matches!(recovery, RecoveryState::Generic) {
                matches = latest
                    .iter()
                    .filter(|candidate| !used_ids.contains(&candidate.id))
                    .filter(|candidate| same_bundle_id(saved, candidate))
                    .cloned()
                    .collect();
            }
            match matches.as_slice() {
                [current] => {
                    used_ids.insert(current.id);
                    refresh_window_fingerprint(&mut target.windows[index], current);
                }
                _ => unresolved_ids.push(saved.id),
            }
        }
        if unresolved_ids.is_empty() {
            return Ok(latest);
        }
    }

    Err(SwitchError::RecoveryTimedOut {
        workspace: target_name.to_string(),
        ids: unresolved_ids,
    })
}

fn same_bundle_id(first: &WindowInfo, second: &WindowInfo) -> bool {
    first
        .bundle_id
        .as_deref()
        .zip(second.bundle_id.as_deref())
        .is_some_and(|(first, second)| first.eq_ignore_ascii_case(second))
}

fn recovery_adapter<'a>(
    window: &WindowInfo,
    recovery: &RecoveryState,
    registry: &'a RecoveryRegistry,
    generic: &'a dyn RecoveryAdapter,
) -> Option<&'a dyn RecoveryAdapter> {
    if matches!(recovery, RecoveryState::Generic) {
        Some(generic)
    } else {
        registry.adapter_for_window(window)
    }
}

fn resolved_windows(saved: &[WindowInfo], current: &[WindowInfo]) -> Vec<WindowInfo> {
    saved
        .iter()
        .filter_map(|window| match resolve_window(window, current) {
            WindowResolution::Resolved(current) => Some(current),
            _ => None,
        })
        .collect()
}

fn rollback_recovery(
    platform: &mut dyn SwitchPlatform,
    before_ids: &BTreeSet<u32>,
    previous_windows: &[WindowInfo],
) {
    if let Ok(current) = platform.list_windows() {
        let created: Vec<_> = current
            .into_iter()
            .filter(|window| !before_ids.contains(&window.id))
            .collect();
        if !created.is_empty() {
            platform.minimize_best_effort(&created);
        }
    }
    if !previous_windows.is_empty() {
        platform.restore(previous_windows).ok();
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use super::*;
    use crate::{WindowBounds, Workspace};

    #[derive(Default)]
    struct FakeAdapter {
        launches: Mutex<Vec<u32>>,
    }

    impl RecoveryAdapter for FakeAdapter {
        fn capture(&self, _window: &WindowInfo) -> Result<RecoveryState, RecoveryError> {
            Ok(RecoveryState::Generic)
        }

        fn restore(
            &self,
            window: &WindowInfo,
            _state: &RecoveryState,
        ) -> Result<(), RecoveryError> {
            self.launches.lock().unwrap().push(window.id);
            Ok(())
        }

        fn matches(&self, saved: &WindowInfo, candidate: &WindowInfo) -> bool {
            saved.bundle_id == candidate.bundle_id && saved.title == candidate.title
        }
    }

    struct FailingAdapter;

    impl RecoveryAdapter for FailingAdapter {
        fn capture(&self, _window: &WindowInfo) -> Result<RecoveryState, RecoveryError> {
            unreachable!()
        }

        fn restore(
            &self,
            _window: &WindowInfo,
            _state: &RecoveryState,
        ) -> Result<(), RecoveryError> {
            Err(RecoveryError::Restore("simulated failure".to_string()))
        }

        fn matches(&self, _saved: &WindowInfo, _candidate: &WindowInfo) -> bool {
            false
        }
    }

    struct FakePlatform {
        snapshots: VecDeque<Vec<WindowInfo>>,
        latest: Vec<WindowInfo>,
        minimized: Vec<Vec<u32>>,
        restored: Vec<Vec<u32>>,
    }

    impl FakePlatform {
        fn new(snapshots: Vec<Vec<WindowInfo>>) -> Self {
            Self {
                snapshots: snapshots.into(),
                latest: Vec::new(),
                minimized: Vec::new(),
                restored: Vec::new(),
            }
        }
    }

    impl SwitchPlatform for FakePlatform {
        fn list_windows(&mut self) -> Result<Vec<WindowInfo>, SwitchError> {
            if let Some(next) = self.snapshots.pop_front() {
                self.latest = next;
            }
            Ok(self.latest.clone())
        }

        fn minimize(&mut self, windows: &[WindowInfo]) -> Result<(), SwitchError> {
            self.minimized
                .push(windows.iter().map(|window| window.id).collect());
            Ok(())
        }

        fn minimize_best_effort(&mut self, windows: &[WindowInfo]) {
            self.minimized
                .push(windows.iter().map(|window| window.id).collect());
        }

        fn restore(&mut self, windows: &[WindowInfo]) -> Result<(), SwitchError> {
            self.restored
                .push(windows.iter().map(|window| window.id).collect());
            Ok(())
        }

        fn wait(&mut self, _duration: Duration) {}
    }

    fn window(id: u32, bundle: &str, title: &str, recovery: bool) -> WindowInfo {
        WindowInfo {
            id,
            pid: id as i32,
            owner: bundle.to_string(),
            title: Some(title.to_string()),
            bounds: Some(WindowBounds {
                x: id as i32,
                y: 20,
                width: 800,
                height: 600,
            }),
            bundle_id: Some(bundle.to_string()),
            application_path: Some(PathBuf::from(format!("/Applications/{bundle}.app"))),
            recovery: recovery.then_some(RecoveryState::Generic),
            recovery_warning: None,
            placement: None,
            placement_warning: None,
        }
    }

    fn config(previous: Vec<WindowInfo>, target: Vec<WindowInfo>) -> Config {
        let mut workspaces = std::collections::BTreeMap::new();
        workspaces.insert("previous".to_string(), workspace(previous));
        workspaces.insert("target".to_string(), workspace(target));
        Config {
            version: 1,
            ignored_windows: Vec::new(),
            workspaces,
        }
    }

    fn workspace(windows: Vec<WindowInfo>) -> Workspace {
        Workspace {
            path: None,
            services: Vec::new(),
            urls: Vec::new(),
            windows,
        }
    }

    #[test]
    fn rejects_unknown_target_before_window_operations() {
        let mut config = Config::from_yaml("version: 1\nworkspaces: {}\n").unwrap();
        let mut state = RuntimeState::default();

        let error = switch_workspace(&mut config, &mut state, "missing").unwrap_err();

        assert!(matches!(error, SwitchError::WorkspaceMissing { .. }));
        assert_eq!(state, RuntimeState::default());
    }

    #[test]
    fn recovers_missing_window_before_minimizing_previous_workspace() {
        let previous = window(1, "previous.app", "Previous", false);
        let saved = window(2, "target.app", "Target", true);
        let recovered = window(99, "target.app", "Target", false);
        let adapter = FakeAdapter::default();
        let mut platform = FakePlatform::new(vec![
            vec![previous.clone()],
            vec![previous.clone(), recovered],
        ]);
        let mut config = config(vec![previous], vec![saved]);
        let mut state = RuntimeState {
            version: 1,
            active_workspace: Some("previous".to_string()),
        };

        switch_workspace_with(
            &mut config,
            &mut state,
            "target",
            &RecoveryRegistry::new(),
            &adapter,
            &mut platform,
        )
        .unwrap();

        assert_eq!(*adapter.launches.lock().unwrap(), [2]);
        assert_eq!(config.workspace("target").unwrap().windows[0].id, 99);
        assert_eq!(platform.minimized, [vec![1]]);
        assert_eq!(platform.restored, [vec![99]]);
        assert_eq!(state.active_workspace.as_deref(), Some("target"));
    }

    #[test]
    fn generic_app_with_multiple_windows_launches_once() {
        let first = window(2, "generic.app", "First", true);
        let second = window(3, "generic.app", "Second", true);
        let recovered_first = window(20, "generic.app", "First", false);
        let recovered_second = window(30, "generic.app", "Second", false);
        let adapter = FakeAdapter::default();
        let mut platform =
            FakePlatform::new(vec![Vec::new(), vec![recovered_first, recovered_second]]);
        let mut config = config(Vec::new(), vec![first, second]);
        let mut state = RuntimeState::default();

        switch_workspace_with(
            &mut config,
            &mut state,
            "target",
            &RecoveryRegistry::new(),
            &adapter,
            &mut platform,
        )
        .unwrap();

        assert_eq!(adapter.launches.lock().unwrap().len(), 1);
        assert_eq!(platform.restored, [vec![20, 30]]);
    }

    #[test]
    fn timeout_minimizes_created_windows_and_keeps_previous_active() {
        let previous = window(1, "previous.app", "Previous", false);
        let first = window(2, "first.app", "First", true);
        let second = window(3, "second.app", "Second", true);
        let recovered_first = window(20, "first.app", "First", false);
        let adapter = FakeAdapter::default();
        let latest = vec![previous.clone(), recovered_first];
        let mut snapshots = vec![vec![previous.clone()]];
        snapshots.extend(std::iter::repeat_n(latest, RECOVERY_POLL_ATTEMPTS + 1));
        let mut platform = FakePlatform::new(snapshots);
        let mut config = config(vec![previous], vec![first, second]);
        let mut state = RuntimeState {
            version: 1,
            active_workspace: Some("previous".to_string()),
        };

        let error = switch_workspace_with(
            &mut config,
            &mut state,
            "target",
            &RecoveryRegistry::new(),
            &adapter,
            &mut platform,
        )
        .unwrap_err();

        assert!(matches!(error, SwitchError::RecoveryTimedOut { .. }));
        assert_eq!(state.active_workspace.as_deref(), Some("previous"));
        assert!(platform.minimized.contains(&vec![20]));
        assert!(platform.restored.contains(&vec![1]));
    }

    #[test]
    fn repeated_switch_does_not_launch_duplicate_windows() {
        let saved = window(2, "target.app", "Target", true);
        let recovered = window(99, "target.app", "Target", false);
        let adapter = FakeAdapter::default();
        let mut platform = FakePlatform::new(vec![Vec::new(), vec![recovered]]);
        let mut config = config(Vec::new(), vec![saved]);
        let mut state = RuntimeState::default();

        switch_workspace_with(
            &mut config,
            &mut state,
            "target",
            &RecoveryRegistry::new(),
            &adapter,
            &mut platform,
        )
        .unwrap();
        switch_workspace_with(
            &mut config,
            &mut state,
            "target",
            &RecoveryRegistry::new(),
            &adapter,
            &mut platform,
        )
        .unwrap();

        assert_eq!(*adapter.launches.lock().unwrap(), [2]);
        assert_eq!(state.active_workspace.as_deref(), Some("target"));
    }

    #[test]
    fn generic_recovery_accepts_a_unique_bundle_window_when_title_changes() {
        let saved = window(2, "target.app", "Old Title", true);
        let recovered = window(99, "target.app", "New Title", false);
        let adapter = FakeAdapter::default();
        let mut platform = FakePlatform::new(vec![Vec::new(), vec![recovered]]);
        let mut config = config(Vec::new(), vec![saved]);
        let mut state = RuntimeState::default();

        switch_workspace_with(
            &mut config,
            &mut state,
            "target",
            &RecoveryRegistry::new(),
            &adapter,
            &mut platform,
        )
        .unwrap();

        let recovered = &config.workspace("target").unwrap().windows[0];
        assert_eq!(recovered.id, 99);
        assert_eq!(recovered.title.as_deref(), Some("New Title"));
        assert_eq!(state.active_workspace.as_deref(), Some("target"));
    }

    #[test]
    fn app_specific_recovery_waits_for_adapter_match() {
        let mut saved = window(2, "target.app", "Expected", false);
        saved.recovery = Some(RecoveryState::Editor {
            project_path: PathBuf::from("/tmp/project"),
        });
        let welcome = window(20, "target.app", "Welcome", false);
        let recovered = window(99, "target.app", "Expected", false);
        let adapter = Arc::new(FakeAdapter::default());
        let mut registry = RecoveryRegistry::new();
        registry.register("target.app", adapter.clone());
        let mut platform = FakePlatform::new(vec![
            Vec::new(),
            vec![welcome.clone()],
            vec![welcome, recovered],
        ]);
        let mut config = config(Vec::new(), vec![saved]);
        let mut state = RuntimeState::default();

        switch_workspace_with(
            &mut config,
            &mut state,
            "target",
            &registry,
            &FakeAdapter::default(),
            &mut platform,
        )
        .unwrap();

        let recovered = &config.workspace("target").unwrap().windows[0];
        assert_eq!(recovered.id, 99);
        assert_eq!(recovered.title.as_deref(), Some("Expected"));
        assert_eq!(*adapter.launches.lock().unwrap(), [2]);
    }

    #[test]
    fn later_adapter_failure_rolls_back_earlier_launches() {
        let previous = window(1, "previous.app", "Previous", false);
        let first = window(2, "first.app", "First", true);
        let mut second = window(3, "second.app", "Second", false);
        second.recovery = Some(RecoveryState::Editor {
            project_path: PathBuf::from("/tmp/second"),
        });
        let created = window(20, "first.app", "First", false);
        let generic = FakeAdapter::default();
        let mut registry = RecoveryRegistry::new();
        registry.register("second.app", Arc::new(FailingAdapter));
        let mut platform = FakePlatform::new(vec![
            vec![previous.clone()],
            vec![previous.clone(), created],
        ]);
        let mut config = config(vec![previous], vec![first, second]);
        let mut state = RuntimeState {
            version: 1,
            active_workspace: Some("previous".to_string()),
        };

        let error = switch_workspace_with(
            &mut config,
            &mut state,
            "target",
            &registry,
            &generic,
            &mut platform,
        )
        .unwrap_err();

        assert!(matches!(error, SwitchError::RecoveryFailed { id: 3, .. }));
        assert_eq!(*generic.launches.lock().unwrap(), [2]);
        assert!(platform.minimized.contains(&vec![20]));
        assert!(platform.restored.contains(&vec![1]));
        assert_eq!(state.active_workspace.as_deref(), Some("previous"));
    }
}

use serde::Serialize;

use crate::{
    RecoveryAdapter, RecoveryError, RecoveryKind, RecoveryRegistry, WindowInfo, WindowState,
    Workspace, reconcile_windows,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SnapshotWindowReport {
    pub id: u32,
    pub application: String,
    pub captured: bool,
    pub recovery_kind: Option<RecoveryKind>,
    pub warning: Option<String>,
}

pub fn snapshot_workspace(
    workspace: &mut Workspace,
    current_windows: &[WindowInfo],
    registry: &RecoveryRegistry,
    fallback: &dyn RecoveryAdapter,
) -> Result<Vec<SnapshotWindowReport>, RecoveryError> {
    let statuses = reconcile_windows(&mut workspace.windows, current_windows);
    let mut report = Vec::with_capacity(workspace.windows.len());

    for (window, status) in workspace.windows.iter_mut().zip(statuses) {
        if status.resolved_id.is_none() {
            let warning = if window.recovery.is_some() {
                format!(
                    "window is {}; preserved its previous recovery snapshot",
                    state_label(status.state)
                )
            } else {
                format!(
                    "window is {} and has no previous recovery snapshot",
                    state_label(status.state)
                )
            };
            report.push(SnapshotWindowReport {
                id: window.id,
                application: window.owner.clone(),
                captured: false,
                recovery_kind: window.recovery.as_ref().map(|state| state.kind()),
                warning: Some(warning),
            });
            continue;
        }

        let selected = registry.adapter_for_window(window);
        let (recovery, warning) = match selected {
            Some(adapter) => match adapter.capture(window) {
                Ok(recovery) => (recovery, None),
                Err(error) => (
                    fallback.capture(window)?,
                    Some(format!(
                        "app-specific capture failed ({error}); using generic recovery"
                    )),
                ),
            },
            None => {
                let reason = if window.bundle_id.is_some() {
                    "no app-specific adapter is registered"
                } else {
                    "application bundle identity is unavailable"
                };
                (
                    fallback.capture(window)?,
                    Some(format!("{reason}; using generic recovery")),
                )
            }
        };
        let recovery_kind = recovery.kind();
        window.recovery = Some(recovery);
        window.recovery_warning.clone_from(&warning);
        report.push(SnapshotWindowReport {
            id: window.id,
            application: window.owner.clone(),
            captured: true,
            recovery_kind: Some(recovery_kind),
            warning,
        });
    }

    Ok(report)
}

fn state_label(state: WindowState) -> &'static str {
    match state {
        WindowState::Visible => "visible",
        WindowState::Minimized => "minimized",
        WindowState::Ambiguous => "ambiguous",
        WindowState::Missing => "missing",
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use super::*;
    use crate::{GenericAppAdapter, RecoveryState};

    struct FailingAdapter;

    impl RecoveryAdapter for FailingAdapter {
        fn capture(&self, _window: &WindowInfo) -> Result<RecoveryState, RecoveryError> {
            Err(RecoveryError::Capture("test capture failure".to_string()))
        }

        fn restore(
            &self,
            _window: &WindowInfo,
            _state: &RecoveryState,
        ) -> Result<(), RecoveryError> {
            unreachable!()
        }

        fn matches(&self, _saved: &WindowInfo, _candidate: &WindowInfo) -> bool {
            false
        }
    }

    fn window(id: u32, title: &str) -> WindowInfo {
        WindowInfo {
            id,
            pid: id as i32,
            owner: "Test App".to_string(),
            title: Some(title.to_string()),
            bounds: None,
            bundle_id: Some("com.example.test".to_string()),
            application_path: Some(PathBuf::from("/Applications/Test App.app")),
            recovery: None,
            recovery_warning: None,
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
    fn refreshes_visible_windows_and_preserves_closed_resources() {
        let mut visible = window(1, "old");
        visible.recovery = Some(RecoveryState::Generic);
        let mut closed = window(2, "closed");
        closed.recovery = Some(RecoveryState::Editor {
            project_path: PathBuf::from("/tmp/closed-project"),
        });
        let mut workspace = workspace(vec![visible, closed]);
        let current = window(1, "old");

        let report = snapshot_workspace(
            &mut workspace,
            &[current],
            &RecoveryRegistry::new(),
            &GenericAppAdapter,
        )
        .unwrap();

        assert_eq!(workspace.windows[0].recovery, Some(RecoveryState::Generic));
        assert_eq!(
            workspace.windows[1].recovery,
            Some(RecoveryState::Editor {
                project_path: PathBuf::from("/tmp/closed-project")
            })
        );
        assert!(report[0].captured);
        assert!(!report[1].captured);
        assert!(report[1].warning.as_deref().unwrap().contains("preserved"));
    }

    #[test]
    fn adapter_capture_failure_falls_back_to_generic_with_warning() {
        let mut registry = RecoveryRegistry::new();
        registry.register("com.example.test", Arc::new(FailingAdapter));
        let current = window(1, "window");
        let mut workspace = workspace(vec![current.clone()]);

        let report =
            snapshot_workspace(&mut workspace, &[current], &registry, &GenericAppAdapter).unwrap();

        assert_eq!(workspace.windows[0].recovery, Some(RecoveryState::Generic));
        assert!(
            report[0]
                .warning
                .as_deref()
                .unwrap()
                .contains("test capture failure")
        );
    }
}

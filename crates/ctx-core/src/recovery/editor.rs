use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
};

use crate::{RecoveryAdapter, RecoveryError, RecoveryState, WindowInfo};

pub trait VsCodePlatform: Send + Sync {
    fn project_path(&self, window: &WindowInfo) -> Result<PathBuf, RecoveryError>;

    fn launch(&self, window: &WindowInfo, project_path: &Path) -> Result<(), RecoveryError>;
}

#[derive(Debug, Default)]
pub struct SystemVsCodePlatform;

impl VsCodePlatform for SystemVsCodePlatform {
    fn project_path(&self, window: &WindowInfo) -> Result<PathBuf, RecoveryError> {
        crate::accessibility::window_document_path(window)
            .map_err(|error| RecoveryError::Capture(error.to_string()))
    }

    fn launch(&self, window: &WindowInfo, project_path: &Path) -> Result<(), RecoveryError> {
        let launcher = window
            .application_path
            .as_ref()
            .map(|path| path.join("Contents/Resources/app/bin/code"))
            .filter(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from("code"));
        let mut command = Command::new(&launcher);
        command
            .arg("--new-window")
            .arg(project_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command.spawn().map(|_| ()).map_err(|error| {
            RecoveryError::Restore(format!(
                "could not launch {} for {}: {error}",
                launcher.display(),
                project_path.display()
            ))
        })
    }
}

pub struct VsCodeAdapter {
    platform: Arc<dyn VsCodePlatform>,
}

impl VsCodeAdapter {
    pub fn new(platform: Arc<dyn VsCodePlatform>) -> Self {
        Self { platform }
    }

    pub fn system() -> Self {
        Self::new(Arc::new(SystemVsCodePlatform))
    }
}

impl RecoveryAdapter for VsCodeAdapter {
    fn capture(&self, window: &WindowInfo) -> Result<RecoveryState, RecoveryError> {
        Ok(RecoveryState::Editor {
            project_path: self.platform.project_path(window)?,
        })
    }

    fn restore(&self, window: &WindowInfo, state: &RecoveryState) -> Result<(), RecoveryError> {
        let RecoveryState::Editor { project_path } = state else {
            return Err(RecoveryError::Restore(format!(
                "{} does not contain editor recovery state",
                window.owner
            )));
        };

        self.platform.launch(window, project_path)
    }

    fn matches(&self, saved: &WindowInfo, candidate: &WindowInfo) -> bool {
        if !same_bundle_id(saved, candidate) {
            return false;
        }

        let Some(RecoveryState::Editor { project_path }) = &saved.recovery else {
            return false;
        };

        self.platform
            .project_path(candidate)
            .is_ok_and(|candidate_path| candidate_path == *project_path)
    }
}

fn same_bundle_id(first: &WindowInfo, second: &WindowInfo) -> bool {
    first
        .bundle_id
        .as_deref()
        .zip(second.bundle_id.as_deref())
        .is_some_and(|(first, second)| first.eq_ignore_ascii_case(second))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct FakePlatform {
        project_path: PathBuf,
        launches: Mutex<Vec<PathBuf>>,
    }

    impl FakePlatform {
        fn new(project_path: impl Into<PathBuf>) -> Self {
            Self {
                project_path: project_path.into(),
                launches: Mutex::new(Vec::new()),
            }
        }
    }

    impl VsCodePlatform for FakePlatform {
        fn project_path(&self, _window: &WindowInfo) -> Result<PathBuf, RecoveryError> {
            Ok(self.project_path.clone())
        }

        fn launch(&self, _window: &WindowInfo, project_path: &Path) -> Result<(), RecoveryError> {
            self.launches
                .lock()
                .unwrap()
                .push(project_path.to_path_buf());
            Ok(())
        }
    }

    fn window(id: u32, bundle_id: &str) -> WindowInfo {
        WindowInfo {
            id,
            pid: id as i32,
            owner: "Visual Studio Code".to_string(),
            title: Some("devLayout".to_string()),
            bounds: None,
            bundle_id: Some(bundle_id.to_string()),
            application_path: None,
            recovery: None,
            recovery_warning: None,
        }
    }

    #[test]
    fn captures_restores_and_matches_project_path() {
        let platform = Arc::new(FakePlatform::new("/tmp/devLayout"));
        let adapter = VsCodeAdapter::new(platform.clone());
        let mut saved = window(1, "com.microsoft.VSCode");
        let state = adapter.capture(&saved).unwrap();
        saved.recovery = Some(state.clone());

        adapter.restore(&saved, &state).unwrap();

        assert_eq!(
            state,
            RecoveryState::Editor {
                project_path: PathBuf::from("/tmp/devLayout")
            }
        );
        assert_eq!(
            *platform.launches.lock().unwrap(),
            [PathBuf::from("/tmp/devLayout")]
        );
        assert!(adapter.matches(&saved, &window(99, "COM.MICROSOFT.VSCODE")));
    }

    #[test]
    fn rejects_other_applications_and_non_editor_state() {
        let platform = Arc::new(FakePlatform::new("/tmp/devLayout"));
        let adapter = VsCodeAdapter::new(platform);
        let mut saved = window(1, "com.microsoft.VSCode");
        saved.recovery = Some(RecoveryState::Generic);

        assert!(!adapter.matches(&saved, &window(2, "com.microsoft.VSCode")));
        assert!(!adapter.matches(&saved, &window(3, "org.mozilla.firefox")));
        assert!(adapter.restore(&saved, &RecoveryState::Generic).is_err());
    }
}

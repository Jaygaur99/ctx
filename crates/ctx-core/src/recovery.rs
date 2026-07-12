use std::{collections::BTreeMap, sync::Arc};

use thiserror::Error;

use crate::{RecoveryState, WindowInfo};

mod browser;
mod editor;
mod terminal;

pub use browser::{FirefoxAdapter, FirefoxPlatform, SystemFirefoxPlatform};

pub use editor::{
    AntigravityAdapter, AntigravityPlatform, SystemAntigravityPlatform, SystemVsCodePlatform,
    VsCodeAdapter, VsCodePlatform,
};
pub use terminal::{SystemWarpPlatform, WarpAdapter, WarpPlatform};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RecoveryError {
    #[error("could not capture recovery state: {0}")]
    Capture(String),

    #[error("could not restore recovery state: {0}")]
    Restore(String),
}

pub trait RecoveryAdapter: Send + Sync {
    fn capture(&self, window: &WindowInfo) -> Result<RecoveryState, RecoveryError>;

    fn restore(&self, window: &WindowInfo, state: &RecoveryState) -> Result<(), RecoveryError>;

    fn matches(&self, saved: &WindowInfo, candidate: &WindowInfo) -> bool;
}

#[derive(Default)]
pub struct RecoveryRegistry {
    adapters: BTreeMap<String, Arc<dyn RecoveryAdapter>>,
}

#[derive(Debug, Default)]
pub struct GenericAppAdapter;

impl RecoveryAdapter for GenericAppAdapter {
    fn capture(&self, _window: &WindowInfo) -> Result<RecoveryState, RecoveryError> {
        Ok(RecoveryState::Generic)
    }

    fn restore(&self, window: &WindowInfo, _state: &RecoveryState) -> Result<(), RecoveryError> {
        launch_application(window)
    }

    fn matches(&self, saved: &WindowInfo, candidate: &WindowInfo) -> bool {
        let same_bundle = saved
            .bundle_id
            .as_deref()
            .zip(candidate.bundle_id.as_deref())
            .is_some_and(|(saved, candidate)| saved.eq_ignore_ascii_case(candidate));
        if !same_bundle {
            return false;
        }

        let same_title = saved.title.is_some() && saved.title == candidate.title;
        let same_bounds = saved
            .bounds
            .zip(candidate.bounds)
            .is_some_and(|(saved, candidate)| saved.is_close_to(candidate));

        same_title || same_bounds || (saved.title.is_none() && saved.bounds.is_none())
    }
}

impl RecoveryRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        bundle_id: impl Into<String>,
        adapter: Arc<dyn RecoveryAdapter>,
    ) -> Option<Arc<dyn RecoveryAdapter>> {
        self.adapters
            .insert(normalize_bundle_id(&bundle_id.into()), adapter)
    }

    pub fn adapter_for(&self, bundle_id: &str) -> Option<&dyn RecoveryAdapter> {
        self.adapters
            .get(&normalize_bundle_id(bundle_id))
            .map(AsRef::as_ref)
    }

    pub fn adapter_for_window(&self, window: &WindowInfo) -> Option<&dyn RecoveryAdapter> {
        window
            .bundle_id
            .as_deref()
            .and_then(|bundle_id| self.adapter_for(bundle_id))
    }
}

pub fn default_recovery_registry() -> RecoveryRegistry {
    let mut registry = RecoveryRegistry::new();
    let adapter: Arc<dyn RecoveryAdapter> = Arc::new(VsCodeAdapter::system());
    for bundle_id in [
        "com.microsoft.VSCode",
        "com.microsoft.VSCodeInsiders",
        "com.visualstudio.code.oss",
    ] {
        registry.register(bundle_id, adapter.clone());
    }
    let adapter: Arc<dyn RecoveryAdapter> = Arc::new(AntigravityAdapter::system());
    for bundle_id in ["com.google.antigravity", "com.google.antigravity-ide"] {
        registry.register(bundle_id, adapter.clone());
    }
    registry.register(
        "dev.warp.Warp-Stable",
        Arc::new(WarpAdapter::system()) as Arc<dyn RecoveryAdapter>,
    );
    registry.register(
        "org.mozilla.firefox",
        Arc::new(FirefoxAdapter::system()) as Arc<dyn RecoveryAdapter>,
    );
    registry
}

fn normalize_bundle_id(bundle_id: &str) -> String {
    bundle_id.trim().to_ascii_lowercase()
}

#[cfg(target_os = "macos")]
fn launch_application(window: &WindowInfo) -> Result<(), RecoveryError> {
    use std::process::{Command, Stdio};

    let mut command = Command::new("/usr/bin/open");
    if let Some(path) = &window.application_path {
        command.arg("-a").arg(path);
    } else if let Some(bundle_id) = &window.bundle_id {
        command.arg("-b").arg(bundle_id);
    } else {
        return Err(RecoveryError::Restore(format!(
            "{} has no bundle ID or application path",
            window.owner
        )));
    }
    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| {
            RecoveryError::Restore(format!("could not reopen {}: {error}", window.owner))
        })?;
    if !output.status.success() {
        return Err(RecoveryError::Restore(format!(
            "macOS could not reopen {}: {}",
            window.owner,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn launch_application(_window: &WindowInfo) -> Result<(), RecoveryError> {
    Err(RecoveryError::Restore(
        "application recovery is only supported on macOS".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use super::*;

    #[derive(Default)]
    struct FakeAdapter {
        restored: Mutex<Vec<u32>>,
    }

    impl RecoveryAdapter for FakeAdapter {
        fn capture(&self, _window: &WindowInfo) -> Result<RecoveryState, RecoveryError> {
            Ok(RecoveryState::Editor {
                project_path: PathBuf::from("/tmp/devLayout"),
            })
        }

        fn restore(
            &self,
            window: &WindowInfo,
            _state: &RecoveryState,
        ) -> Result<(), RecoveryError> {
            self.restored.lock().unwrap().push(window.id);
            Ok(())
        }

        fn matches(&self, saved: &WindowInfo, candidate: &WindowInfo) -> bool {
            saved
                .bundle_id
                .as_deref()
                .zip(candidate.bundle_id.as_deref())
                .is_some_and(|(saved, candidate)| saved.eq_ignore_ascii_case(candidate))
                && saved.title == candidate.title
        }
    }

    fn window(id: u32, bundle_id: Option<&str>) -> WindowInfo {
        WindowInfo {
            id,
            pid: id as i32,
            owner: "Visual Studio Code".to_string(),
            title: Some("devLayout".to_string()),
            bounds: None,
            bundle_id: bundle_id.map(str::to_string),
            application_path: None,
            recovery: None,
            recovery_warning: None,
        }
    }

    #[test]
    fn selects_captures_restores_and_matches_with_registered_adapter() {
        let adapter = Arc::new(FakeAdapter::default());
        let mut registry = RecoveryRegistry::new();
        registry.register("com.microsoft.VSCode", adapter.clone());

        let saved = window(42, Some("COM.MICROSOFT.VSCODE"));
        let candidate = window(99, Some("com.microsoft.VSCode"));
        let selected = registry.adapter_for_window(&saved).unwrap();
        let state = selected.capture(&saved).unwrap();
        selected.restore(&saved, &state).unwrap();

        assert_eq!(
            state,
            RecoveryState::Editor {
                project_path: PathBuf::from("/tmp/devLayout")
            }
        );
        assert!(selected.matches(&saved, &candidate));
        assert_eq!(*adapter.restored.lock().unwrap(), [42]);
    }

    #[test]
    fn windows_without_bundle_identity_have_no_adapter() {
        let mut registry = RecoveryRegistry::new();
        registry.register("com.microsoft.VSCode", Arc::new(FakeAdapter::default()));

        assert!(registry.adapter_for_window(&window(42, None)).is_none());
    }

    #[test]
    fn generic_adapter_captures_and_matches_stable_application_identity() {
        let adapter = GenericAppAdapter;
        let saved = window(42, Some("org.mozilla.firefox"));
        let candidate = window(99, Some("ORG.MOZILLA.FIREFOX"));

        assert_eq!(adapter.capture(&saved).unwrap(), RecoveryState::Generic);
        assert!(adapter.matches(&saved, &candidate));
        assert!(!adapter.matches(&saved, &window(100, Some("com.microsoft.VSCode"))));
    }

    #[test]
    fn default_registry_selects_vscode_by_bundle_id() {
        let registry = default_recovery_registry();

        assert!(registry.adapter_for("com.microsoft.VSCode").is_some());
        assert!(
            registry
                .adapter_for("com.microsoft.VSCodeInsiders")
                .is_some()
        );
        assert!(registry.adapter_for("org.mozilla.firefox").is_some());
        assert!(registry.adapter_for("com.google.antigravity").is_some());
        assert!(registry.adapter_for("com.google.antigravity-ide").is_some());
        assert!(registry.adapter_for("dev.warp.Warp-Stable").is_some());
    }
}

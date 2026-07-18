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

pub trait AntigravityPlatform: Send + Sync {
    fn project_path(&self, window: &WindowInfo) -> Result<PathBuf, RecoveryError>;

    fn launch(&self, window: &WindowInfo, project_path: &Path) -> Result<(), RecoveryError>;
}

#[derive(Debug, Default)]
pub struct SystemVsCodePlatform;

impl VsCodePlatform for SystemVsCodePlatform {
    fn project_path(&self, window: &WindowInfo) -> Result<PathBuf, RecoveryError> {
        if let Some(project_path) = vscode_workspace_path(window) {
            return Ok(project_path);
        }

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

fn vscode_workspace_path(window: &WindowInfo) -> Option<PathBuf> {
    let support_directory = match window.bundle_id.as_deref()?.to_ascii_lowercase().as_str() {
        "com.microsoft.vscode" => "Code",
        "com.microsoft.vscodeinsiders" => "Code - Insiders",
        "com.visualstudio.code.oss" => "Code - OSS",
        _ => return None,
    };
    let storage_path = directories::BaseDirs::new()?
        .home_dir()
        .join("Library/Application Support")
        .join(support_directory)
        .join("User/globalStorage/storage.json");
    let storage = serde_json::from_reader(std::fs::File::open(storage_path).ok()?).ok()?;

    vscode_workspace_path_from_storage(window, &storage)
}

fn vscode_workspace_path_from_storage(
    window: &WindowInfo,
    storage: &serde_json::Value,
) -> Option<PathBuf> {
    let windows_state = storage.get("windowsState")?;
    let mut states = Vec::new();
    if let Some(last_active) = windows_state.get("lastActiveWindow") {
        states.push(last_active);
    }
    if let Some(opened) = windows_state
        .get("openedWindows")
        .and_then(|value| value.as_array())
    {
        states.extend(opened);
    }

    let candidates: Vec<_> = states
        .into_iter()
        .filter_map(|state| {
            let uri = state
                .get("folder")
                .and_then(|value| value.as_str())
                .or_else(|| {
                    state
                        .pointer("/workspace/configPath")
                        .and_then(|value| value.as_str())
                })?;
            Some((file_uri_path(uri)?, vscode_window_bounds(state)))
        })
        .collect();

    if let Some(expected) = window.bounds {
        let matching: Vec<_> = candidates
            .iter()
            .filter(|(_, bounds)| bounds.is_some_and(|actual| expected.is_close_to(actual)))
            .collect();
        if let [candidate] = matching.as_slice() {
            return Some(candidate.0.clone());
        }
    }

    match candidates.as_slice() {
        [(path, _)] => Some(path.clone()),
        _ => None,
    }
}

fn vscode_window_bounds(state: &serde_json::Value) -> Option<crate::WindowBounds> {
    let ui_state = state.get("uiState")?;
    Some(crate::WindowBounds {
        x: i32::try_from(ui_state.get("x")?.as_i64()?).ok()?,
        y: i32::try_from(ui_state.get("y")?.as_i64()?).ok()?,
        width: i32::try_from(ui_state.get("width")?.as_i64()?).ok()?,
        height: i32::try_from(ui_state.get("height")?.as_i64()?).ok()?,
    })
}

fn file_uri_path(uri: &str) -> Option<PathBuf> {
    let encoded_path = uri.strip_prefix("file://")?;
    let bytes = encoded_path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_digit(*bytes.get(index + 1)?)?;
            let low = hex_digit(*bytes.get(index + 2)?)?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded).ok().map(PathBuf::from)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub struct SystemAntigravityPlatform;

impl AntigravityPlatform for SystemAntigravityPlatform {
    fn project_path(&self, window: &WindowInfo) -> Result<PathBuf, RecoveryError> {
        crate::accessibility::window_document_path(window)
            .map_err(|error| RecoveryError::Capture(error.to_string()))
    }

    fn launch(&self, window: &WindowInfo, project_path: &Path) -> Result<(), RecoveryError> {
        let scheme = match window.bundle_id.as_deref() {
            Some(bundle_id) if bundle_id.eq_ignore_ascii_case("com.google.antigravity-ide") => {
                "antigravity-ide"
            }
            Some(bundle_id) if bundle_id.eq_ignore_ascii_case("com.google.antigravity") => {
                "antigravity"
            }
            _ => {
                return Err(RecoveryError::Restore(format!(
                    "{} has an unsupported Antigravity bundle ID",
                    window.owner
                )));
            }
        };
        let project_path = project_path.to_str().ok_or_else(|| {
            RecoveryError::Restore(format!(
                "project path {} is not valid UTF-8",
                project_path.display()
            ))
        })?;
        let deep_link = format!("{scheme}://file{}", encode_uri_path(project_path));

        Command::new("/usr/bin/open")
            .arg(&deep_link)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|error| {
                RecoveryError::Restore(format!(
                    "could not open Antigravity deep link {deep_link}: {error}"
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

        vscode_title_matches_project(candidate.title.as_deref(), project_path)
    }
}

fn vscode_title_matches_project(title: Option<&str>, project_path: &Path) -> bool {
    let label =
        if project_path.extension().and_then(|value| value.to_str()) == Some("code-workspace") {
            project_path.file_stem()
        } else {
            project_path.file_name()
        }
        .and_then(|value| value.to_str());

    label.is_some_and(|label| {
        title.is_some_and(|title| title.split('—').any(|segment| segment.trim() == label))
    })
}

pub struct AntigravityAdapter {
    platform: Arc<dyn AntigravityPlatform>,
}

impl AntigravityAdapter {
    pub fn new(platform: Arc<dyn AntigravityPlatform>) -> Self {
        Self { platform }
    }

    pub fn system() -> Self {
        Self::new(Arc::new(SystemAntigravityPlatform))
    }
}

impl RecoveryAdapter for AntigravityAdapter {
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

fn encode_uri_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(encoded, "%{byte:02X}").expect("writing to a String cannot fail");
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct FakePlatform {
        project_path: PathBuf,
        launches: Mutex<Vec<PathBuf>>,
    }

    impl AntigravityPlatform for FakePlatform {
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
            placement: None,
            placement_warning: None,
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
        let mut welcome = window(100, "com.microsoft.VSCode");
        welcome.title = Some("Welcome".to_string());
        assert!(!adapter.matches(&saved, &welcome));
    }

    #[test]
    fn reads_vscode_workspace_path_when_active_editor_has_no_document() {
        let mut current = window(1, "com.microsoft.VSCode");
        current.bounds = Some(crate::WindowBounds {
            x: 0,
            y: 33,
            width: 1470,
            height: 923,
        });
        let storage = serde_json::json!({
            "windowsState": {
                "lastActiveWindow": {
                    "folder": "file:///Users/jay/My%20Project",
                    "uiState": {
                        "x": 0,
                        "y": 33,
                        "width": 1470,
                        "height": 923
                    }
                },
                "openedWindows": []
            }
        });

        assert_eq!(
            vscode_workspace_path_from_storage(&current, &storage),
            Some(PathBuf::from("/Users/jay/My Project"))
        );
    }

    #[test]
    fn matches_vscode_workspace_by_window_geometry() {
        let mut current = window(1, "com.microsoft.VSCode");
        current.bounds = Some(crate::WindowBounds {
            x: 100,
            y: 100,
            width: 1200,
            height: 800,
        });
        let storage = serde_json::json!({
            "windowsState": {
                "lastActiveWindow": {
                    "folder": "file:///tmp/other",
                    "uiState": { "x": 0, "y": 33, "width": 1470, "height": 923 }
                },
                "openedWindows": [{
                    "folder": "file:///tmp/expected",
                    "uiState": { "x": 100, "y": 100, "width": 1200, "height": 800 }
                }]
            }
        });

        assert_eq!(
            vscode_workspace_path_from_storage(&current, &storage),
            Some(PathBuf::from("/tmp/expected"))
        );
    }

    #[test]
    fn matches_loaded_vscode_folder_title_but_not_welcome() {
        let project_path = Path::new("/Users/jay/git-work/devLayout");

        assert!(vscode_title_matches_project(
            Some("terminal.rs — devLayout"),
            project_path
        ));
        assert!(vscode_title_matches_project(
            Some("browser.rs — devLayout — Modified"),
            project_path
        ));
        assert!(!vscode_title_matches_project(Some("Welcome"), project_path));
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

    #[test]
    fn antigravity_captures_restores_and_matches_project_path() {
        let platform = Arc::new(FakePlatform::new("/tmp/dev Layout"));
        let adapter = AntigravityAdapter::new(platform.clone());
        let mut saved = window(1, "com.google.antigravity-ide");
        saved.owner = "Antigravity IDE".to_string();
        let state = adapter.capture(&saved).unwrap();
        saved.recovery = Some(state.clone());

        adapter.restore(&saved, &state).unwrap();

        assert_eq!(
            state,
            RecoveryState::Editor {
                project_path: PathBuf::from("/tmp/dev Layout")
            }
        );
        assert_eq!(
            *platform.launches.lock().unwrap(),
            [PathBuf::from("/tmp/dev Layout")]
        );
        assert!(adapter.matches(&saved, &window(99, "COM.GOOGLE.ANTIGRAVITY-IDE")));
    }

    #[test]
    fn antigravity_deep_link_paths_are_percent_encoded() {
        assert_eq!(
            encode_uri_path("/Users/jay/My Project/ctx#one"),
            "/Users/jay/My%20Project/ctx%23one"
        );
    }
}

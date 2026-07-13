use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: u32,
    pub pid: i32,
    pub owner: String,
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<WindowBounds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<RecoveryState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_warning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<DesktopPlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopPlacement {
    pub display_uuid: String,
    pub desktop_ordinal: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecoveryState {
    Editor {
        project_path: PathBuf,
    },
    Terminal {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tabs: Vec<TerminalTabState>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        active_tab: Option<usize>,
    },
    Browser {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tabs: Vec<BrowserTabState>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        active_tab: Option<usize>,
    },
    Generic,
}

impl RecoveryState {
    pub fn kind(&self) -> RecoveryKind {
        match self {
            Self::Editor { .. } => RecoveryKind::Editor,
            Self::Terminal { .. } => RecoveryKind::Terminal,
            Self::Browser { .. } => RecoveryKind::Browser,
            Self::Generic => RecoveryKind::Generic,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryKind {
    Editor,
    Terminal,
    Browser,
    Generic,
}

impl RecoveryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Terminal => "terminal",
            Self::Browser => "browser",
            Self::Generic => "generic",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalTabState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserTabState {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowState {
    Visible,
    Minimized,
    Ambiguous,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WindowStatus {
    pub saved_id: u32,
    pub resolved_id: Option<u32>,
    pub pid: Option<i32>,
    pub owner: String,
    pub title: Option<String>,
    pub state: WindowState,
    pub recovery_kind: Option<RecoveryKind>,
    pub recovery_ready: bool,
    pub recovery_degraded: bool,
    pub recovery_warning: Option<String>,
    pub placement: Option<DesktopPlacement>,
    pub placement_degraded: bool,
    pub placement_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum WindowResolution {
    Resolved(WindowInfo),
    Ambiguous(Vec<WindowInfo>),
    Missing,
}

#[derive(Debug, Error)]
pub enum WindowError {
    #[error(
        "Screen Recording permission is required to identify windows; enable your terminal app in System Settings > Privacy & Security > Screen & System Audio Recording, restart the terminal, then retry"
    )]
    ScreenRecordingPermissionRequired,

    #[error("macOS did not return a window list")]
    ListUnavailable,

    #[error("window discovery is only supported on macOS")]
    UnsupportedPlatform,
}

#[cfg(target_os = "macos")]
pub fn list_windows() -> Result<Vec<WindowInfo>, WindowError> {
    use core_graphics::window::{
        kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly,
    };

    list_windows_with_options(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements)
}

#[cfg(target_os = "macos")]
pub fn list_all_windows() -> Result<Vec<WindowInfo>, WindowError> {
    use core_graphics::window::kCGWindowListOptionAll;

    list_windows_with_options(kCGWindowListOptionAll)
}

#[cfg(target_os = "macos")]
fn list_windows_with_options(options: u32) -> Result<Vec<WindowInfo>, WindowError> {
    use std::collections::BTreeMap;

    use core_foundation::{
        array::CFArray,
        base::{CFType, TCFType},
        dictionary::CFDictionary,
        number::CFNumber,
        string::CFString,
    };
    use core_graphics::access::ScreenCaptureAccess;
    use core_graphics::geometry::CGRect;
    use core_graphics::window::{
        copy_window_info, kCGNullWindowID, kCGWindowBounds, kCGWindowLayer, kCGWindowName,
        kCGWindowNumber, kCGWindowOwnerName, kCGWindowOwnerPID,
    };

    let screen_capture_access = ScreenCaptureAccess;
    if !screen_capture_access.preflight() && !screen_capture_access.request() {
        return Err(WindowError::ScreenRecordingPermissionRequired);
    }

    let raw_windows =
        copy_window_info(options, kCGNullWindowID).ok_or(WindowError::ListUnavailable)?;

    let windows: CFArray<CFDictionary<CFString, CFType>> =
        unsafe { TCFType::wrap_under_get_rule(raw_windows.as_concrete_TypeRef()) };

    let number_key = unsafe { CFString::wrap_under_get_rule(kCGWindowNumber) };
    let pid_key = unsafe { CFString::wrap_under_get_rule(kCGWindowOwnerPID) };
    let owner_key = unsafe { CFString::wrap_under_get_rule(kCGWindowOwnerName) };
    let title_key = unsafe { CFString::wrap_under_get_rule(kCGWindowName) };
    let layer_key = unsafe { CFString::wrap_under_get_rule(kCGWindowLayer) };
    let bounds_key = unsafe { CFString::wrap_under_get_rule(kCGWindowBounds) };

    let mut application_identities = BTreeMap::new();
    let windows = windows
        .iter()
        .filter_map(|dictionary| {
            let layer = dictionary
                .find(&layer_key)?
                .downcast::<CFNumber>()?
                .to_i32()?;

            if layer != 0 {
                return None;
            }

            let bounds = dictionary
                .find(&bounds_key)
                .and_then(|value| value.downcast::<CFDictionary>())
                .and_then(|value| CGRect::from_dict_representation(&value))
                .map(|bounds| WindowBounds {
                    x: bounds.origin.x.round() as i32,
                    y: bounds.origin.y.round() as i32,
                    width: bounds.size.width.round() as i32,
                    height: bounds.size.height.round() as i32,
                })?;

            if !bounds.is_manageable() {
                return None;
            }

            let id = dictionary
                .find(&number_key)?
                .downcast::<CFNumber>()?
                .to_i64()?
                .try_into()
                .ok()?;
            let pid = dictionary
                .find(&pid_key)?
                .downcast::<CFNumber>()?
                .to_i32()?;
            let owner = dictionary
                .find(&owner_key)?
                .downcast::<CFString>()?
                .to_string();
            let title = dictionary
                .find(&title_key)
                .and_then(|value| value.downcast::<CFString>())
                .map(|value| value.to_string())
                .filter(|value| !value.is_empty())?;
            let (bundle_id, application_path) = application_identities
                .entry(pid)
                .or_insert_with(|| application_identity(pid))
                .clone();
            Some(WindowInfo {
                id,
                pid,
                owner,
                title: Some(title),
                bounds: Some(bounds),
                bundle_id,
                application_path,
                recovery: None,
                recovery_warning: None,
                placement: None,
                placement_warning: None,
            })
        })
        .collect();

    Ok(windows)
}

#[cfg(target_os = "macos")]
#[allow(deprecated, unexpected_cfgs)]
fn application_identity(pid: i32) -> (Option<String>, Option<PathBuf>) {
    use std::ffi::CStr;

    use cocoa::{
        appkit::NSRunningApplication,
        base::{id, nil},
        foundation::{NSAutoreleasePool, NSString},
    };
    use objc::{msg_send, sel, sel_impl};

    unsafe fn string_value(value: id) -> Option<String> {
        if value == nil {
            return None;
        }

        let bytes = unsafe { value.UTF8String() };
        if bytes.is_null() {
            return None;
        }

        Some(
            unsafe { CStr::from_ptr(bytes) }
                .to_string_lossy()
                .into_owned(),
        )
    }

    unsafe {
        let pool = NSAutoreleasePool::new(nil);
        let application = NSRunningApplication::runningApplicationWithProcessIdentifier(nil, pid);
        if application == nil {
            pool.drain();
            return (None, None);
        }

        let bundle_identifier: id = msg_send![application, bundleIdentifier];
        let bundle_url: id = msg_send![application, bundleURL];
        let bundle_path: id = if bundle_url == nil {
            nil
        } else {
            msg_send![bundle_url, path]
        };
        let identity = (
            string_value(bundle_identifier),
            string_value(bundle_path).map(PathBuf::from),
        );
        pool.drain();
        identity
    }
}

impl WindowBounds {
    fn is_manageable(self) -> bool {
        const MINIMUM_WINDOW_DIMENSION: i32 = 100;

        self.width >= MINIMUM_WINDOW_DIMENSION && self.height >= MINIMUM_WINDOW_DIMENSION
    }

    pub(crate) fn is_close_to(self, other: Self) -> bool {
        const TOLERANCE: i32 = 8;

        (self.x - other.x).abs() <= TOLERANCE
            && (self.y - other.y).abs() <= TOLERANCE
            && (self.width - other.width).abs() <= TOLERANCE
            && (self.height - other.height).abs() <= TOLERANCE
    }
}

pub fn resolve_window(saved: &WindowInfo, current: &[WindowInfo]) -> WindowResolution {
    if let Some(exact) = current
        .iter()
        .find(|window| window.id == saved.id && window.owner == saved.owner)
    {
        return WindowResolution::Resolved(exact.clone());
    }

    let owner_matches: Vec<_> = current
        .iter()
        .filter(|window| window.owner == saved.owner)
        .collect();

    let mut title_matches: Vec<_> = saved
        .title
        .as_deref()
        .map(|title| {
            owner_matches
                .iter()
                .copied()
                .filter(|window| window.title.as_deref() == Some(title))
                .collect()
        })
        .unwrap_or_default();

    if title_matches.len() == 1 {
        return WindowResolution::Resolved(title_matches.remove(0).clone());
    }

    let bounds_matches = |windows: &[&WindowInfo]| -> Vec<WindowInfo> {
        saved
            .bounds
            .map(|saved_bounds| {
                windows
                    .iter()
                    .copied()
                    .filter(|window| {
                        window
                            .bounds
                            .is_some_and(|bounds| saved_bounds.is_close_to(bounds))
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    };

    if title_matches.len() > 1 {
        let narrowed = bounds_matches(&title_matches);
        return match narrowed.as_slice() {
            [window] => WindowResolution::Resolved(window.clone()),
            [] => WindowResolution::Ambiguous(title_matches.into_iter().cloned().collect()),
            _ => WindowResolution::Ambiguous(narrowed),
        };
    }

    let bounds_matches = bounds_matches(&owner_matches);
    match bounds_matches.as_slice() {
        [window] => WindowResolution::Resolved(window.clone()),
        [] => WindowResolution::Missing,
        _ => WindowResolution::Ambiguous(bounds_matches),
    }
}

pub fn inspect_windows(
    saved_windows: &[WindowInfo],
    all_windows: &[WindowInfo],
    visible_windows: &[WindowInfo],
) -> Vec<WindowStatus> {
    saved_windows
        .iter()
        .map(|saved| {
            let recovery_kind = saved.recovery.as_ref().map(RecoveryState::kind);
            let recovery_ready = match saved.recovery.as_ref() {
                Some(RecoveryState::Generic) => {
                    saved.bundle_id.is_some() || saved.application_path.is_some()
                }
                Some(_) => saved.bundle_id.is_some(),
                None => false,
            };
            let recovery_warning = saved.recovery_warning.clone();
            let recovery_degraded = recovery_warning.is_some();
            let placement = saved.placement.clone();
            let placement_warning = saved.placement_warning.clone();
            let placement_degraded = placement_warning.is_some();

            match resolve_window(saved, all_windows) {
                WindowResolution::Resolved(current) => WindowStatus {
                    saved_id: saved.id,
                    resolved_id: Some(current.id),
                    pid: Some(current.pid),
                    owner: current.owner,
                    title: current.title,
                    state: if visible_windows
                        .iter()
                        .any(|visible| visible.id == current.id)
                    {
                        WindowState::Visible
                    } else {
                        WindowState::Minimized
                    },
                    recovery_kind,
                    recovery_ready,
                    recovery_degraded,
                    recovery_warning,
                    placement,
                    placement_degraded,
                    placement_warning,
                },
                WindowResolution::Ambiguous(_) => WindowStatus {
                    saved_id: saved.id,
                    resolved_id: None,
                    pid: None,
                    owner: saved.owner.clone(),
                    title: saved.title.clone(),
                    state: WindowState::Ambiguous,
                    recovery_kind,
                    recovery_ready,
                    recovery_degraded,
                    recovery_warning,
                    placement,
                    placement_degraded,
                    placement_warning,
                },
                WindowResolution::Missing => WindowStatus {
                    saved_id: saved.id,
                    resolved_id: None,
                    pid: None,
                    owner: saved.owner.clone(),
                    title: saved.title.clone(),
                    state: WindowState::Missing,
                    recovery_kind,
                    recovery_ready,
                    recovery_degraded,
                    recovery_warning,
                    placement,
                    placement_degraded,
                    placement_warning,
                },
            }
        })
        .collect()
}

pub fn reconcile_windows(
    saved_windows: &mut [WindowInfo],
    current_windows: &[WindowInfo],
) -> Vec<WindowStatus> {
    let empty_visible = Vec::new();
    let statuses = inspect_windows(saved_windows, current_windows, &empty_visible);

    for (saved, status) in saved_windows.iter_mut().zip(&statuses) {
        if let Some(resolved_id) = status.resolved_id
            && let Some(current) = current_windows
                .iter()
                .find(|window| window.id == resolved_id)
        {
            refresh_window_fingerprint(saved, current);
        }
    }

    statuses
}

pub(crate) fn refresh_window_fingerprint(saved: &mut WindowInfo, current: &WindowInfo) {
    saved.id = current.id;
    saved.pid = current.pid;
    saved.owner.clone_from(&current.owner);
    saved.title.clone_from(&current.title);
    saved.bounds = current.bounds;

    if current.bundle_id.is_some() {
        saved.bundle_id.clone_from(&current.bundle_id);
    }
    if current.application_path.is_some() {
        saved.application_path.clone_from(&current.application_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_window_bounds_are_manageable() {
        assert!(
            WindowBounds {
                x: 0,
                y: 33,
                width: 1470,
                height: 923,
            }
            .is_manageable()
        );
    }

    #[test]
    fn auxiliary_strips_are_not_manageable_windows() {
        assert!(
            !WindowBounds {
                x: 0,
                y: 33,
                width: 1470,
                height: 32,
            }
            .is_manageable()
        );
    }

    fn window(id: u32, owner: &str, title: &str, x: i32) -> WindowInfo {
        WindowInfo {
            id,
            pid: id as i32,
            owner: owner.to_string(),
            title: Some(title.to_string()),
            bounds: Some(WindowBounds {
                x,
                y: 20,
                width: 800,
                height: 600,
            }),
            bundle_id: None,
            application_path: None,
            recovery: None,
            recovery_warning: None,
            placement: None,
            placement_warning: None,
        }
    }

    #[test]
    fn resolves_stale_id_by_title() {
        let saved = window(1, "Code", "project", 10);
        let current = window(99, "Code", "project", 200);

        assert_eq!(
            resolve_window(&saved, std::slice::from_ref(&current)),
            WindowResolution::Resolved(current)
        );
    }

    #[test]
    fn resolves_changed_title_by_bounds() {
        let saved = window(1, "Code", "old title", 10);
        let current = window(99, "Code", "new title", 12);

        assert_eq!(
            resolve_window(&saved, std::slice::from_ref(&current)),
            WindowResolution::Resolved(current)
        );
    }

    #[test]
    fn reports_ambiguous_geometry() {
        let saved = window(1, "Code", "old", 10);
        let first = window(2, "Code", "first", 10);
        let second = window(3, "Code", "second", 10);

        assert!(matches!(
            resolve_window(&saved, &[first, second]),
            WindowResolution::Ambiguous(_)
        ));
    }

    #[test]
    fn reconciliation_preserves_recovery_metadata() {
        let mut saved = window(1, "Code", "old title", 10);
        saved.bundle_id = Some("com.microsoft.VSCode".to_string());
        saved.application_path = Some(PathBuf::from("/Applications/Visual Studio Code.app"));
        saved.recovery = Some(RecoveryState::Editor {
            project_path: PathBuf::from("/tmp/project"),
        });
        saved.recovery_warning = Some("captured with fallback".to_string());
        saved.placement = Some(DesktopPlacement {
            display_uuid: "Main".to_string(),
            desktop_ordinal: 3,
        });
        saved.placement_warning = Some("using previous placement".to_string());

        let current = window(99, "Code", "new title", 12);
        reconcile_windows(std::slice::from_mut(&mut saved), &[current]);

        assert_eq!(saved.id, 99);
        assert_eq!(saved.title.as_deref(), Some("new title"));
        assert_eq!(saved.bundle_id.as_deref(), Some("com.microsoft.VSCode"));
        assert_eq!(
            saved.application_path.as_deref(),
            Some(std::path::Path::new("/Applications/Visual Studio Code.app"))
        );
        assert_eq!(
            saved.recovery,
            Some(RecoveryState::Editor {
                project_path: PathBuf::from("/tmp/project"),
            })
        );
        assert_eq!(
            saved.recovery_warning.as_deref(),
            Some("captured with fallback")
        );
        assert_eq!(
            saved.placement,
            Some(DesktopPlacement {
                display_uuid: "Main".to_string(),
                desktop_ordinal: 3,
            })
        );
        assert_eq!(
            saved.placement_warning.as_deref(),
            Some("using previous placement")
        );
    }

    #[test]
    fn inspection_reports_recovery_and_placement_readiness() {
        let mut saved = window(1, "Code", "project", 10);
        saved.bundle_id = Some("com.microsoft.VSCode".to_string());
        saved.recovery = Some(RecoveryState::Editor {
            project_path: PathBuf::from("/tmp/project"),
        });
        saved.recovery_warning = Some("captured from document metadata".to_string());
        saved.placement = Some(DesktopPlacement {
            display_uuid: "Main".to_string(),
            desktop_ordinal: 2,
        });
        saved.placement_warning = Some("using previous placement".to_string());

        let status = inspect_windows(std::slice::from_ref(&saved), &[], &[])
            .pop()
            .unwrap();

        assert_eq!(status.recovery_kind, Some(RecoveryKind::Editor));
        assert!(status.recovery_ready);
        assert!(status.recovery_degraded);
        assert_eq!(
            status.recovery_warning.as_deref(),
            Some("captured from document metadata")
        );
        let json = serde_json::to_value(status).unwrap();
        assert_eq!(json["recovery_kind"], "editor");
        assert_eq!(json["recovery_ready"], true);
        assert_eq!(json["recovery_degraded"], true);
        assert_eq!(json["placement"]["display_uuid"], "Main");
        assert_eq!(json["placement"]["desktop_ordinal"], 2);
        assert_eq!(json["placement_degraded"], true);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn list_windows() -> Result<Vec<WindowInfo>, WindowError> {
    Err(WindowError::UnsupportedPlatform)
}

#[cfg(not(target_os = "macos"))]
pub fn list_all_windows() -> Result<Vec<WindowInfo>, WindowError> {
    Err(WindowError::UnsupportedPlatform)
}

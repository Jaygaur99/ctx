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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Error)]
pub enum WindowError {
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
    use core_foundation::{
        array::CFArray,
        base::{CFType, TCFType},
        dictionary::CFDictionary,
        number::CFNumber,
        string::CFString,
    };
    use core_graphics::geometry::CGRect;
    use core_graphics::window::{
        copy_window_info, kCGNullWindowID, kCGWindowBounds, kCGWindowLayer, kCGWindowName,
        kCGWindowNumber, kCGWindowOwnerName, kCGWindowOwnerPID,
    };

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
            Some(WindowInfo {
                id,
                pid,
                owner,
                title: Some(title),
                bounds: Some(bounds),
            })
        })
        .collect();

    Ok(windows)
}

impl WindowBounds {
    fn is_manageable(self) -> bool {
        const MINIMUM_WINDOW_DIMENSION: i32 = 100;

        self.width >= MINIMUM_WINDOW_DIMENSION && self.height >= MINIMUM_WINDOW_DIMENSION
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
}

#[cfg(not(target_os = "macos"))]
pub fn list_windows() -> Result<Vec<WindowInfo>, WindowError> {
    Err(WindowError::UnsupportedPlatform)
}

#[cfg(not(target_os = "macos"))]
pub fn list_all_windows() -> Result<Vec<WindowInfo>, WindowError> {
    Err(WindowError::UnsupportedPlatform)
}

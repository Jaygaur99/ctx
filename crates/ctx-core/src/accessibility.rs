use thiserror::Error;

use crate::{WindowBounds, WindowInfo, list_all_windows};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WindowActionFailure {
    pub id: u32,
    pub owner: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WindowActionReport {
    pub affected: Vec<u32>,
    pub skipped: Vec<WindowActionFailure>,
}

#[derive(Debug, Error)]
pub enum AccessibilityError {
    #[error(
        "Accessibility permission is required; enable ctx in System Settings > Privacy & Security > Accessibility, then retry"
    )]
    PermissionRequired,

    #[error("window {id} is no longer available")]
    WindowMissing { id: u32 },

    #[error(
        "window {id} could not be matched to an accessibility window; candidates: {candidates}"
    )]
    WindowUnresolved { id: u32, candidates: String },

    #[error("window {id} matched multiple accessibility windows")]
    WindowAmbiguous { id: u32 },

    #[error("window {id} does not expose a close button")]
    CloseUnavailable { id: u32 },

    #[cfg(target_os = "macos")]
    #[error("macOS accessibility operation failed for window {id}: {source}")]
    Operation {
        id: u32,
        #[source]
        source: accessibility::Error,
    },

    #[error(transparent)]
    Discovery(#[from] crate::WindowError),

    #[error("window control is only supported on macOS")]
    UnsupportedPlatform,
}

#[cfg(target_os = "macos")]
pub fn request_accessibility_permission() -> bool {
    use accessibility_sys::{
        AXIsProcessTrusted, AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt,
    };
    use core_foundation::{
        base::TCFType, boolean::CFBoolean, dictionary::CFDictionary, string::CFString,
    };

    if unsafe { AXIsProcessTrusted() } {
        return true;
    }

    let prompt_key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
    let options = CFDictionary::from_CFType_pairs(&[(prompt_key, CFBoolean::true_value())]);

    unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) }
}

#[cfg(not(target_os = "macos"))]
pub fn request_accessibility_permission() -> bool {
    false
}

#[cfg(target_os = "macos")]
pub fn minimize_windows(saved_windows: &[WindowInfo]) -> Result<(), AccessibilityError> {
    set_minimized(saved_windows, true)
}

#[cfg(target_os = "macos")]
pub fn minimize_windows_best_effort(
    saved_windows: &[WindowInfo],
) -> Result<WindowActionReport, AccessibilityError> {
    if !request_accessibility_permission() {
        return Err(AccessibilityError::PermissionRequired);
    }

    let current_windows = list_all_windows()?;
    let mut report = WindowActionReport {
        affected: Vec::new(),
        skipped: Vec::new(),
    };

    for saved in saved_windows {
        let result = current_windows
            .iter()
            .find(|window| window.id == saved.id && window.owner == saved.owner)
            .ok_or(AccessibilityError::WindowMissing { id: saved.id })
            .and_then(|current| set_window_minimized(current, true));

        match result {
            Ok(()) => report.affected.push(saved.id),
            Err(error) => report.skipped.push(WindowActionFailure {
                id: saved.id,
                owner: saved.owner.clone(),
                error: error.to_string(),
            }),
        }
    }

    Ok(report)
}

#[cfg(target_os = "macos")]
pub fn restore_windows(saved_windows: &[WindowInfo]) -> Result<(), AccessibilityError> {
    set_minimized(saved_windows, false)
}

#[cfg(target_os = "macos")]
pub fn close_windows(saved_windows: &[WindowInfo]) -> Result<(), AccessibilityError> {
    use accessibility::{
        action::AXUIElementActions, attribute::AXAttribute, ui_element::AXUIElement,
    };
    use accessibility_sys::kAXCloseButtonAttribute;
    use core_foundation::{base::CFType, string::CFString};

    if !request_accessibility_permission() {
        return Err(AccessibilityError::PermissionRequired);
    }

    let current_windows = list_all_windows()?;

    for saved in saved_windows {
        let current = current_windows
            .iter()
            .find(|window| window.id == saved.id && window.owner == saved.owner)
            .ok_or(AccessibilityError::WindowMissing { id: saved.id })?;
        let window = accessibility_window(current, true)?;
        let close_button_attribute =
            AXAttribute::<CFType>::new(&CFString::from_static_string(kAXCloseButtonAttribute));
        let close_button = window
            .attribute(&close_button_attribute)
            .ok()
            .and_then(|value| value.downcast::<AXUIElement>())
            .ok_or(AccessibilityError::CloseUnavailable { id: saved.id })?;

        close_button
            .press()
            .map_err(|source| AccessibilityError::Operation {
                id: saved.id,
                source,
            })?;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn set_minimized(saved_windows: &[WindowInfo], minimized: bool) -> Result<(), AccessibilityError> {
    if !request_accessibility_permission() {
        return Err(AccessibilityError::PermissionRequired);
    }

    let current_windows = list_all_windows()?;

    for saved in saved_windows {
        let current = current_windows
            .iter()
            .find(|window| window.id == saved.id && window.owner == saved.owner)
            .ok_or(AccessibilityError::WindowMissing { id: saved.id })?;
        set_window_minimized(current, minimized)?;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn set_window_minimized(current: &WindowInfo, minimized: bool) -> Result<(), AccessibilityError> {
    use accessibility::{action::AXUIElementActions, attribute::AXAttribute};
    use core_foundation::boolean::CFBoolean;

    let window = accessibility_window(current, !minimized)?;
    window
        .set_attribute(
            &AXAttribute::minimized(),
            if minimized {
                CFBoolean::true_value()
            } else {
                CFBoolean::false_value()
            },
        )
        .map_err(|source| AccessibilityError::Operation {
            id: current.id,
            source,
        })?;

    if !minimized {
        window
            .raise()
            .map_err(|source| AccessibilityError::Operation {
                id: current.id,
                source,
            })?;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn accessibility_window(
    current: &WindowInfo,
    activate_application: bool,
) -> Result<accessibility::ui_element::AXUIElement, AccessibilityError> {
    use std::{process::Command, thread, time::Duration};

    use accessibility::{
        attribute::{AXAttribute, AXUIElementAttributes},
        ui_element::AXUIElement,
    };
    use core_foundation::{
        base::{CFType, TCFType},
        boolean::CFBoolean,
        string::CFString,
    };

    if activate_application {
        let _ = Command::new("/usr/bin/open")
            .args(["-a", &current.owner])
            .status();
        thread::sleep(Duration::from_millis(250));
    }

    let application = AXUIElement::application(current.pid);
    for attribute_name in ["AXManualAccessibility", "AXEnhancedUserInterface"] {
        let attribute = AXAttribute::<CFType>::new(&CFString::from_static_string(attribute_name));
        let _ = application.set_attribute(&attribute, CFBoolean::true_value().as_CFType());
    }
    thread::sleep(Duration::from_millis(250));

    let mut windows = application
        .windows()
        .map_err(|source| AccessibilityError::Operation {
            id: current.id,
            source,
        })?;
    if windows.is_empty() {
        windows = application
            .children()
            .map_err(|source| AccessibilityError::Operation {
                id: current.id,
                source,
            })?;
    }

    let matches: Vec<_> = windows
        .iter()
        .filter(|window| {
            let title_matches = current.title.is_some()
                && window_title(window).as_deref() == current.title.as_deref();
            let bounds_match = current
                .bounds
                .zip(window_bounds(window))
                .is_some_and(|(expected, actual)| expected.is_close_to(actual));

            title_matches || bounds_match
        })
        .collect();

    if matches.len() > 1 {
        return Err(AccessibilityError::WindowAmbiguous { id: current.id });
    }

    matches
        .first()
        .map(|window| (**window).clone())
        .ok_or_else(|| {
            let candidates = windows
                .iter()
                .map(|window| {
                    format!(
                        "title={:?} bounds={:?}",
                        window_title(&window),
                        window_bounds(&window)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");

            AccessibilityError::WindowUnresolved {
                id: current.id,
                candidates,
            }
        })
}

#[cfg(target_os = "macos")]
fn window_title(window: &accessibility::ui_element::AXUIElement) -> Option<String> {
    use accessibility::attribute::AXUIElementAttributes;

    window
        .title()
        .ok()
        .map(|title| title.to_string())
        .filter(|title| !title.is_empty())
}

#[cfg(target_os = "macos")]
fn window_bounds(window: &accessibility::ui_element::AXUIElement) -> Option<WindowBounds> {
    use std::{ffi::c_void, mem::MaybeUninit};

    use accessibility::{attribute::AXAttribute, ui_element::AXUIElement};
    use accessibility_sys::{
        AXValueGetValue, AXValueRef, kAXPositionAttribute, kAXSizeAttribute, kAXValueTypeCGPoint,
        kAXValueTypeCGSize,
    };
    use core_foundation::{
        base::{CFType, TCFType},
        string::CFString,
    };

    #[repr(C)]
    struct Point {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    struct Size {
        width: f64,
        height: f64,
    }

    fn value(window: &AXUIElement, attribute: &'static str) -> Option<CFType> {
        let attribute = AXAttribute::new(&CFString::from_static_string(attribute));
        window.attribute(&attribute).ok()
    }

    let position = value(window, kAXPositionAttribute)?;
    let size = value(window, kAXSizeAttribute)?;
    let mut point = MaybeUninit::<Point>::uninit();
    let mut dimensions = MaybeUninit::<Size>::uninit();
    let read_point = unsafe {
        AXValueGetValue(
            position.as_CFTypeRef() as AXValueRef,
            kAXValueTypeCGPoint,
            point.as_mut_ptr().cast::<c_void>(),
        )
    };
    let read_size = unsafe {
        AXValueGetValue(
            size.as_CFTypeRef() as AXValueRef,
            kAXValueTypeCGSize,
            dimensions.as_mut_ptr().cast::<c_void>(),
        )
    };

    if !read_point || !read_size {
        return None;
    }

    let point = unsafe { point.assume_init() };
    let dimensions = unsafe { dimensions.assume_init() };

    Some(WindowBounds {
        x: point.x.round() as i32,
        y: point.y.round() as i32,
        width: dimensions.width.round() as i32,
        height: dimensions.height.round() as i32,
    })
}

#[cfg(not(target_os = "macos"))]
pub fn minimize_windows(_saved_windows: &[WindowInfo]) -> Result<(), AccessibilityError> {
    Err(AccessibilityError::UnsupportedPlatform)
}

#[cfg(not(target_os = "macos"))]
pub fn minimize_windows_best_effort(
    _saved_windows: &[WindowInfo],
) -> Result<WindowActionReport, AccessibilityError> {
    Err(AccessibilityError::UnsupportedPlatform)
}

#[cfg(not(target_os = "macos"))]
pub fn restore_windows(_saved_windows: &[WindowInfo]) -> Result<(), AccessibilityError> {
    Err(AccessibilityError::UnsupportedPlatform)
}

#[cfg(not(target_os = "macos"))]
pub fn close_windows(_saved_windows: &[WindowInfo]) -> Result<(), AccessibilityError> {
    Err(AccessibilityError::UnsupportedPlatform)
}

use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use crate::{BrowserTabState, RecoveryAdapter, RecoveryError, RecoveryState, WindowInfo};

pub trait FirefoxPlatform: Send + Sync {
    fn capture_tabs(
        &self,
        window: &WindowInfo,
    ) -> Result<(Vec<BrowserTabState>, Option<usize>), RecoveryError>;

    fn launch(
        &self,
        window: &WindowInfo,
        tabs: &[BrowserTabState],
        active_tab: Option<usize>,
    ) -> Result<(), RecoveryError>;
}

#[derive(Debug, Default)]
pub struct SystemFirefoxPlatform;

impl FirefoxPlatform for SystemFirefoxPlatform {
    fn capture_tabs(
        &self,
        window: &WindowInfo,
    ) -> Result<(Vec<BrowserTabState>, Option<usize>), RecoveryError> {
        capture_firefox_tabs(window).map_err(|error| RecoveryError::Capture(error.to_string()))
    }

    fn launch(
        &self,
        window: &WindowInfo,
        tabs: &[BrowserTabState],
        active_tab: Option<usize>,
    ) -> Result<(), RecoveryError> {
        if tabs.is_empty() {
            return Err(RecoveryError::Restore(
                "Firefox recovery contains no tabs".to_string(),
            ));
        }

        let before: std::collections::BTreeSet<_> = crate::list_all_windows()
            .map_err(|error| RecoveryError::Restore(error.to_string()))?
            .into_iter()
            .filter(|candidate| same_bundle_id(window, candidate))
            .map(|candidate| candidate.id)
            .collect();
        let executable = window
            .application_path
            .as_ref()
            .map(|path| path.join("Contents/MacOS/firefox"))
            .filter(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from("/Applications/Firefox.app/Contents/MacOS/firefox"));
        let mut command = Command::new(&executable);
        command.args(["--new-window", &tabs[0].url]);
        for tab in &tabs[1..] {
            command.args(["--new-tab", &tab.url]);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                RecoveryError::Restore(format!(
                    "could not launch Firefox with {}: {error}",
                    executable.display()
                ))
            })?;

        let Some(active_tab) = active_tab else {
            return Ok(());
        };
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            let windows = crate::list_all_windows()
                .map_err(|error| RecoveryError::Restore(error.to_string()))?;
            if let Some(restored) = windows.iter().find(|candidate| {
                same_bundle_id(window, candidate) && !before.contains(&candidate.id)
            }) {
                select_firefox_tab(restored, active_tab)
                    .map_err(|error| RecoveryError::Restore(error.to_string()))?;
                return Ok(());
            }
            thread::sleep(Duration::from_millis(250));
        }

        Err(RecoveryError::Restore(
            "Firefox window did not appear within 20 seconds".to_string(),
        ))
    }
}

pub struct FirefoxAdapter {
    platform: Arc<dyn FirefoxPlatform>,
}

impl FirefoxAdapter {
    pub fn new(platform: Arc<dyn FirefoxPlatform>) -> Self {
        Self { platform }
    }

    pub fn system() -> Self {
        Self::new(Arc::new(SystemFirefoxPlatform))
    }
}

impl RecoveryAdapter for FirefoxAdapter {
    fn capture(&self, window: &WindowInfo) -> Result<RecoveryState, RecoveryError> {
        let (tabs, active_tab) = self.platform.capture_tabs(window)?;
        Ok(RecoveryState::Browser { tabs, active_tab })
    }

    fn restore(&self, window: &WindowInfo, state: &RecoveryState) -> Result<(), RecoveryError> {
        let RecoveryState::Browser { tabs, active_tab } = state else {
            return Err(RecoveryError::Restore(format!(
                "{} does not contain browser recovery state",
                window.owner
            )));
        };
        self.platform.launch(window, tabs, *active_tab)
    }

    fn matches(&self, saved: &WindowInfo, candidate: &WindowInfo) -> bool {
        if !same_bundle_id(saved, candidate) {
            return false;
        }
        let Some(RecoveryState::Browser { tabs, active_tab }) = &saved.recovery else {
            return false;
        };

        let expected_title = active_tab
            .and_then(|index| tabs.get(index))
            .and_then(|tab| tab.title.as_deref())
            .or(saved.title.as_deref());

        expected_title
            .zip(candidate.title.as_deref())
            .is_some_and(|(expected, actual)| firefox_titles_match(expected, actual))
    }
}

fn firefox_titles_match(expected: &str, actual: &str) -> bool {
    fn normalize(title: &str) -> &str {
        title
            .strip_suffix(" — Mozilla Firefox")
            .or_else(|| title.strip_suffix(" - Mozilla Firefox"))
            .unwrap_or(title)
            .trim()
    }

    normalize(expected).eq_ignore_ascii_case(normalize(actual))
}

fn same_bundle_id(first: &WindowInfo, second: &WindowInfo) -> bool {
    first
        .bundle_id
        .as_deref()
        .zip(second.bundle_id.as_deref())
        .is_some_and(|(first, second)| first.eq_ignore_ascii_case(second))
}

#[cfg(target_os = "macos")]
fn capture_firefox_tabs(
    window: &WindowInfo,
) -> Result<(Vec<BrowserTabState>, Option<usize>), crate::AccessibilityError> {
    use accessibility::{action::AXUIElementActions, attribute::AXUIElementAttributes};
    use core_foundation::boolean::CFBoolean;

    if !crate::request_accessibility_permission() {
        return Err(crate::AccessibilityError::PermissionRequired);
    }
    let accessibility_window = crate::accessibility::accessibility_window(window, true)?;
    let elements = descendants(&accessibility_window);
    let tabs: Vec<_> = elements
        .iter()
        .filter(|element| {
            element.role().is_ok_and(|role| role == "AXRadioButton")
                && (element
                    .subrole()
                    .is_ok_and(|subrole| subrole == "AXTabButton")
                    || element
                        .description()
                        .is_ok_and(|description| description == "tab"))
        })
        .cloned()
        .collect();
    if tabs.is_empty() {
        return Err(crate::AccessibilityError::DocumentUnavailable { id: window.id });
    }
    let address = elements
        .iter()
        .find(|element| is_firefox_address(element))
        .cloned();

    let active_tab = tabs.iter().position(|tab| {
        tab.value()
            .ok()
            .and_then(|value| value.downcast::<CFBoolean>())
            .is_some_and(bool::from)
    });
    let mut captured = Vec::with_capacity(tabs.len());
    for tab in &tabs {
        tab.press()
            .map_err(|source| crate::AccessibilityError::Operation {
                id: window.id,
                source,
            })?;
        thread::sleep(Duration::from_millis(100));
        let title = tab
            .title()
            .ok()
            .map(|value| value.to_string())
            .filter(|value| !value.is_empty());
        let url = address
            .as_ref()
            .and_then(firefox_address)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "about:newtab".to_string());
        captured.push(BrowserTabState { url, title });
    }
    if let Some(active_tab) = active_tab {
        tabs[active_tab]
            .press()
            .map_err(|source| crate::AccessibilityError::Operation {
                id: window.id,
                source,
            })?;
    }
    Ok((captured, active_tab))
}

#[cfg(target_os = "macos")]
fn select_firefox_tab(
    window: &WindowInfo,
    active_tab: usize,
) -> Result<(), crate::AccessibilityError> {
    use accessibility::{action::AXUIElementActions, attribute::AXUIElementAttributes};

    let accessibility_window = crate::accessibility::accessibility_window(window, true)?;
    let tabs: Vec<_> = descendants(&accessibility_window)
        .into_iter()
        .filter(|element| {
            element.role().is_ok_and(|role| role == "AXRadioButton")
                && (element
                    .subrole()
                    .is_ok_and(|subrole| subrole == "AXTabButton")
                    || element
                        .description()
                        .is_ok_and(|description| description == "tab"))
        })
        .collect();
    let tab = tabs
        .get(active_tab)
        .ok_or(crate::AccessibilityError::DocumentUnavailable { id: window.id })?;
    tab.press()
        .map_err(|source| crate::AccessibilityError::Operation {
            id: window.id,
            source,
        })
}

#[cfg(target_os = "macos")]
fn descendants(
    root: &accessibility::ui_element::AXUIElement,
) -> Vec<accessibility::ui_element::AXUIElement> {
    use accessibility::attribute::AXUIElementAttributes;

    fn visit(
        element: &accessibility::ui_element::AXUIElement,
        depth: usize,
        output: &mut Vec<accessibility::ui_element::AXUIElement>,
    ) {
        if depth >= 32 {
            return;
        }
        if element.role().is_ok_and(|role| role == "AXWebArea") {
            return;
        }
        if let Ok(children) = element.children() {
            for child in children.iter() {
                output.push(child.clone());
                visit(&child, depth + 1, output);
            }
        }
    }

    let mut output = Vec::new();
    visit(root, 0, &mut output);
    output
}

#[cfg(target_os = "macos")]
fn is_firefox_address(element: &accessibility::ui_element::AXUIElement) -> bool {
    use accessibility::attribute::AXUIElementAttributes;

    element.role().is_ok_and(|role| role == "AXComboBox")
        && element
            .description()
            .is_ok_and(|description| !description.to_string().is_empty())
}

#[cfg(target_os = "macos")]
fn firefox_address(element: &accessibility::ui_element::AXUIElement) -> Option<String> {
    use accessibility::attribute::AXUIElementAttributes;
    use core_foundation::string::CFString;

    element
        .value()
        .ok()?
        .downcast::<CFString>()
        .map(|value| value.to_string())
}

#[cfg(not(target_os = "macos"))]
fn capture_firefox_tabs(
    _window: &WindowInfo,
) -> Result<(Vec<BrowserTabState>, Option<usize>), crate::AccessibilityError> {
    Err(crate::AccessibilityError::UnsupportedPlatform)
}

#[cfg(not(target_os = "macos"))]
fn select_firefox_tab(
    _window: &WindowInfo,
    _active_tab: usize,
) -> Result<(), crate::AccessibilityError> {
    Err(crate::AccessibilityError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct FakePlatform {
        tabs: Vec<BrowserTabState>,
        active_tab: Option<usize>,
        capture_calls: Mutex<usize>,
        launches: Mutex<Vec<Vec<BrowserTabState>>>,
    }

    impl FirefoxPlatform for FakePlatform {
        fn capture_tabs(
            &self,
            _window: &WindowInfo,
        ) -> Result<(Vec<BrowserTabState>, Option<usize>), RecoveryError> {
            *self.capture_calls.lock().unwrap() += 1;
            Ok((self.tabs.clone(), self.active_tab))
        }

        fn launch(
            &self,
            _window: &WindowInfo,
            tabs: &[BrowserTabState],
            _active_tab: Option<usize>,
        ) -> Result<(), RecoveryError> {
            self.launches.lock().unwrap().push(tabs.to_vec());
            Ok(())
        }
    }

    fn tabs() -> Vec<BrowserTabState> {
        vec![
            BrowserTabState {
                url: "https://example.com/one".to_string(),
                title: Some("One".to_string()),
            },
            BrowserTabState {
                url: "https://example.com/two".to_string(),
                title: Some("Two".to_string()),
            },
        ]
    }

    fn window(id: u32) -> WindowInfo {
        WindowInfo {
            id,
            pid: id as i32,
            owner: "Firefox".to_string(),
            title: Some("One".to_string()),
            bounds: None,
            bundle_id: Some("org.mozilla.firefox".to_string()),
            application_path: Some(PathBuf::from("/Applications/Firefox.app")),
            recovery: None,
            recovery_warning: None,
        }
    }

    #[test]
    fn captures_restores_and_matches_all_tabs_and_active_tab() {
        let platform = Arc::new(FakePlatform {
            tabs: tabs(),
            active_tab: Some(1),
            capture_calls: Mutex::new(0),
            launches: Mutex::new(Vec::new()),
        });
        let adapter = FirefoxAdapter::new(platform.clone());
        let mut saved = window(1);
        let state = adapter.capture(&saved).unwrap();
        saved.recovery = Some(state.clone());

        adapter.restore(&saved, &state).unwrap();

        assert_eq!(
            state,
            RecoveryState::Browser {
                tabs: tabs(),
                active_tab: Some(1)
            }
        );
        assert_eq!(*platform.launches.lock().unwrap(), [tabs()]);
        let mut recovered = window(99);
        recovered.title = Some("Two — Mozilla Firefox".to_string());
        assert!(adapter.matches(&saved, &recovered));
        recovered.title = Some("Mozilla Firefox".to_string());
        assert!(!adapter.matches(&saved, &recovered));
        assert_eq!(*platform.capture_calls.lock().unwrap(), 1);
    }
}

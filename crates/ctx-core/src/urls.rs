use std::{
    collections::BTreeSet,
    process::{Command, Stdio},
};

use serde::Serialize;
use thiserror::Error;

use crate::{RecoveryState, RuntimeState, Workspace};

#[derive(Debug, Error)]
pub enum UrlError {
    #[error("invalid URL '{input}': {source}")]
    Invalid {
        input: String,
        #[source]
        source: url::ParseError,
    },

    #[error("URL '{url}' uses unsupported scheme '{scheme}'; only http and https are allowed")]
    UnsupportedScheme { url: String, scheme: String },

    #[error("URL '{url}' contains embedded credentials, which are not allowed")]
    CredentialsNotAllowed { url: String },

    #[error("could not determine the current macOS boot session: {0}")]
    BootSession(String),

    #[error("could not open URL '{url}': {message}")]
    Open { url: String, message: String },

    #[error("URL '{url}' is not configured")]
    NotConfigured { url: String },

    #[error("URL launching is only supported on macOS")]
    UnsupportedPlatform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceUrlState {
    Pending,
    Opened,
    RecoveryManaged,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceUrlStatus {
    pub url: String,
    pub state: WorkspaceUrlState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UrlLaunchFailure {
    pub url: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ConfiguredUrlUpdate {
    pub added: Vec<String>,
    pub already_present: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UrlLaunchReport {
    pub workspace: String,
    pub opened: Vec<String>,
    pub already_opened: Vec<String>,
    pub recovery_managed: Vec<String>,
    pub failed: Vec<UrlLaunchFailure>,
}

impl UrlLaunchReport {
    pub fn has_failures(&self) -> bool {
        !self.failed.is_empty()
    }

    pub fn system_failure(workspace: &str, urls: &[String], error: impl ToString) -> Self {
        let error = error.to_string();
        Self {
            workspace: workspace.to_string(),
            failed: urls
                .iter()
                .map(|url| UrlLaunchFailure {
                    url: url.clone(),
                    error: error.clone(),
                })
                .collect(),
            ..Self::default()
        }
    }
}

pub trait UrlOpener {
    fn open(&mut self, url: &str) -> Result<(), UrlError>;
}

#[derive(Debug, Default)]
pub struct SystemUrlOpener;

impl UrlOpener for SystemUrlOpener {
    fn open(&mut self, url: &str) -> Result<(), UrlError> {
        open_system_url(url)
    }
}

pub fn normalize_url(input: &str) -> Result<String, UrlError> {
    let trimmed = input.trim();
    let mut parsed = url::Url::parse(trimmed).map_err(|source| UrlError::Invalid {
        input: input.to_string(),
        source,
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(UrlError::UnsupportedScheme {
            url: input.to_string(),
            scheme: parsed.scheme().to_string(),
        });
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(UrlError::CredentialsNotAllowed {
            url: input.to_string(),
        });
    }
    if parsed.fragment() == Some("") {
        parsed.set_fragment(None);
    }
    Ok(parsed.to_string())
}

pub fn normalize_urls(inputs: &[String]) -> Result<Vec<String>, UrlError> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::with_capacity(inputs.len());
    for input in inputs {
        let url = normalize_url(input)?;
        if seen.insert(url.clone()) {
            normalized.push(url);
        }
    }
    Ok(normalized)
}

pub fn add_urls_to_workspace(
    workspace: &mut Workspace,
    inputs: &[String],
) -> Result<ConfiguredUrlUpdate, UrlError> {
    let normalized = normalize_urls(inputs)?;
    let mut existing: BTreeSet<_> = workspace
        .urls
        .iter()
        .filter_map(|url| normalize_url(url).ok())
        .collect();
    let mut update = ConfiguredUrlUpdate::default();
    for url in normalized {
        if existing.insert(url.clone()) {
            workspace.urls.push(url.clone());
            update.added.push(url);
        } else {
            update.already_present.push(url);
        }
    }
    Ok(update)
}

pub fn remove_urls_from_workspace(
    workspace: &mut Workspace,
    inputs: &[String],
) -> Result<Vec<String>, UrlError> {
    let requested = normalize_urls(inputs)?;
    let configured: BTreeSet<_> = workspace
        .urls
        .iter()
        .filter_map(|url| normalize_url(url).ok())
        .collect();
    if let Some(url) = requested.iter().find(|url| !configured.contains(*url)) {
        return Err(UrlError::NotConfigured { url: url.clone() });
    }
    let requested_set: BTreeSet<_> = requested.iter().cloned().collect();
    workspace.urls.retain(|configured| {
        normalize_url(configured)
            .ok()
            .is_none_or(|url| !requested_set.contains(&url))
    });
    Ok(requested)
}

pub fn workspace_url_statuses(
    workspace_name: &str,
    workspace: &Workspace,
    state: &RuntimeState,
    boot_id: &str,
) -> Vec<WorkspaceUrlStatus> {
    let recovery_urls = recovery_managed_urls(workspace);
    workspace
        .urls
        .iter()
        .map(|configured| match normalize_url(configured) {
            Ok(url) if recovery_urls.contains(&url) => WorkspaceUrlStatus {
                url,
                state: WorkspaceUrlState::RecoveryManaged,
                error: None,
            },
            Ok(url) if state.url_was_opened(boot_id, workspace_name, &url) => WorkspaceUrlStatus {
                url,
                state: WorkspaceUrlState::Opened,
                error: None,
            },
            Ok(url) => match state.url_failure(boot_id, workspace_name, &url) {
                Some(error) => WorkspaceUrlStatus {
                    url,
                    state: WorkspaceUrlState::Failed,
                    error: Some(error.to_string()),
                },
                None => WorkspaceUrlStatus {
                    url,
                    state: WorkspaceUrlState::Pending,
                    error: None,
                },
            },
            Err(error) => WorkspaceUrlStatus {
                url: configured.clone(),
                state: WorkspaceUrlState::Failed,
                error: Some(error.to_string()),
            },
        })
        .collect()
}

pub fn launch_workspace_urls(
    workspace_name: &str,
    workspace: &Workspace,
    state: &mut RuntimeState,
    boot_id: &str,
    force: bool,
    opener: &mut dyn UrlOpener,
) -> UrlLaunchReport {
    state.ensure_url_boot_session(boot_id);
    let recovery_urls = recovery_managed_urls(workspace);
    let mut report = UrlLaunchReport {
        workspace: workspace_name.to_string(),
        ..UrlLaunchReport::default()
    };

    for configured in &workspace.urls {
        let url = match normalize_url(configured) {
            Ok(url) => url,
            Err(error) => {
                report.failed.push(UrlLaunchFailure {
                    url: configured.clone(),
                    error: error.to_string(),
                });
                continue;
            }
        };

        if !force && recovery_urls.contains(&url) {
            state.clear_url_failure(workspace_name, &url);
            report.recovery_managed.push(url);
            continue;
        }
        if !force && state.url_was_opened(boot_id, workspace_name, &url) {
            report.already_opened.push(url);
            continue;
        }

        match opener.open(&url) {
            Ok(()) => {
                state.mark_url_opened(workspace_name, &url);
                report.opened.push(url);
            }
            Err(error) => {
                let message = error.to_string();
                state.mark_url_failed(workspace_name, &url, &message);
                report.failed.push(UrlLaunchFailure {
                    url,
                    error: message,
                });
            }
        }
    }

    report
}

pub fn recovery_managed_urls(workspace: &Workspace) -> BTreeSet<String> {
    workspace
        .windows
        .iter()
        .filter(|window| {
            window
                .bundle_id
                .as_deref()
                .is_some_and(|bundle| bundle.eq_ignore_ascii_case("org.mozilla.firefox"))
        })
        .filter_map(|window| match window.recovery.as_ref() {
            Some(RecoveryState::Browser { tabs, .. }) => Some(tabs),
            _ => None,
        })
        .flatten()
        .filter_map(|tab| normalize_url(&tab.url).ok())
        .collect()
}

#[cfg(target_os = "macos")]
pub fn current_boot_id() -> Result<String, UrlError> {
    let output = Command::new("/usr/sbin/sysctl")
        .args(["-n", "kern.boottime"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| UrlError::BootSession(error.to_string()))?;
    if !output.status.success() {
        return Err(UrlError::BootSession(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    let boot_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if boot_id.is_empty() {
        Err(UrlError::BootSession(
            "sysctl returned an empty boot time".to_string(),
        ))
    } else {
        Ok(boot_id)
    }
}

#[cfg(not(target_os = "macos"))]
pub fn current_boot_id() -> Result<String, UrlError> {
    Err(UrlError::UnsupportedPlatform)
}

#[cfg(target_os = "macos")]
fn open_system_url(url: &str) -> Result<(), UrlError> {
    let output = Command::new("/usr/bin/open")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| UrlError::Open {
            url: url.to_string(),
            message: error.to_string(),
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(UrlError::Open {
            url: url.to_string(),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

#[cfg(not(target_os = "macos"))]
fn open_system_url(_url: &str) -> Result<(), UrlError> {
    Err(UrlError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use super::*;
    use crate::{BrowserTabState, Service, WindowInfo};

    #[derive(Default)]
    struct FakeOpener {
        opened: Vec<String>,
        fail: BTreeSet<String>,
    }

    impl UrlOpener for FakeOpener {
        fn open(&mut self, url: &str) -> Result<(), UrlError> {
            if self.fail.contains(url) {
                return Err(UrlError::Open {
                    url: url.to_string(),
                    message: "simulated failure".to_string(),
                });
            }
            self.opened.push(url.to_string());
            Ok(())
        }
    }

    fn workspace(urls: &[&str]) -> Workspace {
        Workspace {
            path: Some(PathBuf::from("/tmp/project")),
            services: Vec::<Service>::new(),
            urls: urls.iter().map(|url| (*url).to_string()).collect(),
            windows: Vec::new(),
        }
    }

    fn firefox_window(url: &str) -> WindowInfo {
        WindowInfo {
            id: 42,
            pid: 100,
            owner: "Firefox".to_string(),
            title: Some("Firefox".to_string()),
            bounds: None,
            bundle_id: Some("org.mozilla.firefox".to_string()),
            application_path: None,
            recovery: Some(RecoveryState::Browser {
                tabs: vec![BrowserTabState {
                    url: url.to_string(),
                    title: None,
                }],
                active_tab: Some(0),
            }),
            recovery_warning: None,
            placement: None,
            placement_warning: None,
        }
    }

    #[test]
    fn normalizes_http_urls_and_deduplicates_inputs() {
        assert_eq!(
            normalize_urls(&[
                " HTTPS://Example.COM/docs ".to_string(),
                "https://example.com/docs".to_string(),
            ])
            .unwrap(),
            ["https://example.com/docs"]
        );
    }

    #[test]
    fn rejects_unsupported_schemes_and_credentials() {
        assert!(matches!(
            normalize_url("file:///tmp/context"),
            Err(UrlError::UnsupportedScheme { .. })
        ));
        assert!(matches!(
            normalize_url("https://user:secret@example.com"),
            Err(UrlError::CredentialsNotAllowed { .. })
        ));
    }

    #[test]
    fn adding_urls_is_normalized_and_idempotent() {
        let mut workspace = workspace(&["https://example.com/docs"]);

        let update = add_urls_to_workspace(
            &mut workspace,
            &[
                "HTTPS://EXAMPLE.COM/docs".to_string(),
                "http://localhost:3000".to_string(),
            ],
        )
        .unwrap();

        assert_eq!(update.already_present, ["https://example.com/docs"]);
        assert_eq!(update.added, ["http://localhost:3000/"]);
        assert_eq!(workspace.urls.len(), 2);
    }

    #[test]
    fn removal_is_atomic_when_any_url_is_missing() {
        let mut workspace = workspace(&["https://example.com/one", "https://example.com/two"]);
        let before = workspace.urls.clone();

        let error = remove_urls_from_workspace(
            &mut workspace,
            &[
                "https://example.com/one".to_string(),
                "https://example.com/missing".to_string(),
            ],
        )
        .unwrap_err();

        assert!(matches!(error, UrlError::NotConfigured { .. }));
        assert_eq!(workspace.urls, before);
    }

    #[test]
    fn removal_preserves_unrelated_and_invalid_legacy_entries() {
        let mut workspace = workspace(&[
            "https://example.com/one",
            "https://example.com/two",
            "legacy custom value",
        ]);

        let removed =
            remove_urls_from_workspace(&mut workspace, &["https://example.com/one".to_string()])
                .unwrap();

        assert_eq!(removed, ["https://example.com/one"]);
        assert_eq!(
            workspace.urls,
            [
                "https://example.com/two".to_string(),
                "legacy custom value".to_string(),
            ]
        );
    }

    #[test]
    fn opens_each_url_once_per_boot_and_resets_on_boot_change() {
        let workspace = workspace(&["https://example.com/one", "https://example.com/two"]);
        let mut state = RuntimeState::default();
        let mut opener = FakeOpener::default();

        let first = launch_workspace_urls(
            "coding",
            &workspace,
            &mut state,
            "boot-1",
            false,
            &mut opener,
        );
        let repeated = launch_workspace_urls(
            "coding",
            &workspace,
            &mut state,
            "boot-1",
            false,
            &mut opener,
        );
        let next_boot = launch_workspace_urls(
            "coding",
            &workspace,
            &mut state,
            "boot-2",
            false,
            &mut opener,
        );

        assert_eq!(first.opened.len(), 2);
        assert_eq!(repeated.already_opened.len(), 2);
        assert_eq!(next_boot.opened.len(), 2);
        assert_eq!(opener.opened.len(), 4);
    }

    #[test]
    fn failures_are_retried_and_only_successes_are_marked() {
        let workspace = workspace(&["https://example.com/ok", "https://example.com/fail"]);
        let mut state = RuntimeState::default();
        let mut opener = FakeOpener {
            fail: BTreeSet::from(["https://example.com/fail".to_string()]),
            ..FakeOpener::default()
        };

        let first = launch_workspace_urls(
            "coding",
            &workspace,
            &mut state,
            "boot-1",
            false,
            &mut opener,
        );
        opener.fail.clear();
        let second = launch_workspace_urls(
            "coding",
            &workspace,
            &mut state,
            "boot-1",
            false,
            &mut opener,
        );

        assert_eq!(first.opened, ["https://example.com/ok"]);
        assert_eq!(first.failed.len(), 1);
        assert_eq!(second.already_opened, ["https://example.com/ok"]);
        assert_eq!(second.opened, ["https://example.com/fail"]);
    }

    #[test]
    fn firefox_recovery_satisfies_a_url_unless_forced() {
        let mut workspace = workspace(&["https://example.com/docs"]);
        workspace
            .windows
            .push(firefox_window("https://example.com/docs"));
        let mut state = RuntimeState::default();
        let mut opener = FakeOpener::default();

        let normal = launch_workspace_urls(
            "research",
            &workspace,
            &mut state,
            "boot-1",
            false,
            &mut opener,
        );
        let forced = launch_workspace_urls(
            "research",
            &workspace,
            &mut state,
            "boot-1",
            true,
            &mut opener,
        );

        assert_eq!(normal.recovery_managed, ["https://example.com/docs"]);
        assert!(normal.opened.is_empty());
        assert_eq!(forced.opened, ["https://example.com/docs"]);
    }

    #[test]
    fn statuses_report_pending_opened_recovery_and_failed() {
        let mut workspace = workspace(&[
            "https://example.com/pending",
            "https://example.com/opened",
            "https://example.com/recovered",
            "not a URL",
        ]);
        workspace
            .windows
            .push(firefox_window("https://example.com/recovered"));
        let mut state = RuntimeState::default();
        state.ensure_url_boot_session("boot-1");
        state.mark_url_opened("research", "https://example.com/opened");

        let statuses = workspace_url_statuses("research", &workspace, &state, "boot-1");

        assert_eq!(statuses[0].state, WorkspaceUrlState::Pending);
        assert_eq!(statuses[1].state, WorkspaceUrlState::Opened);
        assert_eq!(statuses[2].state, WorkspaceUrlState::RecoveryManaged);
        assert_eq!(statuses[3].state, WorkspaceUrlState::Failed);
    }

    #[test]
    fn report_system_failure_covers_each_configured_url() {
        let report = UrlLaunchReport::system_failure(
            "coding",
            &["https://one.example".into(), "https://two.example".into()],
            "boot lookup failed",
        );

        assert_eq!(report.workspace, "coding");
        assert_eq!(report.failed.len(), 2);
        assert!(report.opened.is_empty());
    }

    #[test]
    fn workspace_helper_does_not_require_services() {
        let workspace = Workspace {
            path: None,
            services: Vec::new(),
            urls: vec!["https://example.com".into()],
            windows: Vec::new(),
        };
        let config = crate::Config {
            version: 1,
            ignored_windows: Vec::new(),
            workspaces: BTreeMap::from([("coding".to_string(), workspace)]),
        };

        assert_eq!(config.workspace("coding").unwrap().urls.len(), 1);
    }
}

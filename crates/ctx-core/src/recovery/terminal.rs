use std::{
    collections::{BTreeMap, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    process::{Command, Stdio},
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::{RecoveryAdapter, RecoveryError, RecoveryState, TerminalTabState, WindowInfo};

pub trait WarpPlatform: Send + Sync {
    fn capture_tabs(
        &self,
        window: &WindowInfo,
    ) -> Result<(Vec<TerminalTabState>, Option<usize>), RecoveryError>;

    fn launch(
        &self,
        window: &WindowInfo,
        tabs: &[TerminalTabState],
        active_tab: Option<usize>,
    ) -> Result<(), RecoveryError>;
}

#[derive(Debug, Default)]
pub struct SystemWarpPlatform;

impl WarpPlatform for SystemWarpPlatform {
    fn capture_tabs(
        &self,
        window: &WindowInfo,
    ) -> Result<(Vec<TerminalTabState>, Option<usize>), RecoveryError> {
        let database = warp_database_path()?;
        let output = Command::new("/usr/bin/sqlite3")
            .args(["-readonly", "-json"])
            .arg(&database)
            .arg(WARP_SESSION_QUERY)
            .output()
            .map_err(|error| {
                RecoveryError::Capture(format!(
                    "could not read Warp session database {}: {error}",
                    database.display()
                ))
            })?;
        if !output.status.success() {
            return Err(RecoveryError::Capture(format!(
                "Warp session query failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        let rows: Vec<WarpSessionRow> =
            serde_json::from_slice(&output.stdout).map_err(|error| {
                RecoveryError::Capture(format!("could not parse Warp session data: {error}"))
            })?;
        capture_window_from_rows(window, rows)
    }

    fn launch(
        &self,
        _window: &WindowInfo,
        tabs: &[TerminalTabState],
        active_tab: Option<usize>,
    ) -> Result<(), RecoveryError> {
        if tabs.is_empty() {
            return Err(RecoveryError::Restore(
                "Warp recovery contains no tabs".to_string(),
            ));
        }

        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| RecoveryError::Restore("HOME is not set".to_string()))?;
        let directory = home.join(".warp/launch_configurations");
        fs::create_dir_all(&directory).map_err(|error| {
            RecoveryError::Restore(format!(
                "could not create Warp launch configuration directory {}: {error}",
                directory.display()
            ))
        })?;

        let config = WarpLaunchConfiguration::new(tabs, active_tab);
        let yaml = serde_yaml::to_string(&config).map_err(|error| {
            RecoveryError::Restore(format!("could not serialize Warp launch config: {error}"))
        })?;
        let path = directory.join(config.file_name());
        let temporary = path.with_extension("yaml.tmp");
        fs::write(&temporary, yaml).map_err(|error| {
            RecoveryError::Restore(format!(
                "could not write Warp launch config {}: {error}",
                temporary.display()
            ))
        })?;
        fs::rename(&temporary, &path).map_err(|error| {
            RecoveryError::Restore(format!(
                "could not install Warp launch config {}: {error}",
                path.display()
            ))
        })?;

        let uri = format!(
            "warp://launch/{}",
            encode_uri_component(&path.to_string_lossy())
        );
        Command::new("/usr/bin/open")
            .arg(&uri)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|error| {
                RecoveryError::Restore(format!("could not open Warp launch URI {uri}: {error}"))
            })
    }
}

pub struct WarpAdapter {
    platform: Arc<dyn WarpPlatform>,
}

impl WarpAdapter {
    pub fn new(platform: Arc<dyn WarpPlatform>) -> Self {
        Self { platform }
    }

    pub fn system() -> Self {
        Self::new(Arc::new(SystemWarpPlatform))
    }
}

impl RecoveryAdapter for WarpAdapter {
    fn capture(&self, window: &WindowInfo) -> Result<RecoveryState, RecoveryError> {
        let (tabs, active_tab) = self.platform.capture_tabs(window)?;
        Ok(RecoveryState::Terminal { tabs, active_tab })
    }

    fn restore(&self, window: &WindowInfo, state: &RecoveryState) -> Result<(), RecoveryError> {
        let RecoveryState::Terminal { tabs, active_tab } = state else {
            return Err(RecoveryError::Restore(format!(
                "{} does not contain terminal recovery state",
                window.owner
            )));
        };
        self.platform.launch(window, tabs, *active_tab)
    }

    fn matches(&self, saved: &WindowInfo, candidate: &WindowInfo) -> bool {
        if !same_bundle_id(saved, candidate) {
            return false;
        }
        let Some(RecoveryState::Terminal {
            tabs: saved_tabs, ..
        }) = &saved.recovery
        else {
            return false;
        };
        self.platform
            .capture_tabs(candidate)
            .is_ok_and(|(candidate_tabs, _)| candidate_tabs == *saved_tabs)
    }
}

const WARP_SESSION_QUERY: &str = "
SELECT w.id AS window_id, w.active_tab_index, w.window_width, w.window_height,
       w.origin_x, t.id AS tab_id, t.custom_title AS title, tp.cwd, pl.is_focused
FROM windows w
JOIN tabs t ON t.window_id = w.id
JOIN pane_nodes pn ON pn.tab_id = t.id
JOIN pane_leaves pl ON pl.pane_node_id = pn.id
JOIN terminal_panes tp ON tp.id = pn.id
ORDER BY w.id, t.id, pl.is_focused DESC, pn.id
";

#[derive(Debug, Deserialize)]
struct WarpSessionRow {
    window_id: i64,
    active_tab_index: usize,
    window_width: Option<f64>,
    window_height: Option<f64>,
    origin_x: Option<f64>,
    tab_id: i64,
    title: Option<String>,
    cwd: Option<PathBuf>,
    is_focused: i64,
}

fn capture_window_from_rows(
    window: &WindowInfo,
    rows: Vec<WarpSessionRow>,
) -> Result<(Vec<TerminalTabState>, Option<usize>), RecoveryError> {
    let mut windows: BTreeMap<i64, Vec<WarpSessionRow>> = BTreeMap::new();
    for row in rows {
        windows.entry(row.window_id).or_default().push(row);
    }

    let matching_ids: Vec<_> = windows
        .iter()
        .filter(|(_, rows)| warp_geometry_matches(window, &rows[0]))
        .map(|(id, _)| *id)
        .collect();
    let selected_id = match matching_ids.as_slice() {
        [id] => *id,
        [] if windows.len() == 1 => *windows.keys().next().expect("length checked"),
        [] => {
            return Err(RecoveryError::Capture(format!(
                "could not match Warp window {} to its saved session",
                window.id
            )));
        }
        _ => {
            return Err(RecoveryError::Capture(format!(
                "Warp window {} matched multiple saved sessions",
                window.id
            )));
        }
    };
    let rows = windows.remove(&selected_id).expect("selected key exists");
    let active_tab = rows.first().map(|row| row.active_tab_index);
    let mut tabs: BTreeMap<i64, TerminalTabState> = BTreeMap::new();
    for row in rows {
        let tab = TerminalTabState {
            working_directory: row.cwd,
            title: row.title,
        };
        if row.is_focused != 0 {
            tabs.insert(row.tab_id, tab);
        } else {
            tabs.entry(row.tab_id).or_insert(tab);
        }
    }

    if tabs.is_empty() {
        return Err(RecoveryError::Capture(format!(
            "Warp window {} has no terminal tabs",
            window.id
        )));
    }
    Ok((tabs.into_values().collect(), active_tab))
}

fn warp_geometry_matches(window: &WindowInfo, row: &WarpSessionRow) -> bool {
    const TOLERANCE: f64 = 8.0;
    let Some(bounds) = window.bounds else {
        return false;
    };
    row.window_width
        .zip(row.window_height)
        .is_some_and(|(width, height)| {
            (width - f64::from(bounds.width)).abs() <= TOLERANCE
                && (height - f64::from(bounds.height)).abs() <= TOLERANCE
        })
        && row
            .origin_x
            .is_none_or(|x| (x - f64::from(bounds.x)).abs() <= TOLERANCE)
}

fn warp_database_path() -> Result<PathBuf, RecoveryError> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| RecoveryError::Capture("HOME is not set".to_string()))?;
    Ok(home.join(
        "Library/Group Containers/2BBY89MBSN.dev.warp/Library/Application Support/dev.warp.Warp-Stable/warp.sqlite",
    ))
}

fn same_bundle_id(first: &WindowInfo, second: &WindowInfo) -> bool {
    first
        .bundle_id
        .as_deref()
        .zip(second.bundle_id.as_deref())
        .is_some_and(|(first, second)| first.eq_ignore_ascii_case(second))
}

#[derive(Debug, Serialize)]
struct WarpLaunchConfiguration {
    name: String,
    active_window_index: usize,
    windows: Vec<WarpLaunchWindow>,
}

#[derive(Debug, Serialize)]
struct WarpLaunchWindow {
    #[serde(skip_serializing_if = "Option::is_none")]
    active_tab_index: Option<usize>,
    tabs: Vec<WarpLaunchTab>,
}

#[derive(Debug, Serialize)]
struct WarpLaunchTab {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    layout: WarpLaunchLayout,
}

#[derive(Debug, Serialize)]
struct WarpLaunchLayout {
    cwd: PathBuf,
}

impl WarpLaunchConfiguration {
    fn new(tabs: &[TerminalTabState], active_tab: Option<usize>) -> Self {
        let mut hasher = DefaultHasher::new();
        for tab in tabs {
            tab.working_directory.hash(&mut hasher);
            tab.title.hash(&mut hasher);
        }
        let hash = hasher.finish();
        Self {
            name: format!("Ctx Recovery {hash:016x}"),
            active_window_index: 0,
            windows: vec![WarpLaunchWindow {
                active_tab_index: active_tab,
                tabs: tabs
                    .iter()
                    .map(|tab| WarpLaunchTab {
                        title: tab.title.clone(),
                        layout: WarpLaunchLayout {
                            cwd: tab.working_directory.clone().unwrap_or_default(),
                        },
                    })
                    .collect(),
            }],
        }
    }

    fn file_name(&self) -> String {
        format!("{}.yaml", self.name.to_lowercase().replace(' ', "-"))
    }
}

fn encode_uri_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
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
    use crate::WindowBounds;

    struct FakePlatform {
        tabs: Vec<TerminalTabState>,
        active_tab: Option<usize>,
        launches: Mutex<Vec<Vec<TerminalTabState>>>,
    }

    impl WarpPlatform for FakePlatform {
        fn capture_tabs(
            &self,
            _window: &WindowInfo,
        ) -> Result<(Vec<TerminalTabState>, Option<usize>), RecoveryError> {
            Ok((self.tabs.clone(), self.active_tab))
        }

        fn launch(
            &self,
            _window: &WindowInfo,
            tabs: &[TerminalTabState],
            _active_tab: Option<usize>,
        ) -> Result<(), RecoveryError> {
            self.launches.lock().unwrap().push(tabs.to_vec());
            Ok(())
        }
    }

    fn tabs() -> Vec<TerminalTabState> {
        vec![
            TerminalTabState {
                working_directory: Some(PathBuf::from("/tmp/api")),
                title: Some("API".to_string()),
            },
            TerminalTabState {
                working_directory: Some(PathBuf::from("/tmp/web")),
                title: Some("Web".to_string()),
            },
        ]
    }

    fn window(id: u32) -> WindowInfo {
        WindowInfo {
            id,
            pid: id as i32,
            owner: "Warp".to_string(),
            title: Some("devLayout".to_string()),
            bounds: Some(WindowBounds {
                x: 0,
                y: 33,
                width: 1470,
                height: 923,
            }),
            bundle_id: Some("dev.warp.Warp-Stable".to_string()),
            application_path: Some(PathBuf::from("/Applications/Warp.app")),
            recovery: None,
            recovery_warning: None,
        }
    }

    #[test]
    fn captures_restores_and_matches_tabs_without_commands() {
        let platform = Arc::new(FakePlatform {
            tabs: tabs(),
            active_tab: Some(1),
            launches: Mutex::new(Vec::new()),
        });
        let adapter = WarpAdapter::new(platform.clone());
        let mut saved = window(1);
        let state = adapter.capture(&saved).unwrap();
        saved.recovery = Some(state.clone());

        adapter.restore(&saved, &state).unwrap();

        assert_eq!(
            state,
            RecoveryState::Terminal {
                tabs: tabs(),
                active_tab: Some(1)
            }
        );
        assert_eq!(*platform.launches.lock().unwrap(), [tabs()]);
        assert!(adapter.matches(&saved, &window(99)));
    }

    #[test]
    fn generated_launch_config_contains_no_commands() {
        let config = WarpLaunchConfiguration::new(&tabs(), Some(1));
        let yaml = serde_yaml::to_string(&config).unwrap();

        assert!(yaml.contains("cwd: /tmp/api"));
        assert!(yaml.contains("active_tab_index: 1"));
        assert!(!yaml.contains("commands"));
        assert!(!yaml.contains("exec"));
    }

    #[test]
    fn sqlite_rows_are_grouped_into_tabs_for_the_matching_window() {
        let rows = vec![row(7, 10, "API", "/tmp/api"), row(7, 11, "Web", "/tmp/web")];

        assert_eq!(
            capture_window_from_rows(&window(1), rows).unwrap(),
            (tabs(), Some(1))
        );
    }

    fn row(window_id: i64, tab_id: i64, title: &str, cwd: &str) -> WarpSessionRow {
        WarpSessionRow {
            window_id,
            active_tab_index: 1,
            window_width: Some(1470.0),
            window_height: Some(923.0),
            origin_x: Some(0.0),
            tab_id,
            title: Some(title.to_string()),
            cwd: Some(PathBuf::from(cwd)),
            is_focused: 1,
        }
    }

    #[test]
    fn launch_uri_uses_component_encoding() {
        assert_eq!(
            encode_uri_component("/Users/jay/.warp/ctx one.yaml"),
            "%2FUsers%2Fjay%2F.warp%2Fctx%20one.yaml"
        );
    }
}

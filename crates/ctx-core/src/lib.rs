pub mod accessibility;
pub mod application;
pub mod config;
pub mod mutation_lock;
pub mod paths;
pub mod persistence;
pub mod recovery;
pub mod runtime;
pub mod snapshot;
pub mod spaces;
pub mod switcher;
pub mod urls;
pub mod windows;

pub use accessibility::{
    AccessibilityError, WindowActionFailure, WindowActionReport, accessibility_permission_granted,
    close_windows, minimize_windows, minimize_windows_best_effort,
    request_accessibility_permission, restore_windows,
};
pub use application::{
    AddWindowsReport, CreateWorkspaceReport, CtxApp, CtxAppError, CtxOverview,
    DeleteWorkspacesReport, EditWorkspaceReport, HideAllReport, WindowCandidate,
    WindowPickerOverview, WorkspaceOverview,
};
pub use config::{Config, ConfigError, Service, Workspace};
pub use mutation_lock::{
    DEFAULT_MUTATION_LOCK_TIMEOUT, MutationGuard, MutationLockError, acquire_mutation_lock,
};
pub use paths::{AppPaths, PathsError};
pub use persistence::{SwitchPersistenceError, save_switch_transaction};
pub use recovery::{
    AntigravityAdapter, AntigravityPlatform, FirefoxAdapter, FirefoxPlatform, GenericAppAdapter,
    RecoveryAdapter, RecoveryError, RecoveryRegistry, SystemAntigravityPlatform,
    SystemFirefoxPlatform, SystemVsCodePlatform, SystemWarpPlatform, VsCodeAdapter, VsCodePlatform,
    WarpAdapter, WarpPlatform, default_recovery_registry,
};
pub use runtime::{RuntimeError, RuntimeState, UrlSessionState};
pub use snapshot::{SnapshotWindowReport, snapshot_workspace};
pub use spaces::{
    DesktopSpace, DisplaySpaces, PlacementChange, SpaceError, SpaceInventory, WindowPlacement,
    capture_desktop_placement, current_desktop_placement, list_spaces, move_window_to_desktop,
    window_placement,
};
pub use switcher::{SwitchError, SwitchReport, switch_workspace};
pub use urls::{
    ConfiguredUrlUpdate, SystemUrlOpener, UrlError, UrlLaunchFailure, UrlLaunchReport, UrlOpener,
    WorkspaceUrlState, WorkspaceUrlStatus, add_urls_to_workspace, current_boot_id,
    launch_workspace_urls, normalize_url, normalize_urls, recovery_managed_urls,
    remove_urls_from_workspace, workspace_url_statuses,
};
pub use windows::{
    BrowserTabState, DesktopPlacement, RecoveryKind, RecoveryState, TerminalTabState, WindowBounds,
    WindowError, WindowInfo, WindowResolution, WindowState, WindowStatus, inspect_windows,
    list_all_windows, list_windows, reconcile_windows, resolve_window,
    screen_recording_permission_granted,
};

pub mod accessibility;
pub mod config;
pub mod paths;
pub mod persistence;
pub mod recovery;
pub mod runtime;
pub mod snapshot;
pub mod spaces;
pub mod switcher;
pub mod windows;

pub use accessibility::{
    AccessibilityError, WindowActionFailure, WindowActionReport, close_windows, minimize_windows,
    minimize_windows_best_effort, request_accessibility_permission, restore_windows,
};
pub use config::{Config, ConfigError, Service, Workspace};
pub use paths::{AppPaths, PathsError};
pub use persistence::{SwitchPersistenceError, save_switch_transaction};
pub use recovery::{
    AntigravityAdapter, AntigravityPlatform, FirefoxAdapter, FirefoxPlatform, GenericAppAdapter,
    RecoveryAdapter, RecoveryError, RecoveryRegistry, SystemAntigravityPlatform,
    SystemFirefoxPlatform, SystemVsCodePlatform, SystemWarpPlatform, VsCodeAdapter, VsCodePlatform,
    WarpAdapter, WarpPlatform, default_recovery_registry,
};
pub use runtime::{RuntimeError, RuntimeState};
pub use snapshot::{SnapshotWindowReport, snapshot_workspace};
pub use spaces::{
    DesktopSpace, DisplaySpaces, PlacementChange, SpaceError, SpaceInventory, WindowPlacement,
    capture_desktop_placement, current_desktop_placement, list_spaces, move_window_to_desktop,
    window_placement,
};
pub use switcher::{SwitchError, switch_workspace};
pub use windows::{
    BrowserTabState, DesktopPlacement, RecoveryKind, RecoveryState, TerminalTabState, WindowBounds,
    WindowError, WindowInfo, WindowResolution, WindowState, WindowStatus, inspect_windows,
    list_all_windows, list_windows, reconcile_windows, resolve_window,
};

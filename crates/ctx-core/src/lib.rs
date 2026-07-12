pub mod accessibility;
pub mod config;
pub mod paths;
pub mod runtime;
pub mod switcher;
pub mod windows;

pub use accessibility::{
    AccessibilityError, minimize_windows, request_accessibility_permission, restore_windows,
};
pub use config::{Config, ConfigError, Service, Workspace};
pub use paths::{AppPaths, PathsError};
pub use runtime::{RuntimeError, RuntimeState};
pub use switcher::{SwitchError, switch_workspace};
pub use windows::{WindowBounds, WindowError, WindowInfo, list_all_windows, list_windows};

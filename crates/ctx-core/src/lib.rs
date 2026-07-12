pub mod config;
pub mod paths;
pub mod windows;

pub use config::{Config, ConfigError, Service, Workspace};
pub use paths::{AppPaths, PathsError};
pub use windows::{WindowError, WindowInfo, list_all_windows, list_windows};

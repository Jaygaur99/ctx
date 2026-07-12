use ctx_core::{
    AccessibilityError, ConfigError, PathsError, RuntimeError, SwitchError, WindowError,
    WindowState,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Paths(#[from] PathsError),

    #[error(transparent)]
    Runtime(#[from] RuntimeError),

    #[error(transparent)]
    Window(#[from] WindowError),

    #[error(transparent)]
    Accessibility(#[from] AccessibilityError),

    #[error(transparent)]
    Switch(#[from] SwitchError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("workspace '{name}' does not exist")]
    WorkspaceMissing { name: String },

    #[error("no active workspace; provide a workspace name")]
    NoActiveWorkspace,

    #[error("window {id} is not selectable; run `ctx listAll` again")]
    WindowNotSelectable { id: u32 },

    #[error("workspace '{workspace}' window {id} is {state:?}")]
    WindowUnavailable {
        workspace: String,
        id: u32,
        state: WindowState,
    },
}

impl CliError {
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Accessibility(AccessibilityError::PermissionRequired)
            | Self::Switch(SwitchError::Accessibility(AccessibilityError::PermissionRequired)) => 3,
            Self::WorkspaceMissing { .. }
            | Self::NoActiveWorkspace
            | Self::WindowNotSelectable { .. }
            | Self::WindowUnavailable { .. }
            | Self::Config(ConfigError::WorkspaceMissing { .. })
            | Self::Config(ConfigError::WorkspaceAlreadyExists { .. })
            | Self::Switch(SwitchError::WorkspaceMissing { .. }) => 2,
            _ => 1,
        }
    }
}

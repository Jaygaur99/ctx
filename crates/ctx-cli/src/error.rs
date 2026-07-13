use ctx_core::{
    AccessibilityError, ConfigError, PathsError, RecoveryError, RuntimeError, SpaceError,
    SwitchError, SwitchPersistenceError, WindowError, WindowState,
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
    Recovery(#[from] RecoveryError),

    #[error(transparent)]
    Space(#[from] SpaceError),

    #[error(transparent)]
    Switch(#[from] SwitchError),

    #[error(transparent)]
    SwitchPersistence(#[from] SwitchPersistenceError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("workspace '{name}' does not exist")]
    WorkspaceMissing { name: String },

    #[error("no active workspace; provide a workspace name")]
    NoActiveWorkspace,

    #[error("window {id} is not selectable; run `ctx listAll` again")]
    WindowNotSelectable { id: u32 },

    #[error("window {id} is not ignored; run `ctx listAll` to inspect exclusions")]
    WindowNotIgnored { id: u32 },

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
            Self::Window(WindowError::ScreenRecordingPermissionRequired)
            | Self::Accessibility(AccessibilityError::PermissionRequired)
            | Self::Switch(SwitchError::Accessibility(AccessibilityError::PermissionRequired)) => 3,
            Self::WorkspaceMissing { .. }
            | Self::NoActiveWorkspace
            | Self::WindowNotSelectable { .. }
            | Self::WindowNotIgnored { .. }
            | Self::WindowUnavailable { .. }
            | Self::Config(ConfigError::WorkspaceMissing { .. })
            | Self::Config(ConfigError::WorkspaceAlreadyExists { .. })
            | Self::Switch(SwitchError::WorkspaceMissing { .. }) => 2,
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_recording_permission_failure_uses_permission_exit_code() {
        let error = CliError::Window(WindowError::ScreenRecordingPermissionRequired);

        assert_eq!(error.exit_code(), 3);
    }
}

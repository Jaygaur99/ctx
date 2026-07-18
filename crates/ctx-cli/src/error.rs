use ctx_core::{
    AccessibilityError, ConfigError, CtxAppError, PathsError, RecoveryError, RuntimeError,
    SpaceError, SwitchError, SwitchPersistenceError, UrlError, WindowError, WindowState,
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

    #[error(transparent)]
    Url(#[from] UrlError),

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

    #[error("URL '{url}' is not configured for workspace '{workspace}'")]
    UrlNotConfigured { workspace: String, url: String },

    #[error("{failed} workspace URL(s) could not be opened")]
    UrlLaunchPartial { failed: usize },
}

impl From<CtxAppError> for CliError {
    fn from(error: CtxAppError) -> Self {
        match error {
            CtxAppError::Paths(error) => Self::Paths(error),
            CtxAppError::Config(error) => Self::Config(error),
            CtxAppError::Runtime(error) => Self::Runtime(error),
            CtxAppError::Window(error) => Self::Window(error),
            CtxAppError::Url(error) => Self::Url(error),
            CtxAppError::Switch(error) => Self::Switch(error),
            CtxAppError::Persistence(error) => Self::SwitchPersistence(error),
            CtxAppError::WorkspaceMissing { name } => Self::WorkspaceMissing { name },
        }
    }
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
            | Self::UrlNotConfigured { .. }
            | Self::Config(ConfigError::WorkspaceMissing { .. })
            | Self::Config(ConfigError::WorkspaceAlreadyExists { .. })
            | Self::Switch(SwitchError::WorkspaceMissing { .. })
            | Self::Url(
                UrlError::Invalid { .. }
                | UrlError::UnsupportedScheme { .. }
                | UrlError::CredentialsNotAllowed { .. }
                | UrlError::NotConfigured { .. },
            ) => 2,
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

    #[test]
    fn application_errors_preserve_existing_exit_categories() {
        let missing = CliError::from(CtxAppError::WorkspaceMissing {
            name: "missing".to_string(),
        });
        let permission = CliError::from(CtxAppError::Window(
            WindowError::ScreenRecordingPermissionRequired,
        ));

        assert_eq!(missing.exit_code(), 2);
        assert_eq!(permission.exit_code(), 3);
    }
}

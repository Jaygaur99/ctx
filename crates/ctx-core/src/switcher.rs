use thiserror::Error;

use crate::{AccessibilityError, Config, RuntimeState, minimize_windows, restore_windows};

#[derive(Debug, Error)]
pub enum SwitchError {
    #[error("workspace '{name}' does not exist")]
    WorkspaceMissing { name: String },

    #[error("active workspace '{name}' no longer exists in the configuration")]
    ActiveWorkspaceMissing { name: String },

    #[error(transparent)]
    Accessibility(#[from] AccessibilityError),
}

pub fn switch_workspace(
    config: &Config,
    state: &mut RuntimeState,
    target_name: &str,
) -> Result<(), SwitchError> {
    let target = config
        .workspace(target_name)
        .ok_or_else(|| SwitchError::WorkspaceMissing {
            name: target_name.to_string(),
        })?;

    if state.active_workspace.as_deref() == Some(target_name) {
        restore_windows(&target.windows)?;
        return Ok(());
    }

    let previous = state
        .active_workspace
        .as_deref()
        .map(|name| {
            config
                .workspace(name)
                .ok_or_else(|| SwitchError::ActiveWorkspaceMissing {
                    name: name.to_string(),
                })
        })
        .transpose()?;

    if let Some(previous) = previous {
        minimize_windows(&previous.windows)?;
    }

    if let Err(error) = restore_windows(&target.windows) {
        if let Some(previous) = previous {
            let _ = restore_windows(&previous.windows);
        }

        return Err(error.into());
    }

    state.active_workspace = Some(target_name.to_string());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_target_before_window_operations() {
        let config = Config::from_yaml("version: 1\nworkspaces: {}\n").unwrap();
        let mut state = RuntimeState::default();

        let error = switch_workspace(&config, &mut state, "missing").unwrap_err();

        assert!(matches!(error, SwitchError::WorkspaceMissing { .. }));
        assert_eq!(state, RuntimeState::default());
    }
}

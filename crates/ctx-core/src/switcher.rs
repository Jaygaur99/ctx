use thiserror::Error;

use crate::{
    AccessibilityError, Config, RuntimeState, WindowState, list_all_windows, minimize_windows,
    reconcile_windows, restore_windows,
};

#[derive(Debug, Error)]
pub enum SwitchError {
    #[error("workspace '{name}' does not exist")]
    WorkspaceMissing { name: String },

    #[error("active workspace '{name}' no longer exists in the configuration")]
    ActiveWorkspaceMissing { name: String },

    #[error(transparent)]
    Accessibility(#[from] AccessibilityError),

    #[error(transparent)]
    Discovery(#[from] crate::WindowError),

    #[error("workspace '{workspace}' window {id} is {state:?}")]
    WindowUnavailable {
        workspace: String,
        id: u32,
        state: WindowState,
    },
}

pub fn switch_workspace(
    config: &mut Config,
    state: &mut RuntimeState,
    target_name: &str,
) -> Result<(), SwitchError> {
    if !config.workspaces.contains_key(target_name) {
        return Err(SwitchError::WorkspaceMissing {
            name: target_name.to_string(),
        });
    }

    let current_windows = list_all_windows()?;
    let relevant_names = [state.active_workspace.as_deref(), Some(target_name)];

    for (name, workspace) in &mut config.workspaces {
        let statuses = reconcile_windows(&mut workspace.windows, &current_windows);

        if relevant_names.contains(&Some(name.as_str()))
            && let Some(unavailable) = statuses
                .into_iter()
                .find(|status| status.resolved_id.is_none())
        {
            return Err(SwitchError::WindowUnavailable {
                workspace: name.clone(),
                id: unavailable.saved_id,
                state: unavailable.state,
            });
        }
    }

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
        let mut config = Config::from_yaml("version: 1\nworkspaces: {}\n").unwrap();
        let mut state = RuntimeState::default();

        let error = switch_workspace(&mut config, &mut state, "missing").unwrap_err();

        assert!(matches!(error, SwitchError::WorkspaceMissing { .. }));
        assert_eq!(state, RuntimeState::default());
    }
}

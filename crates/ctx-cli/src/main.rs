mod cli;
mod error;

use std::{collections::BTreeMap, path::PathBuf, process::ExitCode};

use clap::Parser;
use cli::{Cli, Commands, WindowFilters};
use ctx_core::{
    AppPaths, Config, RuntimeState, WindowInfo, WindowResolution, WindowState, WindowStatus,
    close_windows, inspect_windows, list_all_windows, list_windows, minimize_windows_best_effort,
    reconcile_windows, resolve_window, switch_workspace,
};
use error::CliError;
use serde_json::{Value, json};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json_output = cli.json;

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let exit_code = error.exit_code();
            if json_output {
                eprintln!(
                    "{}",
                    json!({ "error": error.to_string(), "exit_code": exit_code })
                );
            } else {
                eprintln!("ctx: {error}");
            }
            ExitCode::from(exit_code)
        }
    }
}

fn run(cli: Cli) -> Result<(), CliError> {
    let Cli {
        config,
        json,
        command,
    } = cli;

    match command {
        Commands::Init => init_config(config, json),
        Commands::List { filters } => list_window_command(config, false, filters, json),
        Commands::ListAll { filters } => list_window_command(config, true, filters, json),
        Commands::Add { name, window_ids } => add_workspace(config, name, window_ids, json),
        Commands::Switch { name } => switch_to_workspace(config, name, json),
        Commands::Status => show_status(config, json),
        Commands::HideAll => hide_all(config, json),
        Commands::Show { name } => show_workspace(config, name, json),
        Commands::Remove { name } => remove_workspace(config, name, json),
        Commands::Close { name } => close_workspace(config, name, json),
    }
}

fn hide_all(config_override: Option<PathBuf>, json_output: bool) -> Result<(), CliError> {
    let app_paths = AppPaths::discover()?;
    let config_path = config_override.unwrap_or(app_paths.config_file);
    let mut config = Config::load(&config_path)?;
    let state = RuntimeState::load(app_paths.runtime_file)?;
    let active_name = state.active_workspace.ok_or(CliError::NoActiveWorkspace)?;
    let current_windows = list_all_windows()?;
    let active =
        config
            .workspaces
            .get_mut(&active_name)
            .ok_or_else(|| CliError::WorkspaceMissing {
                name: active_name.clone(),
            })?;
    let statuses = reconcile_windows(&mut active.windows, &current_windows);
    ensure_windows_resolved(&active_name, &statuses)?;
    let active_ids: std::collections::BTreeSet<_> =
        active.windows.iter().map(|window| window.id).collect();
    let windows_to_hide: Vec<_> = current_windows
        .into_iter()
        .filter(|window| !active_ids.contains(&window.id))
        .collect();
    let report = minimize_windows_best_effort(&windows_to_hide)?;

    config.save(config_path)?;

    if json_output {
        print_json(json!({
            "active_workspace": active_name,
            "hidden": report.affected,
            "skipped": report.skipped,
        }))?;
    } else {
        println!(
            "Hid {} windows outside workspace '{}'.",
            report.affected.len(),
            active_name
        );
        for failure in report.skipped {
            println!(
                "Skipped window {} ({}): {}",
                failure.id, failure.owner, failure.error
            );
        }
    }

    Ok(())
}

fn init_config(config_override: Option<PathBuf>, json_output: bool) -> Result<(), CliError> {
    let config_path = resolve_config_path(config_override)?;

    Config::init(&config_path)?;
    if json_output {
        print_json(json!({ "config": config_path }))?;
    } else {
        println!("Created config: {}", config_path.display());
    }

    Ok(())
}

fn list_window_command(
    config_override: Option<PathBuf>,
    include_all_desktops: bool,
    filters: WindowFilters,
    json_output: bool,
) -> Result<(), CliError> {
    let config_path = resolve_config_path(config_override)?;
    let config = Config::load(config_path)?;
    let all_windows = list_all_windows()?;
    let listed_windows = if include_all_desktops {
        all_windows.clone()
    } else {
        list_windows()?
    };
    let assignments = assignment_map(&config, &all_windows);
    let filtered: Vec<_> = listed_windows
        .into_iter()
        .filter(|window| window_matches_filters(window, &filters))
        .collect();

    if json_output {
        let output: Vec<_> = filtered
            .iter()
            .map(|window| {
                json!({
                    "id": window.id,
                    "pid": window.pid,
                    "application": window.owner,
                    "title": window.title,
                    "bounds": window.bounds,
                    "assigned_to": assignments.get(&window.id).cloned().unwrap_or_default(),
                })
            })
            .collect();
        print_json(json!(output))?;
    } else {
        println!(
            "{:<10}  {:<8}  {:<24}  {:<20}  TITLE",
            "ID", "PID", "APPLICATION", "WORKSPACES"
        );

        for window in filtered {
            let assigned = assignments
                .get(&window.id)
                .map(|names| names.join(","))
                .unwrap_or_else(|| "-".to_string());
            println!(
                "{:<10}  {:<8}  {:<24}  {:<20}  {}",
                window.id,
                window.pid,
                window.owner,
                assigned,
                window.title.as_deref().unwrap_or("<untitled>")
            );
        }
    }

    Ok(())
}

fn add_workspace(
    config_override: Option<PathBuf>,
    name: String,
    window_ids: Vec<u32>,
    json_output: bool,
) -> Result<(), CliError> {
    let config_path = resolve_config_path(config_override)?;
    let mut config = Config::load(&config_path)?;
    let available: BTreeMap<_, _> = list_all_windows()?
        .into_iter()
        .map(|window| (window.id, window))
        .collect();
    let mut selected: Vec<WindowInfo> = Vec::with_capacity(window_ids.len());

    for id in window_ids {
        let window = available
            .get(&id)
            .ok_or(CliError::WindowNotSelectable { id })?;

        if !selected.iter().any(|selected| selected.id == id) {
            selected.push(window.clone());
        }
    }

    config.add_workspace(&name, selected.clone())?;
    config.save(&config_path)?;

    if json_output {
        print_json(json!({
            "workspace": name,
            "windows": selected,
            "config": config_path,
        }))?;
    } else {
        println!("Added workspace '{name}' to {}", config_path.display());
    }

    Ok(())
}

fn switch_to_workspace(
    config_override: Option<PathBuf>,
    name: String,
    json_output: bool,
) -> Result<(), CliError> {
    let app_paths = AppPaths::discover()?;
    let config_path = config_override.unwrap_or(app_paths.config_file);
    let mut config = Config::load(&config_path)?;
    let mut state = RuntimeState::load(&app_paths.runtime_file)?;

    switch_workspace(&mut config, &mut state, &name)?;
    config.save(&config_path)?;
    state.save(app_paths.runtime_file)?;

    if json_output {
        print_json(json!({ "active_workspace": name }))?;
    } else {
        println!("Switched to workspace '{name}'");
    }

    Ok(())
}

fn show_status(config_override: Option<PathBuf>, json_output: bool) -> Result<(), CliError> {
    let app_paths = AppPaths::discover()?;
    let config_path = config_override.unwrap_or(app_paths.config_file);
    let config = Config::load(&config_path)?;
    let state = RuntimeState::load(app_paths.runtime_file)?;
    let all_windows = list_all_windows()?;
    let visible_windows = list_windows()?;

    if json_output {
        let workspaces: Vec<_> = config
            .workspaces
            .iter()
            .map(|(name, workspace)| {
                json!({
                    "name": name,
                    "active": state.active_workspace.as_deref() == Some(name),
                    "windows": inspect_windows(&workspace.windows, &all_windows, &visible_windows),
                })
            })
            .collect();
        print_json(json!({
            "config": config_path,
            "active_workspace": state.active_workspace,
            "workspaces": workspaces,
        }))?;
    } else {
        println!("Config: {}", config_path.display());
        println!(
            "Active: {}",
            state.active_workspace.as_deref().unwrap_or("<none>")
        );
        println!("Workspaces: {}", config.workspaces.len());

        for (name, workspace) in &config.workspaces {
            let marker = if state.active_workspace.as_deref() == Some(name) {
                "*"
            } else {
                " "
            };
            println!("{marker} {name}");
            print_window_statuses(&inspect_windows(
                &workspace.windows,
                &all_windows,
                &visible_windows,
            ));
        }
    }

    Ok(())
}

fn show_workspace(
    config_override: Option<PathBuf>,
    name: String,
    json_output: bool,
) -> Result<(), CliError> {
    let app_paths = AppPaths::discover()?;
    let config_path = config_override.unwrap_or(app_paths.config_file);
    let config = Config::load(config_path)?;
    let state = RuntimeState::load(app_paths.runtime_file)?;
    let workspace = config
        .workspace(&name)
        .ok_or_else(|| CliError::WorkspaceMissing { name: name.clone() })?;
    let statuses = inspect_windows(&workspace.windows, &list_all_windows()?, &list_windows()?);
    let active = state.active_workspace.as_deref() == Some(&name);

    if json_output {
        print_json(json!({
            "name": name,
            "active": active,
            "path": workspace.path,
            "services": workspace.services,
            "urls": workspace.urls,
            "windows": statuses,
        }))?;
    } else {
        println!("Workspace: {name}");
        println!("Active: {active}");
        print_window_statuses(&statuses);
    }

    Ok(())
}

fn remove_workspace(
    config_override: Option<PathBuf>,
    name: String,
    json_output: bool,
) -> Result<(), CliError> {
    let app_paths = AppPaths::discover()?;
    let config_path = config_override.unwrap_or(app_paths.config_file);
    let mut config = Config::load(&config_path)?;
    let mut state = RuntimeState::load(&app_paths.runtime_file)?;

    config.remove_workspace(&name)?;
    config.save(&config_path)?;

    if state.active_workspace.as_deref() == Some(&name) {
        state.active_workspace = None;
        state.save(app_paths.runtime_file)?;
    }

    if json_output {
        print_json(json!({ "removed_workspace": name }))?;
    } else {
        println!("Removed workspace '{name}'");
    }

    Ok(())
}

fn close_workspace(
    config_override: Option<PathBuf>,
    name: Option<String>,
    json_output: bool,
) -> Result<(), CliError> {
    let app_paths = AppPaths::discover()?;
    let config_path = config_override.unwrap_or(app_paths.config_file);
    let mut config = Config::load(&config_path)?;
    let mut state = RuntimeState::load(&app_paths.runtime_file)?;
    let name = name
        .or_else(|| state.active_workspace.clone())
        .ok_or(CliError::NoActiveWorkspace)?;
    let workspace = config
        .workspaces
        .get_mut(&name)
        .ok_or_else(|| CliError::WorkspaceMissing { name: name.clone() })?;
    let statuses = reconcile_windows(&mut workspace.windows, &list_all_windows()?);

    ensure_windows_resolved(&name, &statuses)?;
    close_windows(&workspace.windows)?;
    config.save(config_path)?;

    if state.active_workspace.as_deref() == Some(&name) {
        state.active_workspace = None;
        state.save(app_paths.runtime_file)?;
    }

    if json_output {
        print_json(json!({ "closed_workspace": name }))?;
    } else {
        println!("Closed workspace '{name}'");
    }

    Ok(())
}

fn assignment_map(config: &Config, current_windows: &[WindowInfo]) -> BTreeMap<u32, Vec<String>> {
    let mut assignments: BTreeMap<u32, Vec<String>> = BTreeMap::new();

    for (name, workspace) in &config.workspaces {
        for saved in &workspace.windows {
            if let WindowResolution::Resolved(current) = resolve_window(saved, current_windows) {
                assignments
                    .entry(current.id)
                    .or_default()
                    .push(name.clone());
            }
        }
    }

    assignments
}

fn window_matches_filters(window: &WindowInfo, filters: &WindowFilters) -> bool {
    let app_matches = filters.app.as_ref().is_none_or(|application| {
        window
            .owner
            .to_lowercase()
            .contains(&application.to_lowercase())
    });
    let pid_matches = filters.pid.is_none_or(|pid| window.pid == pid);

    app_matches && pid_matches
}

fn ensure_windows_resolved(workspace: &str, statuses: &[WindowStatus]) -> Result<(), CliError> {
    if let Some(unavailable) = statuses.iter().find(|status| status.resolved_id.is_none()) {
        return Err(CliError::WindowUnavailable {
            workspace: workspace.to_string(),
            id: unavailable.saved_id,
            state: unavailable.state,
        });
    }

    Ok(())
}

fn print_window_statuses(statuses: &[WindowStatus]) {
    for status in statuses {
        println!(
            "    {:<10} {:<10} {:<24} {}",
            status
                .resolved_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| status.saved_id.to_string()),
            window_state_label(status.state),
            status.owner,
            status.title.as_deref().unwrap_or("<untitled>")
        );
    }
}

fn window_state_label(state: WindowState) -> &'static str {
    match state {
        WindowState::Visible => "visible",
        WindowState::Minimized => "minimized",
        WindowState::Ambiguous => "ambiguous",
        WindowState::Missing => "missing",
    }
}

fn print_json(value: Value) -> Result<(), CliError> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn resolve_config_path(config_override: Option<PathBuf>) -> Result<PathBuf, CliError> {
    Ok(config_override.unwrap_or(AppPaths::discover()?.config_file))
}

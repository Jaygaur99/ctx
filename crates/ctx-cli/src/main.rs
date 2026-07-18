mod cli;
mod error;

use std::{collections::BTreeMap, path::PathBuf, process::ExitCode};

use clap::Parser;
use cli::{Cli, Commands, UrlCommands, WindowFilters};
use ctx_core::{
    AppPaths, Config, CtxApp, GenericAppAdapter, RuntimeState, UrlError, UrlLaunchReport,
    WindowInfo, WindowResolution, WindowState, WindowStatus, WorkspaceUrlState, WorkspaceUrlStatus,
    add_urls_to_workspace, capture_desktop_placement, close_windows, current_boot_id,
    default_recovery_registry, list_all_windows, list_spaces, list_windows,
    minimize_windows_best_effort, reconcile_windows, remove_urls_from_workspace, resolve_window,
    save_switch_transaction, snapshot_workspace, window_placement, workspace_url_statuses,
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
        Commands::Spaces { window_id } => show_spaces(window_id, json),
        Commands::Add { name, window_ids } => add_workspace(config, name, window_ids, json),
        Commands::Switch { name } => switch_to_workspace(config, name, json),
        Commands::Snapshot { name } => snapshot(config, name, json),
        Commands::Url { command } => url_command(config, command, json),
        Commands::Status => show_status(config, json),
        Commands::HideAll => hide_all(config, json),
        Commands::Ignore { window_ids } => ignore_windows(config, window_ids, json),
        Commands::Unignore { window_ids } => unignore_windows(config, window_ids, json),
        Commands::Show { name } => show_workspace(config, name, json),
        Commands::Remove { name } => remove_workspace(config, name, json),
        Commands::Close { name } => close_workspace(config, name, json),
    }
}

fn show_spaces(window_id: Option<u32>, json_output: bool) -> Result<(), CliError> {
    if let Some(window_id) = window_id {
        let placement = window_placement(window_id)?;
        if json_output {
            print_json(serde_json::to_value(placement)?)?;
        } else {
            println!(
                "Window {}: Desktop {} on display {} (space {})",
                placement.window_id,
                placement.desktop_ordinal,
                placement.display_uuid,
                placement.space_id
            );
        }
        return Ok(());
    }
    let inventory = list_spaces()?;
    if json_output {
        print_json(serde_json::to_value(inventory)?)?;
    } else {
        for display in inventory.displays {
            println!("Display {}", display.uuid);
            for desktop in display.desktops {
                let current = if desktop.id == display.current_space_id {
                    " *"
                } else {
                    ""
                };
                println!(
                    "  Desktop {} (space {}){current}",
                    desktop.ordinal, desktop.id
                );
            }
        }
    }
    Ok(())
}

fn snapshot(
    config_override: Option<PathBuf>,
    name: Option<String>,
    json_output: bool,
) -> Result<(), CliError> {
    let app_paths = AppPaths::discover()?;
    let config_path = config_override.unwrap_or(app_paths.config_file);
    let mut config = Config::load(&config_path)?;
    let state = RuntimeState::load(app_paths.runtime_file)?;
    let name = name
        .or(state.active_workspace)
        .ok_or(CliError::NoActiveWorkspace)?;
    let workspace = config
        .workspaces
        .get_mut(&name)
        .ok_or_else(|| CliError::WorkspaceMissing { name: name.clone() })?;
    let report = snapshot_workspace(
        workspace,
        &list_all_windows()?,
        &default_recovery_registry(),
        &GenericAppAdapter,
    )?;
    config.save(&config_path)?;

    if json_output {
        print_json(json!({
            "workspace": name,
            "windows": report,
            "config": config_path,
        }))?;
    } else {
        let captured = report.iter().filter(|window| window.captured).count();
        let degraded = report
            .iter()
            .filter(|window| window.warning.is_some() || window.placement_warning.is_some())
            .count();
        println!(
            "Snapshotted {captured}/{} windows in workspace '{name}'.",
            report.len()
        );
        for window in report {
            if let Some(warning) = window.warning {
                println!(
                    "Warning for window {} ({}): {warning}",
                    window.id, window.application
                );
            }
            if let Some(warning) = window.placement_warning {
                println!(
                    "Placement warning for window {} ({}): {warning}",
                    window.id, window.application
                );
            }
        }
        if degraded > 0 {
            println!(
                "Snapshot is degraded for {degraded} window(s); do not close them if exact context recovery is required."
            );
        }
    }

    Ok(())
}

fn url_command(
    config_override: Option<PathBuf>,
    command: UrlCommands,
    json_output: bool,
) -> Result<(), CliError> {
    match command {
        UrlCommands::Add { workspace, urls } => {
            add_workspace_urls(config_override, workspace, urls, json_output)
        }
        UrlCommands::Remove { workspace, urls } => {
            remove_workspace_urls(config_override, workspace, urls, json_output)
        }
        UrlCommands::List { workspace } => {
            list_workspace_urls(config_override, workspace, json_output)
        }
        UrlCommands::Open { workspace, force } => {
            open_workspace_urls(config_override, workspace, force, json_output)
        }
    }
}

fn add_workspace_urls(
    config_override: Option<PathBuf>,
    workspace_name: String,
    urls: Vec<String>,
    json_output: bool,
) -> Result<(), CliError> {
    let config_path = resolve_config_path(config_override)?;
    let mut config = Config::load(&config_path)?;
    let workspace =
        config
            .workspaces
            .get_mut(&workspace_name)
            .ok_or_else(|| CliError::WorkspaceMissing {
                name: workspace_name.clone(),
            })?;
    let update = add_urls_to_workspace(workspace, &urls)?;
    let configured = workspace.urls.clone();
    config.save(&config_path)?;

    if json_output {
        print_json(json!({
            "workspace": workspace_name,
            "added": update.added,
            "already_present": update.already_present,
            "urls": configured,
            "config": config_path,
        }))?;
    } else {
        for url in &update.added {
            println!("Added URL to '{workspace_name}': {url}");
        }
        for url in &update.already_present {
            println!("URL already configured for '{workspace_name}': {url}");
        }
    }
    Ok(())
}

fn remove_workspace_urls(
    config_override: Option<PathBuf>,
    workspace_name: String,
    urls: Vec<String>,
    json_output: bool,
) -> Result<(), CliError> {
    let app_paths = AppPaths::discover()?;
    let config_path = config_override.unwrap_or(app_paths.config_file);
    let mut config = Config::load(&config_path)?;
    let mut state = RuntimeState::load(&app_paths.runtime_file)?;
    let workspace =
        config
            .workspaces
            .get_mut(&workspace_name)
            .ok_or_else(|| CliError::WorkspaceMissing {
                name: workspace_name.clone(),
            })?;
    let requested = match remove_urls_from_workspace(workspace, &urls) {
        Ok(removed) => removed,
        Err(UrlError::NotConfigured { url }) => {
            return Err(CliError::UrlNotConfigured {
                workspace: workspace_name,
                url,
            });
        }
        Err(error) => return Err(error.into()),
    };
    let remaining = workspace.urls.clone();
    for url in &requested {
        state.clear_workspace_url(&workspace_name, url);
    }
    save_switch_transaction(&config, &config_path, &state, &app_paths.runtime_file)?;

    if json_output {
        print_json(json!({
            "workspace": workspace_name,
            "removed": requested,
            "urls": remaining,
            "config": config_path,
        }))?;
    } else {
        for url in requested {
            println!("Removed URL from '{workspace_name}': {url}");
        }
    }
    Ok(())
}

fn list_workspace_urls(
    config_override: Option<PathBuf>,
    workspace_name: Option<String>,
    json_output: bool,
) -> Result<(), CliError> {
    let app_paths = AppPaths::discover()?;
    let config_path = config_override.unwrap_or(app_paths.config_file);
    let config = Config::load(config_path)?;
    let state = RuntimeState::load(app_paths.runtime_file)?;
    let boot_id = current_boot_id()?;
    let names: Vec<_> = match workspace_name {
        Some(name) => {
            if !config.workspaces.contains_key(&name) {
                return Err(CliError::WorkspaceMissing { name });
            }
            vec![name]
        }
        None => config.workspaces.keys().cloned().collect(),
    };

    if json_output {
        let workspaces: Vec<_> = names
            .iter()
            .map(|name| {
                let workspace = config.workspace(name).expect("workspace name validated");
                json!({
                    "name": name,
                    "urls": workspace.urls,
                    "url_statuses": workspace_url_statuses(name, workspace, &state, &boot_id),
                })
            })
            .collect();
        print_json(json!({ "workspaces": workspaces }))?;
    } else {
        for name in names {
            let workspace = config.workspace(&name).expect("workspace name validated");
            println!("Workspace: {name}");
            print_url_statuses(&workspace_url_statuses(&name, workspace, &state, &boot_id));
        }
    }
    Ok(())
}

fn open_workspace_urls(
    config_override: Option<PathBuf>,
    workspace_name: Option<String>,
    force: bool,
    json_output: bool,
) -> Result<(), CliError> {
    let app = CtxApp::discover(config_override)?;
    let workspace_name = workspace_name
        .or(app.active_workspace()?)
        .ok_or(CliError::NoActiveWorkspace)?;
    let report = app.open_workspace_urls(&workspace_name, force)?;
    print_url_launch_report(&report, json_output)?;
    if report.has_failures() {
        return Err(CliError::UrlLaunchPartial {
            failed: report.failed.len(),
        });
    }
    Ok(())
}

fn print_url_launch_report(report: &UrlLaunchReport, json_output: bool) -> Result<(), CliError> {
    if json_output {
        return print_json(serde_json::to_value(report)?);
    }
    for url in &report.opened {
        println!("Opened URL for '{}': {url}", report.workspace);
    }
    for url in &report.already_opened {
        println!("Already opened this boot for '{}': {url}", report.workspace);
    }
    for url in &report.recovery_managed {
        println!(
            "Managed by browser recovery for '{}': {url}",
            report.workspace
        );
    }
    for failure in &report.failed {
        println!(
            "Failed to open URL for '{}': {} ({})",
            report.workspace, failure.url, failure.error
        );
    }
    Ok(())
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
    let ignored_statuses = reconcile_windows(&mut config.ignored_windows, &current_windows);
    let ignored_ids: std::collections::BTreeSet<_> = ignored_statuses
        .into_iter()
        .filter_map(|status| status.resolved_id)
        .collect();
    let windows_to_hide: Vec<_> = current_windows
        .into_iter()
        .filter(|window| !active_ids.contains(&window.id) && !ignored_ids.contains(&window.id))
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

fn ignore_windows(
    config_override: Option<PathBuf>,
    window_ids: Vec<u32>,
    json_output: bool,
) -> Result<(), CliError> {
    let config_path = resolve_config_path(config_override)?;
    let mut config = Config::load(&config_path)?;
    let current_windows = list_all_windows()?;
    reconcile_windows(&mut config.ignored_windows, &current_windows);
    let available: BTreeMap<_, _> = current_windows
        .into_iter()
        .map(|window| (window.id, window))
        .collect();
    let mut ignored = Vec::new();

    for id in window_ids {
        let window = available
            .get(&id)
            .ok_or(CliError::WindowNotSelectable { id })?;
        if !config
            .ignored_windows
            .iter()
            .any(|ignored| ignored.id == id)
        {
            config.ignored_windows.push(window.clone());
            ignored.push(window.clone());
        }
    }

    config.save(&config_path)?;
    if json_output {
        print_json(json!({ "ignored": ignored, "config": config_path }))?;
    } else {
        println!("Ignored {} windows for hideAll.", ignored.len());
    }

    Ok(())
}

fn unignore_windows(
    config_override: Option<PathBuf>,
    window_ids: Vec<u32>,
    json_output: bool,
) -> Result<(), CliError> {
    let config_path = resolve_config_path(config_override)?;
    let mut config = Config::load(&config_path)?;
    reconcile_windows(&mut config.ignored_windows, &list_all_windows()?);

    for id in &window_ids {
        if !config
            .ignored_windows
            .iter()
            .any(|ignored| ignored.id == *id)
        {
            return Err(CliError::WindowNotIgnored { id: *id });
        }
    }

    let ids: std::collections::BTreeSet<_> = window_ids.into_iter().collect();
    config
        .ignored_windows
        .retain(|window| !ids.contains(&window.id));
    config.save(&config_path)?;

    if json_output {
        print_json(json!({ "unignored": ids, "config": config_path }))?;
    } else {
        println!(
            "Removed {} windows from the hideAll ignore list.",
            ids.len()
        );
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
                    "bundle_id": window.bundle_id,
                    "application_path": window.application_path,
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
            let mut window = window.clone();
            match capture_desktop_placement(window.id) {
                Ok(placement) => window.placement = Some(placement),
                Err(error) => {
                    window.placement_warning =
                        Some(format!("Desktop placement capture failed: {error}"));
                }
            }
            selected.push(window);
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
    let report = CtxApp::discover(config_override)?.switch_workspace(&name)?;

    if json_output {
        print_json(json!({
            "active_workspace": name,
            "urls": report.urls,
        }))?;
    } else {
        println!("Switched to workspace '{name}'");
        print_url_launch_report(&report.urls, false)?;
    }

    Ok(())
}

fn show_status(config_override: Option<PathBuf>, json_output: bool) -> Result<(), CliError> {
    let overview = CtxApp::discover(config_override)?.overview()?;

    if json_output {
        let workspaces: Vec<_> = overview
            .workspaces
            .iter()
            .map(|workspace| {
                json!({
                    "name": workspace.name,
                    "active": workspace.active,
                    "urls": workspace.urls,
                    "url_statuses": workspace.url_statuses,
                    "windows": workspace.windows,
                })
            })
            .collect();
        print_json(json!({
            "config": overview.config_path,
            "active_workspace": overview.active_workspace,
            "workspaces": workspaces,
        }))?;
    } else {
        println!("Config: {}", overview.config_path.display());
        println!(
            "Active: {}",
            overview.active_workspace.as_deref().unwrap_or("<none>")
        );
        println!("Workspaces: {}", overview.workspaces.len());

        for workspace in &overview.workspaces {
            let marker = if workspace.active { "*" } else { " " };
            println!("{marker} {}", workspace.name);
            print_window_statuses(&workspace.windows);
            print_url_statuses(&workspace.url_statuses);
        }
    }

    Ok(())
}

fn show_workspace(
    config_override: Option<PathBuf>,
    name: String,
    json_output: bool,
) -> Result<(), CliError> {
    let overview = CtxApp::discover(config_override)?.overview()?;
    let workspace = overview
        .workspaces
        .iter()
        .find(|workspace| workspace.name == name)
        .ok_or_else(|| CliError::WorkspaceMissing { name: name.clone() })?;

    if json_output {
        print_json(json!({
            "name": name,
            "active": workspace.active,
            "path": workspace.path,
            "services": workspace.services,
            "urls": workspace.urls,
            "url_statuses": workspace.url_statuses,
            "windows": workspace.windows,
        }))?;
    } else {
        println!("Workspace: {name}");
        println!("Active: {}", workspace.active);
        print_window_statuses(&workspace.windows);
        print_url_statuses(&workspace.url_statuses);
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
    if state.active_workspace.as_deref() == Some(&name) {
        state.active_workspace = None;
    }
    state.clear_workspace_urls(&name);
    save_switch_transaction(&config, &config_path, &state, &app_paths.runtime_file)?;

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

    for saved in &config.ignored_windows {
        if let WindowResolution::Resolved(current) = resolve_window(saved, current_windows) {
            assignments
                .entry(current.id)
                .or_default()
                .push("<ignored>".to_string());
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
            "    {:<10} {:<10} {:<12} {:<24} {}",
            status
                .resolved_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| status.saved_id.to_string()),
            window_state_label(status.state),
            recovery_label(status),
            status.owner,
            status.title.as_deref().unwrap_or("<untitled>")
        );
        if let Some(warning) = &status.recovery_warning {
            println!("      recovery warning: {warning}");
        }
        if let Some(placement) = &status.placement {
            println!(
                "      placement: Desktop {} on display {}",
                placement.desktop_ordinal, placement.display_uuid
            );
        }
        if let Some(warning) = &status.placement_warning {
            println!("      placement warning: {warning}");
        }
    }
}

fn print_url_statuses(statuses: &[WorkspaceUrlStatus]) {
    if statuses.is_empty() {
        return;
    }
    println!("    URLs:");
    for status in statuses {
        println!("      {:<17} {}", url_state_label(status.state), status.url);
        if let Some(error) = &status.error {
            println!("        warning: {error}");
        }
    }
}

fn url_state_label(state: WorkspaceUrlState) -> &'static str {
    match state {
        WorkspaceUrlState::Pending => "pending",
        WorkspaceUrlState::Opened => "opened",
        WorkspaceUrlState::RecoveryManaged => "recovery-managed",
        WorkspaceUrlState::Failed => "failed",
    }
}

fn recovery_label(status: &WindowStatus) -> String {
    match (
        status.recovery_kind,
        status.recovery_ready,
        status.recovery_degraded,
    ) {
        (Some(kind), true, true) => format!("{}:degraded", kind.as_str()),
        (Some(kind), true, false) => kind.as_str().to_string(),
        (Some(kind), false, _) => format!("{}:not-ready", kind.as_str()),
        (None, _, _) => "none".to_string(),
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

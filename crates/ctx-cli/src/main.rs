mod cli;

use std::{collections::BTreeMap, error::Error, io, path::PathBuf, process::ExitCode};

use clap::Parser;
use cli::{Cli, Commands};
use ctx_core::{
    AppPaths, Config, RuntimeState, WindowInfo, list_all_windows, list_windows, switch_workspace,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ctx: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            init_config(cli.config)?;
        }
        Commands::List => show_windows(list_windows()?),
        Commands::ListAll => show_windows(list_all_windows()?),
        Commands::Add { name, window_ids } => {
            add_workspace(cli.config, name, window_ids)?;
        }
        Commands::Switch { name } => {
            switch_to_workspace(cli.config, name)?;
        }
        Commands::Status => {
            show_status(cli.config)?;
        }
        Commands::Close { name } => {
            println!("Closing workspace: {name:?}");
        }
    }

    Ok(())
}

fn show_windows(windows: Vec<WindowInfo>) {
    println!("{:<10}  {:<8}  {:<24}  TITLE", "ID", "PID", "APPLICATION");

    for window in windows {
        println!(
            "{:<10}  {:<8}  {:<24}  {}",
            window.id,
            window.pid,
            window.owner,
            window.title.as_deref().unwrap_or("<untitled>")
        );
    }
}

fn add_workspace(
    config_override: Option<PathBuf>,
    name: String,
    window_ids: Vec<u32>,
) -> Result<(), Box<dyn Error>> {
    let config_path = resolve_config_path(config_override)?;
    let mut config = Config::load(&config_path)?;
    let available: BTreeMap<_, _> = list_all_windows()?
        .into_iter()
        .map(|window| (window.id, window))
        .collect();
    let mut selected: Vec<WindowInfo> = Vec::with_capacity(window_ids.len());

    for id in window_ids {
        let window = available.get(&id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("window {id} is not currently visible; run `ctx list` again"),
            )
        })?;

        if !selected.iter().any(|selected| selected.id == id) {
            selected.push(window.clone());
        }
    }

    config.add_workspace(&name, selected)?;
    config.save(&config_path)?;

    println!("Added workspace '{name}' to {}", config_path.display());

    Ok(())
}

fn switch_to_workspace(
    config_override: Option<PathBuf>,
    name: String,
) -> Result<(), Box<dyn Error>> {
    let app_paths = AppPaths::discover()?;
    let config_path = config_override.unwrap_or(app_paths.config_file);
    let config = Config::load(config_path)?;
    let mut state = RuntimeState::load(&app_paths.runtime_file)?;

    switch_workspace(&config, &mut state, &name)?;
    state.save(app_paths.runtime_file)?;

    println!("Switched to workspace '{name}'");

    Ok(())
}

fn init_config(config_override: Option<PathBuf>) -> Result<(), Box<dyn Error>> {
    let config_path = resolve_config_path(config_override)?;

    Config::init(&config_path)?;
    println!("Created config: {}", config_path.display());

    Ok(())
}

fn show_status(config_override: Option<PathBuf>) -> Result<(), Box<dyn Error>> {
    let app_paths = AppPaths::discover()?;
    let config_path = config_override.unwrap_or(app_paths.config_file);
    let config = Config::load(&config_path)?;
    let state = RuntimeState::load(app_paths.runtime_file)?;

    println!("Config: {}", config_path.display());
    println!(
        "Active: {}",
        state.active_workspace.as_deref().unwrap_or("<none>")
    );
    println!("Workspaces: {}", config.workspaces.len());

    for name in config.workspaces.keys() {
        println!("  {name}");
    }

    Ok(())
}

fn resolve_config_path(config_override: Option<PathBuf>) -> Result<PathBuf, Box<dyn Error>> {
    let path = match config_override {
        Some(path) => path,
        None => AppPaths::discover()?.config_file,
    };

    Ok(path)
}

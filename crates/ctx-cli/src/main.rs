mod cli;

use std::{error::Error, path::PathBuf, process::ExitCode};

use clap::Parser;
use cli::{Cli, Commands};
use ctx_core::{AppPaths, Config};

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
        Commands::Switch { name } => {
            println!("Switching to workspace: {name}");
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

fn init_config(config_override: Option<PathBuf>) -> Result<(), Box<dyn Error>> {
    let config_path = resolve_config_path(config_override)?;

    Config::init(&config_path)?;
    println!("Created config: {}", config_path.display());

    Ok(())
}

fn show_status(config_override: Option<PathBuf>) -> Result<(), Box<dyn Error>> {
    let config_path = resolve_config_path(config_override)?;
    let config = Config::load(&config_path)?;

    println!("Config: {}", config_path.display());
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

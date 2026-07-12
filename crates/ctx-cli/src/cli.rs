use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ctx", version, about = "Switch between development workspaces")]
pub struct Cli {
    /// Use a specific workspace configuration file
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum Commands {
    /// Create an empty workspace configuration
    Init,

    /// Switch to another workspace
    Switch {
        /// Name of the workspace
        name: String,
    },

    /// Show the current workspace
    Status,

    /// Stop and close a workspace
    Close {
        /// Workspace name; defaults to the active workspace
        name: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_switch_command() {
        let cli = Cli::try_parse_from(["ctx", "switch", "devlayout"]).unwrap();

        assert_eq!(
            cli.command,
            Commands::Switch {
                name: "devlayout".to_string()
            }
        )
    }

    #[test]
    fn parses_init_command() {
        let cli = Cli::try_parse_from(["ctx", "init"]).unwrap();

        assert_eq!(cli.command, Commands::Init);
    }

    #[test]
    fn parses_close_without_name() {
        let cli = Cli::try_parse_from(["ctx", "close"]).unwrap();

        assert_eq!(cli.command, Commands::Close { name: None });
    }

    #[test]
    fn rejects_switch_without_name() {
        let result = Cli::try_parse_from(["ctx", "switch"]);

        assert!(result.is_err());
    }

    #[test]
    fn parses_global_config_override() {
        let cli =
            Cli::try_parse_from(["ctx", "status", "--config", "/tmp/ctx-workspaces.yaml"]).unwrap();

        assert_eq!(cli.config, Some(PathBuf::from("/tmp/ctx-workspaces.yaml")));
        assert_eq!(cli.command, Commands::Status);
    }
}

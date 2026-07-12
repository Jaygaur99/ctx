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

    /// List visible windows that can be added to a workspace
    List,

    /// List windows across all macOS Desktops, including minimized windows
    #[command(name = "listAll", visible_alias = "list-all")]
    ListAll,

    /// Create a workspace from visible window IDs
    Add {
        /// Name for the new workspace
        name: String,

        /// Window IDs shown by `ctx list`
        #[arg(required = true, num_args = 1..)]
        window_ids: Vec<u32>,
    },

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
    fn parses_add_command() {
        let cli = Cli::try_parse_from(["ctx", "add", "backend", "42", "57"]).unwrap();

        assert_eq!(
            cli.command,
            Commands::Add {
                name: "backend".to_string(),
                window_ids: vec![42, 57],
            }
        );
    }

    #[test]
    fn parses_list_all_command_and_alias() {
        let camel_case = Cli::try_parse_from(["ctx", "listAll"]).unwrap();
        let kebab_case = Cli::try_parse_from(["ctx", "list-all"]).unwrap();

        assert_eq!(camel_case.command, Commands::ListAll);
        assert_eq!(kebab_case.command, Commands::ListAll);
    }

    #[test]
    fn add_requires_at_least_one_window() {
        let result = Cli::try_parse_from(["ctx", "add", "backend"]);

        assert!(result.is_err());
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

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ctx", version, about = "Switch between development workspaces")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum Commands {
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
    fn parses_close_without_name() {
        let cli = Cli::try_parse_from(["ctx", "close"]).unwrap();

        assert_eq!(cli.command, Commands::Close { name: None });
    }

    #[test]
    fn rejects_switch_without_name() {
        let result = Cli::try_parse_from(["ctx", "switch"]);

        assert!(result.is_err());
    }
}

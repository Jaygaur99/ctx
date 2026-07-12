use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ctx", version, about = "Switch between development workspaces")]
pub struct Cli {
    /// Use a specific workspace configuration file
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Emit machine-readable JSON
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum Commands {
    /// Create an empty workspace configuration
    Init,

    /// List visible windows that can be added to a workspace
    List {
        #[command(flatten)]
        filters: WindowFilters,
    },

    /// List selectable windows across all macOS Desktops, including minimized windows
    #[command(name = "listAll", visible_alias = "list-all")]
    ListAll {
        #[command(flatten)]
        filters: WindowFilters,
    },

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

    /// Capture durable recovery state for a workspace
    Snapshot {
        /// Workspace name; defaults to the active workspace
        name: Option<String>,
    },

    /// Show the current workspace
    Status,

    /// Minimize every window except those in the active workspace
    #[command(name = "hideAll", visible_aliases = ["hide-all", "hidall"])]
    HideAll,

    /// Exclude windows from hideAll
    Ignore {
        /// Window IDs shown by `ctx listAll`
        #[arg(required = true, num_args = 1..)]
        window_ids: Vec<u32>,
    },

    /// Remove windows from the hideAll exclusion list
    Unignore {
        /// Current window IDs shown by `ctx listAll`
        #[arg(required = true, num_args = 1..)]
        window_ids: Vec<u32>,
    },

    /// Show one workspace and its live window state
    Show {
        /// Workspace name
        name: String,
    },

    /// Remove a workspace definition
    Remove {
        /// Workspace name
        name: String,
    },

    /// Stop and close a workspace
    Close {
        /// Workspace name; defaults to the active workspace
        name: Option<String>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Args)]
pub struct WindowFilters {
    /// Filter by application name (case-insensitive substring)
    #[arg(long, value_name = "NAME")]
    pub app: Option<String>,

    /// Filter by owning process ID
    #[arg(long)]
    pub pid: Option<i32>,
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

        assert_eq!(
            camel_case.command,
            Commands::ListAll {
                filters: WindowFilters::default()
            }
        );
        assert_eq!(
            kebab_case.command,
            Commands::ListAll {
                filters: WindowFilters::default()
            }
        );
    }

    #[test]
    fn parses_window_filters() {
        let cli = Cli::try_parse_from(["ctx", "listAll", "--app", "code", "--pid", "42"]).unwrap();

        assert_eq!(
            cli.command,
            Commands::ListAll {
                filters: WindowFilters {
                    app: Some("code".to_string()),
                    pid: Some(42),
                }
            }
        );
    }

    #[test]
    fn parses_json_globally() {
        let before = Cli::try_parse_from(["ctx", "--json", "status"]).unwrap();
        let after = Cli::try_parse_from(["ctx", "status", "--json"]).unwrap();

        assert!(before.json);
        assert!(after.json);
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
    fn parses_snapshot_with_optional_workspace() {
        let active = Cli::try_parse_from(["ctx", "snapshot"]).unwrap();
        let named = Cli::try_parse_from(["ctx", "snapshot", "coding"]).unwrap();

        assert_eq!(active.command, Commands::Snapshot { name: None });
        assert_eq!(
            named.command,
            Commands::Snapshot {
                name: Some("coding".to_string())
            }
        );
    }

    #[test]
    fn parses_hide_all_command_and_aliases() {
        for name in ["hideAll", "hide-all", "hidall"] {
            let cli = Cli::try_parse_from(["ctx", name]).unwrap();

            assert_eq!(cli.command, Commands::HideAll);
        }
    }

    #[test]
    fn parses_ignore_commands() {
        let ignore = Cli::try_parse_from(["ctx", "ignore", "42", "57"]).unwrap();
        let unignore = Cli::try_parse_from(["ctx", "unignore", "42"]).unwrap();

        assert_eq!(
            ignore.command,
            Commands::Ignore {
                window_ids: vec![42, 57]
            }
        );
        assert_eq!(
            unignore.command,
            Commands::Unignore {
                window_ids: vec![42]
            }
        );
    }

    #[test]
    fn parses_show_and_remove_commands() {
        let show = Cli::try_parse_from(["ctx", "show", "coding"]).unwrap();
        let remove = Cli::try_parse_from(["ctx", "remove", "coding"]).unwrap();

        assert_eq!(
            show.command,
            Commands::Show {
                name: "coding".to_string()
            }
        );
        assert_eq!(
            remove.command,
            Commands::Remove {
                name: "coding".to_string()
            }
        );
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

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

    /// Inspect macOS displays and their ordered user Desktops
    Spaces {
        /// Show the Desktop placement of one Core Graphics window
        window_id: Option<u32>,
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

    /// Manage workspace URL launch shortcuts
    Url {
        #[command(subcommand)]
        command: UrlCommands,
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

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum UrlCommands {
    /// Add one or more URLs to a workspace
    Add {
        /// Workspace name
        workspace: String,

        /// HTTP or HTTPS URLs to add
        #[arg(required = true, num_args = 1..)]
        urls: Vec<String>,
    },

    /// Remove one or more URLs from a workspace
    Remove {
        /// Workspace name
        workspace: String,

        /// Configured URLs to remove
        #[arg(required = true, num_args = 1..)]
        urls: Vec<String>,
    },

    /// List configured URLs and their runtime state
    List {
        /// Workspace name; omit to list every workspace
        workspace: Option<String>,
    },

    /// Open configured URLs in the macOS default browser
    Open {
        /// Workspace name; defaults to the active workspace
        workspace: Option<String>,

        /// Open every configured URL even if it was already launched this boot
        #[arg(long)]
        force: bool,
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
    fn parses_spaces_command() {
        let cli = Cli::try_parse_from(["ctx", "spaces"]).unwrap();

        assert_eq!(cli.command, Commands::Spaces { window_id: None });
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
    fn parses_nested_url_commands() {
        let add = Cli::try_parse_from([
            "ctx",
            "url",
            "add",
            "coding",
            "https://example.com",
            "http://localhost:3000",
        ])
        .unwrap();
        let remove =
            Cli::try_parse_from(["ctx", "url", "remove", "coding", "https://example.com"]).unwrap();
        let list = Cli::try_parse_from(["ctx", "url", "list"]).unwrap();
        let open = Cli::try_parse_from(["ctx", "url", "open", "coding", "--force"]).unwrap();

        assert_eq!(
            add.command,
            Commands::Url {
                command: UrlCommands::Add {
                    workspace: "coding".to_string(),
                    urls: vec![
                        "https://example.com".to_string(),
                        "http://localhost:3000".to_string(),
                    ],
                }
            }
        );
        assert_eq!(
            remove.command,
            Commands::Url {
                command: UrlCommands::Remove {
                    workspace: "coding".to_string(),
                    urls: vec!["https://example.com".to_string()],
                }
            }
        );
        assert_eq!(
            list.command,
            Commands::Url {
                command: UrlCommands::List { workspace: None }
            }
        );
        assert_eq!(
            open.command,
            Commands::Url {
                command: UrlCommands::Open {
                    workspace: Some("coding".to_string()),
                    force: true,
                }
            }
        );
    }

    #[test]
    fn url_add_and_remove_require_at_least_one_url() {
        assert!(Cli::try_parse_from(["ctx", "url", "add", "coding"]).is_err());
        assert!(Cli::try_parse_from(["ctx", "url", "remove", "coding"]).is_err());
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

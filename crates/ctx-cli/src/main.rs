use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ctx", version, about = "Switch between development workspaces")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Switch { name } => {
            println!("Switching to workspace: {name}");
        }
        Commands::Status => {
            println!("Showing workspace status");
        }
        Commands::Close { name } => {
            println!("Closing workspace: {name:?}");
        }
    }
}

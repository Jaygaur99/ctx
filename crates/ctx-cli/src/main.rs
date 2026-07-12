mod cli;

use clap::Parser;
use cli::{Cli, Commands};

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

//! Lazy Herd — Herdr plugin host for lazy sub-plugins.

mod app;
mod herdr;
mod plugins;
mod registry;
mod storage;
mod ui;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "lazy-herd", version, about = "Lazy Herd Herdr plugin")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the interactive TUI (default when no subcommand).
    Tui,
    /// Print plugin id / version for smoke tests.
    Version,
    /// Run doctor checks non-interactively (JSON to stdout).
    Doctor {
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Commands::Tui) {
        Commands::Tui => app::run(),
        Commands::Version => {
            println!("lazy-herd {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Commands::Doctor { json } => plugins::doctor::run_cli(json),
    }
}

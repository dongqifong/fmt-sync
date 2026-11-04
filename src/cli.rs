use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{
    adapters,
    config::{Config, Language},
    init, report,
};

#[derive(Parser)]
#[command(
    version,
    about = "Format and lint projects using their local configuration"
)]
struct Cli {
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Initialize the current project, preserving existing native settings
    Init {
        #[arg(long, value_delimiter = ',', required = true, num_args = 1..)]
        lang: Vec<Language>,
        #[arg(long, value_name = "DIRECTORY")]
        preset: Option<PathBuf>,
    },
    /// Format using fmt-sync.toml in the current directory
    Format,
    /// Check formatting and lint without modifying source files
    Check,
}

pub fn run() -> i32 {
    let cli = Cli::parse();
    match dispatch(cli) {
        Ok(code) => code,
        Err(error) => {
            let yellow = clap::builder::styling::AnsiColor::Yellow.on_default();
            anstream::eprintln!("{yellow}error: {error:#}{yellow:#}");
            2
        }
    }
}

fn dispatch(cli: Cli) -> Result<i32> {
    let root = std::env::current_dir()?;
    match cli.command {
        Action::Init { lang, preset } => {
            for message in init::initialize(&root, &lang, preset.as_deref())? {
                println!("{message}");
            }
            Ok(0)
        }
        Action::Format | Action::Check => {
            let config = Config::load(&root)?;
            let results = adapters::execute(&root, &config, matches!(cli.command, Action::Check));
            Ok(report::print(&results))
        }
    }
}

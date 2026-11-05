use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{
    config::Config, execution::RunMode, init, language::Language, orchestrator, report, update,
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
    /// Update to the latest stable release by compiling with Cargo
    Update,
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
    if matches!(cli.command, Action::Update) {
        return Ok(update::run());
    }
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
            let results = orchestrator::execute(
                &root,
                &config,
                if matches!(cli.command, Action::Check) {
                    RunMode::Check
                } else {
                    RunMode::Format
                },
            );
            Ok(report::print(&results))
        }
        Action::Update => unreachable!(),
    }
}

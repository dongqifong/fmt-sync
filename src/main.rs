mod adapters;
mod cli;
mod config;
mod execution;
mod init;
mod language;
mod native;
mod orchestrator;
mod paths;
mod report;
mod runner;
mod scope;
mod tools;
mod update;

fn main() {
    update::cleanup();
    std::process::exit(cli::run());
}

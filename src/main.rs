mod adapters;
mod cli;
mod config;
mod init;
mod native;
mod report;
mod rust_project;
mod scope;
mod tools;
mod update;

fn main() {
    update::cleanup();
    std::process::exit(cli::run());
}

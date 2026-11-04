mod adapters;
mod cli;
mod config;
mod init;
mod native;
mod report;
mod rust_project;
mod scope;
mod tools;

fn main() {
    std::process::exit(cli::run());
}

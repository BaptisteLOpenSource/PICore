mod commands;
mod output;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "picore", version)]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    quiet: bool,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    Root {
        #[command(subcommand)]
        action: RootCmd,
    },
    Index,
    List,
}

#[derive(Subcommand)]
pub enum RootCmd {
    Add { path: PathBuf },
    Remove { path: PathBuf },
    List,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    commands::run(cli.json, cli.quiet, &cli.command)
}

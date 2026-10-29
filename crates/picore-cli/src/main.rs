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
    Gui,
    #[command(visible_alias = "o")]
    Open {
        name: String,
    },
    Status {
        name: Option<String>,
        #[arg(long)]
        remote: bool,
    },
    Worktrees {
        name: Option<String>,
    },
    Prune {
        target: Option<String>,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        force: bool,
    },
    #[command(hide = true)]
    Completions {
        shell: clap_complete::Shell,
    },
    #[command(hide = true)]
    Man,
}

#[derive(Subcommand)]
pub enum RootCmd {
    Add { path: PathBuf },
    Remove { path: PathBuf },
    List,
}

fn main() -> ExitCode {
    #[cfg(unix)]
    {
        // Rust ignores SIGPIPE by default, so restore it: pipes end us quietly.
        unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };
    }
    let cli = Cli::parse();
    commands::run(cli.json, cli.quiet, &cli.command)
}

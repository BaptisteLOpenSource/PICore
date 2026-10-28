use clap::Parser;

#[derive(Parser)]
#[command(name = "picore", version)]
struct Cli {}

fn main() {
    Cli::parse();
}

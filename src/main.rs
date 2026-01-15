use codecrafters_bittorrent::Cli;

use clap::Parser;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    if let Err(err) = cli.command.run().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

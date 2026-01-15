use codecrafters_bittorrent::Cli;

use clap::Parser;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(err) = cli.command.run().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

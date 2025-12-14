use codecrafters_bittorrent as bit;

use bit::{Cli, Command, bencode::Bencode};
use clap::Parser;
use std::error::Error;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::Decode { token } => {
            let v = Bencode::new(&token)?;
            println!("{v}");
        }
    }

    Ok(())
}

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
            let v = Bencode::parse(token.as_bytes())?;
            println!("{v}");
        }
        Command::Info { path } => {
            let file = std::fs::File::open(path)?;
            let meta_info = bit::file::MetaInfo::new(file)?;
            println!("Tracker URL: {}", meta_info.announce);
            println!("Length: {}", meta_info.info.length);
        }
    }

    Ok(())
}

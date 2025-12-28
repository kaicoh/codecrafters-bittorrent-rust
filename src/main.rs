use codecrafters_bittorrent as bit;

use bit::{
    Cli, Command,
    bencode::{Bencode, Serializer},
};
use clap::Parser;
use serde::Serialize;
use sha1::{Digest, Sha1};
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

            let info = meta_info.info;

            let mut bytes = Vec::new();
            info.serialize(&mut Serializer::new(&mut bytes))?;
            let hash = Sha1::digest(&bytes);
            println!("Info Hash: {:x}", hash);

            println!("Piece Length: {}", info.piece_length);

            println!("Piece Hashes:");
            for hash in info.piece_hashes()? {
                let digest = Sha1::digest(hash);
                println!("{:x}", digest);
            }
        }
    }

    Ok(())
}
